import importlib.util
import json
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock


REPO_ROOT = Path(__file__).resolve().parents[1]
BUILDER_PATH = REPO_ROOT / "scripts" / "build_agent_knowledge.py"
SPEC = importlib.util.spec_from_file_location("build_agent_knowledge", BUILDER_PATH)
assert SPEC and SPEC.loader
BUILDER = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = BUILDER
SPEC.loader.exec_module(BUILDER)


class AuthorityManifestTests(unittest.TestCase):
    def test_supported_sources_exactly_match_top_level_manifest_inventory(self):
        policies = BUILDER.load_authority_manifest()
        manifested_top_level = {
            path for path in policies if Path(path).parent == Path("whitepapers")
        }

        self.assertEqual(BUILDER.supported_top_level_sources(), manifested_top_level)

    def test_unmanifested_supported_source_fails_closed(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            repo_root = Path(temp_dir)
            whitepapers_dir = repo_root / "whitepapers"
            whitepapers_dir.mkdir()
            (whitepapers_dir / "reviewed.pdf").write_bytes(b"")
            (whitepapers_dir / "new_supported_source.html").write_text("")
            manifest_path = whitepapers_dir / "authority_manifest.json"
            manifest_path.write_text(
                json.dumps(
                    {
                        "schema_version": 2,
                        "included_documents": ["whitepapers/reviewed.pdf"],
                        "documents": [],
                    }
                )
            )

            with (
                mock.patch.object(BUILDER, "REPO_ROOT", repo_root),
                mock.patch.object(BUILDER, "WHITEPAPERS_DIR", whitepapers_dir),
                mock.patch.object(BUILDER, "AUTHORITY_MANIFEST_PATH", manifest_path),
            ):
                with self.assertRaisesRegex(
                    SystemExit,
                    r"^Authority manifest/source parity failure: "
                    r"unmanifested supported sources: "
                    r"whitepapers/new_supported_source\.html$",
                ):
                    BUILDER.load_authority_manifest()

    def test_reviewed_exclusions_never_enter_retrieval(self):
        policies = BUILDER.load_authority_manifest()
        retrieved = {
            f"whitepapers/{path.name}" for path in BUILDER.iter_docs(policies)
        }

        self.assertNotIn(
            "whitepapers/_archive/disputed/"
            "mechanics_of_billiards_and_analysis_of_willie_hoppe_s_stroke.pdf",
            retrieved,
        )
        self.assertNotIn(
            "whitepapers/modified_pipe_friction_diagrams_"
            "that_eliminate_trial_and_error_from_traditional_problem_solution_methods.pdf",
            retrieved,
        )
        self.assertNotIn("whitepapers/rolling_friction_intro.pdf", retrieved)

    def test_petit_is_scoped_and_not_a_starter(self):
        policies = BUILDER.load_authority_manifest()
        petit = policies["whitepapers/the_art_of_billiards_play.html"]

        self.assertEqual(petit["retrieval"], "scope_limited")
        self.assertIn("cue_squirt", petit["excluded_scopes"])
        self.assertIn(
            "the_art_of_billiards_play.html",
            BUILDER.PRIMARY_STARTER_DOCS,
            "the regression depends on the manifest overriding the legacy starter list",
        )
        self.assertNotIn(
            "the_art_of_billiards_play.html",
            {
                path.name
                for path in BUILDER.iter_docs(policies)
                if policies.get(f"whitepapers/{path.name}", {}).get("retrieval", "include")
                == "include"
                and path.name in BUILDER.PRIMARY_STARTER_DOCS
            },
        )

    def test_petit_scope_exclusions_drive_formula_and_guide_filtering(self):
        policies = BUILDER.load_authority_manifest()
        petit_path = "whitepapers/the_art_of_billiards_play.html"
        petit = policies[petit_path]
        excluded_scopes = {
            "cue_squirt",
            "off_center_cue_impact",
            "cue_end_mass_calibration",
        }

        self.assertEqual(set(petit["excluded_scopes"]), excluded_scopes)
        for scope in excluded_scopes:
            with self.subTest(scope=scope):
                self.assertFalse(BUILDER.scope_is_allowed(petit, scope))
                single_exclusion = {**petit, "excluded_scopes": [scope]}
                self.assertFalse(BUILDER.formula_candidates_allowed(single_exclusion))
                self.assertFalse(
                    BUILDER.topic_is_allowed(
                        single_exclusion, "cue_ball_motion_and_spin"
                    )
                )

        self.assertFalse(BUILDER.formula_candidates_allowed(petit))
        self.assertFalse(BUILDER.topic_is_allowed(petit, "cue_ball_motion_and_spin"))

    def test_petit_remains_retrievable_for_allowed_scopes(self):
        policies = BUILDER.load_authority_manifest()
        petit_path = "whitepapers/the_art_of_billiards_play.html"
        petit = policies[petit_path]

        retrieved = {
            f"whitepapers/{path.name}" for path in BUILDER.iter_docs(policies)
        }
        self.assertIn(petit_path, retrieved)
        for scope in ("ball_collision", "cloth_motion", "spin_decay", "gearing"):
            with self.subTest(scope=scope):
                self.assertTrue(BUILDER.scope_is_allowed(petit, scope))
        for topic in ("collisions_and_impacts", "history_and_general_physics"):
            with self.subTest(topic=topic):
                self.assertTrue(BUILDER.topic_is_allowed(petit, topic))


if __name__ == "__main__":
    unittest.main()
