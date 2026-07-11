import importlib.util
import sys
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
BUILDER_PATH = REPO_ROOT / "scripts" / "build_agent_knowledge.py"
SPEC = importlib.util.spec_from_file_location("build_agent_knowledge", BUILDER_PATH)
assert SPEC and SPEC.loader
BUILDER = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = BUILDER
SPEC.loader.exec_module(BUILDER)


class AuthorityManifestTests(unittest.TestCase):
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


if __name__ == "__main__":
    unittest.main()
