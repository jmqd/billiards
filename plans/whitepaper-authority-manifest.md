# Establish a reviewed whitepaper authority manifest

Date: 2026-07-10

Severity: High for physics-source retrieval integrity; no direct runtime-physics effect today

Priority: P1

## Problem statement

The knowledge builder currently treats directory membership, broad keyword matches, and incidental repository references as proxies for scientific authority. That is not a reliable contract. A source can become a starter or a high-ranked physics result without a reviewed statement of what it is authoritative for, and unrelated or unreadable files can enter the same retrieval surfaces as implementation-grade billiards sources.

The implementation should introduce a checked-in, declarative authority manifest consumed by `scripts/build_agent_knowledge.py`. The manifest must own canonical source identity, citation aliases, retrieval state, authority tier, starter order, allowed scopes, excluded claims, extraction policy, and citation-source roles. The builder must validate that metadata and deterministically derive every file under `agent_knowledge/`; generated artifacts must never become a second source of truth.

This plan addresses three connected source-integrity failures:

1. Scope-label Régis Petit’s unsound §7.2/C27 cue-impact assumption without discarding the source’s sound collision and cloth-motion sections.
2. Resolve current and nested citations to one canonical record and stop historical/TODO prose from impersonating current implementation authority.
3. Exclude reviewed off-domain/meta documents from physics retrieval and quarantine unreadable sources while retaining billiards-relevant scans for later OCR/review.

## Current behavior and impact

### Observed builder defects

| Current symbol/range | Observed behavior | Impact |
| --- | --- | --- |
| `TOPIC_RULES` and `classify_topics(...)`, `scripts/build_agent_knowledge.py:50-79,354-359` | Classification searches the title, filename, and only the first 4,000 extracted characters for broad words such as `friction`, `impact`, `simulation`, and `robot`. If nothing matches, the source becomes `uncategorized`. | Pipe friction, education, general robotics, and news articles are classified into billiards physics topics; unreadable but relevant scans are indistinguishable from unrelated unreadable files. |
| `PRIMARY_STARTER_DOCS`, `scripts/build_agent_knowledge.py:81-104` | Starter status and intended order are hard-coded separately from any reliability, scope, or extraction metadata. | `the_art_of_billiards_play.html` is promoted generically even though its C27 cue claim is invalid; maintenance can change authority without an auditable review record. |
| `DocRecord`, `scripts/build_agent_knowledge.py:139-154` | The generated model has `topics`, citation booleans, and `primary_start`, but no canonical identity, authority tier, review status, evidence type, allowed scope, claim exclusion, extraction status, or citation locations. | Consumers cannot distinguish a peer-reviewed/experimental source from a meta-index, a scoped derivation from a disputed claim, or an extraction failure from an empty document. |
| `pdf_text(...)`, `scripts/build_agent_knowledge.py:299-307` | A `pdftotext` subprocess failure is collapsed to the same empty string produced by an image-only scan. | Transient tool failure and a genuine no-text scan silently produce the same generated record instead of failing or entering an explicit quarantine state. |
| `parse_todo_aliases(...)` and `gather_repo_citations(...)`, `scripts/build_agent_knowledge.py:404-443` | The builder recursively scans broad repository prose, parses TODO old→new migration notes as live aliases, reduces every citation to `Path(...).name`, and classifies every `.rs` file as code. | Audits, plans, historical notes, and TODO mappings elevate sources like implementation citations; nested paths lose identity; basename collisions are possible; current nested `bil_praa.html` references cannot attach to the indexed top-level Petit record. |
| `iter_docs(...)`, `scripts/build_agent_knowledge.py:446-451` | Every supported top-level document is yielded; only subdirectories are skipped. | Directory membership is effectively an allowlist, even though the directory contains off-domain, meta, historical, and unreadable material. |
| `render_guide(...)`, `scripts/build_agent_knowledge.py:466-540` | `starters` preserves alphabetic `iter_docs()` order rather than `PRIMARY_STARTER_DOCS` order. Topic score is only `(primary_start, cited_by_repo, formula_line_count, title)`. | The guide says “Use this order” but does not use the declared order, and citation/formula volume can outrank scope, authority, experimental basis, extraction quality, or a disputed claim. |
| `main(...)`, `scripts/build_agent_knowledge.py:579-669` | Every yielded source gets an index record, formula section, and corpus section, including empty extraction and irrelevant sources. Generated files are overwritten without a manifest-parity or deterministic-check mode. | Retrieval poison is propagated consistently into `whitepapers_index.jsonl`, `whitepapers_formula_candidates.txt`, `whitepapers_corpus.txt`, and `agent_reading_guide.md`; staleness is not detected before consumers use the files. |

### Observed generated consequences

- `agent_knowledge/agent_reading_guide.md:35-38` reports all 427 top-level documents as indexed and 2,724 harvested formula-like lines.
- The guide’s code/doc-cited list at `agent_knowledge/agent_reading_guide.md:40-61` calls `whitepapers/30_degree_rule_for_caroms.html` and `whitepapers/publications_presentations_and_software_index.html` code-cited, although the source-reliability audit found no current non-generated code/test/doc reference to either. Conversely, `whitepapers/the_physics_of_billiards.html` is currently marked uncited even though `src/lib.rs` directly cites it in three symbols.
- `whitepapers/the_art_of_billiards_play.html` is a starter and high-ranked collision/history source at `agent_knowledge/agent_reading_guide.md:19,113,138`, but its generated record cannot express that §7.2/C27 is excluded from cue-squirt authority while §§7.1 and 7.3-7.5 remain useful.
- The guide exposes unrelated `Uncategorized` records at `agent_knowledge/agent_reading_guide.md:196-210`. Relevant image-only billiards scans and unrelated image-only documents appear in the same unqualified surface.
- `whitepapers/rolling_friction_intro.pdf` has `char_count: 0` but is presented as doc-cited in the guide. It cannot presently substantiate a physics claim.

### User-facing impact

The current artifacts can lead an agent or maintainer to select an irrelevant, unreadable, stale-cited, or out-of-scope source for a physics implementation. The immediate Rust runtime is unaffected, but the pipeline is load-bearing research infrastructure: a bad authority choice can become a later equation, calibration, or model justification. The Petit case demonstrates why whole-document trust is too coarse, while the off-domain and no-extract cases demonstrate why “everything in the directory” is too broad.

## Whitepaper evidence for Petit’s scoped exclusion

### Observed claim

`whitepapers/the_art_of_billiards_play.html`, generated corpus `agent_knowledge/whitepapers_corpus.txt:51123-51155`, develops cue impact using

\[
M\mathbf W_i=M\mathbf W_a+\mathbf P,\qquad
I\boldsymbol\Omega_i=I\boldsymbol\Omega_a+\overrightarrow{GC}\times\mathbf P,
\]

then assumes at `51137-51138`:

\[
\text{(C27)}\qquad \mathbf P\parallel\mathbf W'_a,
\]

because “the hand or bridge guiding the cue completely absorbs the transversal shock wave.” The downstream cue-ball speed expression at `51152-51153` explicitly depends on C27. This is the claim that must be excluded for `cue_squirt`, `off_center_cue_impact`, and cue-end-mass calibration retrieval.

### Observed contradicting evidence

- Shepard, `whitepapers/everything_you_always_wanted_to_know_about_cue_ball_squirt_but_were_afraid_to_ask.pdf`, corpus `22678-22691`, argues that tip-ball contact is too short for the grip/bridge flesh to tighten and transmit the assumed corrective force. The sideways force on the ball has an equal and opposite sideways force on the tip, so the cue’s effective end mass and flex matter.
- High-speed evidence in the corpus at `39441-39450` shows the cue ball already leaving after `1 ms` and separated after `2 ms` while the shaft at the bridge has not moved; maximum bridge-region flex occurs after separation.
- Kim, `whitepapers/motions_of_a_billiard_ball_after_a_cue_stroke.pdf`, corpus `34728-34740`, reports nonzero squirt for off-center impact. For a well-made cue with cue-end/ball mass ratio below `0.1` and impact parameter `b < 0.55R`, squirt is below but not equal to `1°`.

### Numeric reproducer with units

Kim’s carom-table example gives the observable scale of the error. With a long-axis travel distance of approximately `d = 2.84 m` and squirt angle `ψ = 1°`,

\[
\Delta y=d\tan\psi=(2.84\ \mathrm m)\tan(1^\circ)
\approx 0.0496\ \mathrm m=49.6\ \mathrm{mm}.
\]

For Kim’s `R \approx 36.9 mm` carom ball, `b < 0.55R` means `b < 20.3 mm`. Petit C27 constrains the impulse parallel to the cue and therefore predicts zero lateral squirt and `0 mm` lateral error in this setup. The contradiction is material even when the angle is small: the difference is roughly `49.6 mm`, about five-eighths of a carom-ball thickness per Kim’s text. This reproducer is evidence for a retrieval-scope exclusion, not a request to implement a runtime cue-squirt model.

### Sound Petit material that must remain available

Current code cites Petit only for claims outside C27: equal-ball ideal collision, cushion/contact friction structure, cloth sliding and rolling transition, vertical-spin decay, instantaneous-event idealization, throw/contact slip, and gearing. The manifest must therefore keep the source retrievable for reviewed scopes including `ball_ball_collision`, `cushion_collision`, `cloth_sliding_to_rolling`, `vertical_spin_decay`, `instantaneous_collision_event`, and `throw_and_gearing`, while attaching a visible §7.2/C27 warning and making it ineligible for the excluded cue scopes.

## Exact affected files, symbols, and callers

### Source-of-truth and builder

- New human-reviewed source of truth: `whitepapers/authority_manifest.json`.
- `scripts/build_agent_knowledge.py`:
  - remove `PRIMARY_STARTER_DOCS` as an independent authority list;
  - replace authoritative use of `TOPIC_RULES`/`classify_topics(...)` with reviewed manifest scopes;
  - extend or replace `DocRecord` with manifest authority/scope/extraction/citation fields;
  - make `pdf_text(...)` return a structured extraction result rather than collapsing errors and no-text scans;
  - delete `parse_todo_aliases(...)`;
  - make `gather_repo_citations(...)` use full normalized paths, manifest aliases, and reviewed citation-source roles;
  - make `iter_docs(...)` validate against manifest inventory rather than implicitly including every top-level file;
  - extract a deterministic `rank_records_for_scope(...)` used by `render_guide(...)`;
  - make `render_guide(...)`, `render_readme(...)`, and `main(...)` consume validated manifest records and expose authority/scope caveats;
  - add a callable `build_agent_knowledge(...)` seam and CLI `--check` mode so tests and CI do not depend on mutating checked-in output.
- New focused tests: `tests/test_build_agent_knowledge.py`, using Python’s standard-library `unittest` so the root project does not acquire an unrelated Python package dependency.
- Optional command wiring after behavior works: `justfile` recipes for focused regeneration/check commands.

### Current citation callsites to migrate to canonical paths

Migrate all current `whitepapers/art_of_billiards_play_files/bil_praa.html` comments in `src/lib.rs` to `whitepapers/the_art_of_billiards_play.html`. The affected symbols/comments are:

- `CollisionModel`, around `src/lib.rs:732-752`;
- `RailCollisionConfig`, around `797-824`;
- `SlidingFrictionModel`, around `2640-2649`;
- `SpinDecayModel`, around `2651-2660`;
- `compute_next_transition_on_table(...)`, around `5054-5079`;
- `compute_next_ball_ball_collision_on_table(...)`, around `5666-5691`;
- `compute_next_ball_ball_collision_during_current_phases_on_table(...)`, around `5751-5774`;
- `collide_ball_ball_detailed_on_table_with_config(...)`, around `11850-11880`;
- `collide_ball_rail_on_table_with_radius_and_profile(...)`, around `13212-13243`;
- `gearing_english_for_radius(...)`, around `13530-13560`.

The manifest should nevertheless retain the exact nested path as an alias so historical current-source paths resolve deterministically rather than by basename guessing.

Fresh citation parity must also recognize the existing direct `whitepapers/the_physics_of_billiards.html` citations in:

- `compute_next_ball_ball_collision_on_table(...)`, `src/lib.rs:5674-5680`;
- `compute_next_ball_ball_collision_during_current_phases_on_table(...)`, `src/lib.rs:5756-5761`;
- `Ball::ghost_ball(...)`, `src/lib.rs:14240-14253`.

`TODO.org:134-139` records the unresolved nested-versus-top-level decision. Once the canonical callsite migration lands and parity tests pass, remove that completed optional follow-up rather than retaining it as a live alias source.

### Generated consumers

Regenerate, but never hand-edit:

- `agent_knowledge/README.md`;
- `agent_knowledge/whitepapers_index.jsonl`;
- `agent_knowledge/whitepapers_formula_candidates.txt`;
- `agent_knowledge/whitepapers_corpus.txt`;
- `agent_knowledge/agent_reading_guide.md`.

Update `AGENTS.md` only after successful regeneration so it identifies `whitepapers/authority_manifest.json` and `scripts/build_agent_knowledge.py` as the editable sources and explains the new authority/scope fields. Raw whitepapers remain preserved source material, not generated artifacts.

## Root cause

There is no reviewed metadata boundary between raw-source preservation and implementation-authority retrieval. The builder tries to infer domain, authority, citation status, and reading priority from filenames, the first part of extracted text, formula volume, and repository-wide textual coincidence. Those signals are useful only as discovery hints; they cannot answer whether a source is in-domain, extracted successfully, canonical, authoritative for a specific claim, or explicitly excluded.

The fix is not another keyword special case. It is a declarative ownership boundary with strict validation and deterministic derivation.

## Proposed manifest contract

### Ownership

`whitepapers/authority_manifest.json` will be the sole checked-in, human-reviewed metadata source for the raw whitepaper inventory. The raw documents remain the evidence; the manifest records the repository’s reviewed retrieval policy for them. `scripts/build_agent_knowledge.py` validates and applies that policy. Everything under `agent_knowledge/` remains derived output and must be reproducible byte-for-byte from raw sources, the manifest, current allowlisted citations, and the builder.

The format should be JSON because the builder already uses Python’s standard library, JSON is deterministic when emitted with explicit sorting, and no YAML/TOML parser dependency is otherwise needed.

Every supported active top-level source must have exactly one manifest record. Explicit archived records may point below `whitepapers/_archive/...`; their paths must exist and their retrieval state must prevent indexing. A newly added top-level `.pdf`, `.html`, `.htm`, or `.txt` without a manifest record must fail the build instead of silently entering retrieval.

### Top-level schema

| Field | Contract |
| --- | --- |
| `schema_version` | Integer, initially `1`; unknown versions fail with an actionable error. |
| `scope_order` | Ordered, duplicate-free list of allowed retrieval-scope identifiers. It replaces accidental alphabetic topic order and validates every document scope. |
| `citation_sources` | Ordered glob/role rules. Roles are `code`, `docs`, or `context`; only `code` and `docs` can set authority-elevating citation flags. Every scanned repository citation must match exactly one role rule, while `whitepapers/`, `agent_knowledge/`, VCS/build directories, and binary files remain excluded from scanning. |
| `documents` | Canonical-path-sorted list of document records. Paths and aliases are globally unique. Active top-level source coverage is total; archived/excluded paths are allowed but must exist. |

The initial `citation_sources` review must classify `src/**/*.rs` as `code`; current maintained documentation as `docs`; and tests, benches, examples, `physics_audits/**`, `plans/**`, historical root plans, and `TODO.org` as `context` unless a specific maintained document is deliberately promoted. Context citations remain visible in diagnostics/locations but never set `cited_in_code`, `cited_in_docs`, starter status, or authority rank. There is no TODO alias parser and no basename fallback.

### Per-document schema

| Field | Contract |
| --- | --- |
| `path` | Unique normalized repository-relative canonical path. Exact path matching is case-sensitive; `..`, absolute paths, and basename-only identities are rejected. |
| `aliases` | Sorted exact repository-relative citation paths that resolve to `path`. An alias may resolve to one canonical record only. `whitepapers/art_of_billiards_play_files/bil_praa.html` is the representative required mapping. |
| `retrieval` | `include`, `scope_limited`, `quarantine`, or `exclude`. `include` and `scope_limited` may enter retrieval only with usable extraction; `quarantine` and `exclude` never enter index/corpus/formula/topic/starter output. |
| `authority` | `primary`, `secondary`, `contextual`, `disputed`, or `none`. Ranking weights are explicit in code and tested. `disputed`/`none` cannot be starters. |
| `evidence_kinds` | Reviewed list drawn from `peer_reviewed`, `experiment`, `high_speed_video`, `analytical_derivation`, `technical_proof`, `secondary_summary`, `historical`, and `meta`. This is exposed to consumers and used only after exact scope eligibility. |
| `published_year` | Integer or `null`, used only as a deterministic late ranking signal; it never overrides scope, authority, or extraction eligibility. |
| `starter_order` | Unique positive integer or `null`. The guide renders non-null values in ascending order. A scope-limited source may be a starter only when the guide displays its caveat; Petit should be demoted to `null` in this migration. |
| `scopes` | Object with nonempty `allow` and optional `exclude` lists validated against `scope_order`. `scope_limited` requires at least one exclusion. `exclude`/`quarantine` records have no eligible scopes. |
| `claim_exclusions` | Structured list containing `id`, excluded `scopes`, human-readable `claim`, stable source `locator`, `reason`, and exact `contradicted_by` paths/locators. These exclusions are emitted in the JSONL record and warning headers/guide text; they are not inferred from generated corpus line numbers. |
| `extraction_policy` | `required_text`, `quarantine_until_reviewed`, or `not_retrieved`. An extraction subprocess error is always fatal. Successful but empty normalized extraction is allowed only for `quarantine_until_reviewed`; newly successful OCR/text still requires a manifest review before promotion. |
| `review` | Object containing `status: reviewed`, ISO date, and nonempty evidence/basis locators. No retrieval-eligible record may remain pending review. |
| `duplicate_of` | Canonical path or `null`; duplicates may remain preserved but cannot independently gain higher authority/rank than their canonical source. |

### Retrieval-state semantics

- `include`: emitted to all applicable generated retrieval artifacts, ranked only for manifest `allow` scopes.
- `scope_limited`: emitted with visible caveats in the index, corpus header, formula-section header, and guide; excluded claims/scopes are ineligible in `rank_records_for_scope(...)`.
- `quarantine`: raw file and manifest record are preserved, but the file is absent from the generated index, corpus, formula candidates, starter list, and topic maps. This state means “potentially relevant but not presently reviewable/retrievable,” not “bad source.”
- `exclude`: raw file and manifest record are preserved, but the file is absent from all generated retrieval surfaces. This state means “reviewed as off-domain/meta/non-authoritative for this physics corpus,” not “the document’s contents are false.”

A source with `char_count == 0` can never be `include`, `scope_limited`, starter-ranked, code/doc authority, or a top-scope result. A transient extraction command failure must fail the build rather than silently changing a source to quarantine.

## Initial reviewed policy decisions

### Petit scope-limited record

`whitepapers/the_art_of_billiards_play.html` should be `retrieval: scope_limited`, `authority: secondary`, and `starter_order: null`.

Allowed scopes must retain the currently used sound sections:

- `ball_ball_collision` and `instantaneous_collision_event` for §7.1/current collision-event citations;
- `cushion_collision` for the reviewed §7.1 cushion decomposition/friction structure;
- `cloth_sliding_to_rolling` for §7.3 equations including `(M4)`, `(M8)`, and `(M10')`;
- `vertical_spin_decay` for §§7.3-7.5, including `(M13)` through `(M14'')`;
- `throw_and_gearing` for the reviewed `(C6')`, `(C11)`, and `(C13)` contact-slip/adherence relationships.

The record must exclude `cue_squirt`, `off_center_cue_impact`, and `cue_end_mass_calibration`, with a claim exclusion located at `§7.2, Eq. (C27)` and contradictions pointing to Shepard corpus `22678-22691`, high-speed corpus `39441-39450`, and Kim corpus `34728-34740`.

`whitepapers/everything_you_always_wanted_to_know_about_cue_ball_squirt_but_were_afraid_to_ask.pdf` and `whitepapers/motions_of_a_billiard_ball_after_a_cue_stroke.pdf` must be eligible for `cue_squirt` and rank ahead of Petit for that scope. Petit must remain eligible for at least `cloth_sliding_to_rolling` and `throw_and_gearing`, proving this is not a whole-document cull.

### Canonical citation decisions

- Canonical record: `whitepapers/the_art_of_billiards_play.html`.
- Exact legacy/nested alias: `whitepapers/art_of_billiards_play_files/bil_praa.html`.
- Current `src/lib.rs` citations migrate to the canonical record; the alias remains for deterministic resolution of historical references.
- `whitepapers/the_physics_of_billiards.html` is already canonical and must become freshly `cited_in_code: true` with the three current source locations.
- `whitepapers/30_degree_rule_for_caroms.html` and `whitepapers/publications_presentations_and_software_index.html` must not remain code-cited unless a fresh allowlisted code scan finds an actual canonical reference.
- `TODO.org` old→new mappings and plan/audit citations are `context` only and cannot elevate authority.

### Reviewed off-domain/meta exclusions

The initial manifest must explicitly set `retrieval: exclude`, retain the raw files, and supply reviewed reasons for the substantiated paths below.

Full-text/meta false positives:

- `whitepapers/publications_presentations_and_software_index.html` — author bibliography/meta-index, not physics evidence;
- `whitepapers/integrating_mechatronics_into_a_mechanical_engineering_curriculum.pdf` — engineering-education paper;
- `whitepapers/tinkering_makes_comeback_amid_crisis.html` — general news article;
- `whitepapers/websights_physics_of_sports.pdf` — web-links column with only a billiards-site pointer.

No-extract, off-domain false positives:

- `whitepapers/modified_pipe_friction_diagrams_that_eliminate_trial_and_error_from_traditional_problem_solution_methods.pdf`;
- `whitepapers/a_heuristic_application_specific_path_planner_for_robot_motion_planning.pdf`;
- `whitepapers/robot_can_read_terrain_and_adjust.pdf`;
- `whitepapers/security_robot_roams_shrieks_at_intruders.pdf`;
- `whitepapers/simulation_of_multiple_nozzle_surface_finishing_operations.pdf`.

Reviewed off-domain uncategorized records:

- `whitepapers/a_winding_number_and_point_in_polygon_algorithm.pdf`;
- `whitepapers/closed_form_solution_of_the_general_three_dimensional_radiation_configuration_factor_problem_with_microcomputer_solution.pdf`;
- `whitepapers/computer_graphics_modeling_of_anatomy_from_2d_data_acquisition_to_3d_sculpting.pdf`;
- `whitepapers/importing_and_reshaping_digitized_data_for_use_in_rapid_prototyping_a_system_for_sculpting_polygonal_mesh_surfaces.pdf`;
- `whitepapers/matrix_solution_of_digitized_planar_human_body_dynamics_for_biomechanics_laboratory_instruction.pdf`;
- `whitepapers/model_development_and_control_implementation_for_a_magnetic_levitation_apparatus.pdf`;
- `whitepapers/multipulley_belt_drive_mechanics_creep_theory_vs_shear_theory.pdf`;
- `whitepapers/the_best_least_squares_line_fit.pdf`;
- `whitepapers/variable_focus_three_dimensional_laser_digitizing_system.pdf`;
- `whitepapers/ryan_pooh_poohed_those_low_carb_diets.pdf`;
- `whitepapers/ryan_demonstrated_the_basics_of_torsion_mechanics.pdf`;
- `whitepapers/richard_and_jen_ace_stochastics.pdf`.

These are retrieval exclusions, not assertions that each document is internally false. They stay on disk until a separately reviewed repository-retention decision says otherwise.

### No-extract quarantine and scan preservation

Every successful-but-empty extraction that is not a reviewed off-domain exclusion must enter `quarantine` until OCR/extraction and content review succeed. The audit specifically requires preserving these billiards-relevant scans:

- `whitepapers/pool_mythology_what_you_accept_as_truth_just_might_deserve_a_second_look.pdf`;
- `whitepapers/racking_up_the_physics_of_pool.pdf`;
- `whitepapers/big_shot_with_the_help_of_physics_a_teenage_pool_player_goes_pro.pdf`.

Also quarantine `whitepapers/rolling_friction_intro.pdf`: it is plan-cited but presently has no extracted text, so it cannot substantiate an authority claim. Quarantine must not move, delete, or relabel these sources as scientifically wrong. Promotion requires usable extraction plus explicit scope/authority review and a manifest change.

## Ranking and rendering contract

Introduce one deterministic `rank_records_for_scope(records, requested_scope)` implementation and make the guide use it rather than embedding a separate score closure.

Eligibility is decided before scoring:

1. retrieval is `include` or `scope_limited`;
2. extraction status is usable;
3. `requested_scope` is in `scopes.allow`;
4. `requested_scope` is not excluded by `scopes.exclude` or `claim_exclusions`.

Eligible records are ranked, in order, by exact scope match, authority tier, reviewed evidence kind, current code citation, current maintained-doc citation, optional publication year, formula-line count, and canonical path as the final ascending tie-breaker. `starter_order` is a separate explicit ordering used only by “What to read first.” Formula volume and incidental citation can never make an ineligible source eligible.

Generated surfaces must expose enough metadata for consumers to understand the decision:

- JSONL: canonical path, aliases, retrieval state, authority, evidence kinds, scopes, claim exclusions, extraction status, starter order, and sorted citation locations/roles.
- Corpus/formula sections: canonical path plus concise authority/scope/exclusion warning headers. Petit’s C27 text remains visible for auditability but is clearly marked non-authoritative for the excluded scopes.
- Reading guide: manifest starter order, authority/evidence tags, scope caveats, no `Uncategorized` retrieval section, and only eligible topic results.
- README: physical-source, included, scoped, quarantined, and excluded counts; manifest ownership; regeneration/check commands.

## Explicit non-goals

- Moore’s incorrect gyroscopic-precession source, its evidence, reversible archive move, and exact cull mechanics are owned by `plans/cull-moore-gyroscopic-source.md`. This plan only provides the manifest state needed to represent that sibling decision; it does not duplicate or replace it.
- Do not implement cue squirt, cue flexibility, or any other Rust physics behavior. `physics_audits/2026-04-24-cue-cloth-motion.md:105-111` already records the runtime model limitation; this plan governs source selection only.
- Do not cull all of `whitepapers/the_art_of_billiards_play.html`; its reviewed sound sections remain retrievable.
- Do not implement OCR. Quarantine preserves relevant scans until OCR and a later content review are available.
- Do not delete off-domain or quarantined raw files. Retrieval exclusion and repository retention are separate policies.
- Do not hand-edit, patch, or treat any `agent_knowledge/*` file as source metadata.
- Do not redesign PDF title extraction or formula detection beyond making extraction failure/status explicit and attaching scope warnings.
- Do not treat citation presence as proof that a claim is scientifically correct; citation role is only one late ranking signal after manifest eligibility.

## Phased implementation plan

### Phase 1 — Lock observable contracts with focused tests

1. Add `tests/test_build_agent_knowledge.py` with temporary miniature repositories and HTML/TXT fixtures so most behavior can run without Poppler or the 5.7 MB checked-in corpus.
2. Add failing tests for total manifest coverage, unique canonical paths/aliases, invalid/missing paths, unique starter order, known scope identifiers, and rejection of an unreviewed retrieval-eligible source.
3. Add failing citation tests proving that a full nested path and canonical path resolve to one canonical record, basename-only collisions are rejected, TODO/audit references remain context-only, and unresolved code/doc citations fail with source path and line number.
4. Add failing retrieval tests proving excluded and quarantined sources cannot enter any output; an included empty extraction fails; a no-text quarantine remains preserved in manifest/source inventory; and a subprocess extraction failure is fatal.
5. Add failing Petit tests proving C27 exclusions are emitted, the source remains eligible for sound cloth/collision scopes, and Shepard/Kim are eligible ahead of Petit for `cue_squirt`.
6. Add a byte-determinism test that builds the same fixture twice into separate output directories and compares all generated files exactly.

### Phase 2 — Introduce and validate the manifest source of truth

1. Add `whitepapers/authority_manifest.json` with `schema_version: 1`, ordered scope and citation-role definitions, and an explicit record for every active top-level supported document plus any reviewed archived record supplied by sibling work.
2. Review every retrieval-eligible record sufficiently to assign authority, evidence kind, scopes, extraction policy, and starter order; do not bootstrap a silent `include` default. Newly discovered/unreviewed sources fail coverage and must be reviewed or explicitly quarantined.
3. Implement typed manifest dataclasses and `load_authority_manifest(...)`/`validate_authority_manifest(...)` using the Python standard library. Aggregate all validation errors and print canonical paths/fields so a reviewer can repair the manifest in one pass.
4. Remove `PRIMARY_STARTER_DOCS`. Stop using `TOPIC_RULES`/`classify_topics(...)` as authoritative output; if retained as a review aid, its suggestions must not enter generated records or ranking without manifest metadata.
5. Make source iteration follow canonical manifest records and assert exact parity with supported active top-level files. Validate archived/excluded path existence separately.

### Phase 3 — Cut citation identity over to canonical full paths

1. Replace `parse_todo_aliases(...)` and basename sets with a manifest-backed resolver from exact normalized citation path to canonical path.
2. Refactor `gather_repo_citations(...)` to return sorted occurrence records containing canonical source, citing repo path, line, and role. Scan current text sources, apply manifest citation-role rules, and exclude raw/generated/build/VCS trees.
3. Fail on unresolved `whitepapers/...` citations in code or maintained docs. Preserve context-only unresolved references in diagnostics only when their role policy explicitly allows historical paths; they cannot elevate authority.
4. Migrate every current `src/lib.rs` `bil_praa.html` citation listed above to `whitepapers/the_art_of_billiards_play.html` in one clean cutover. Keep the nested alias only in the manifest.
5. Remove the resolved nested-citation follow-up from `TODO.org` after the canonical callsites and parity test pass.
6. Assert current-repo parity: the three `the_physics_of_billiards.html` callsites are code citations; canonical Petit callsites attach to the top-level record; stale `30_degree_rule_for_caroms.html` and publication-index flags disappear unless a real current allowlisted occurrence exists.

### Phase 4 — Enforce extraction, exclusion, and quarantine before rendering

1. Replace the empty-string `pdf_text(...)` contract with `ExtractionResult(status, text, diagnostic)`, distinguishing `ok`, successful `no_text`, and fatal `tool_error`.
2. Normalize text before applying extraction policy. Require usable text for `include`/`scope_limited`; accept empty text only for reviewed `quarantine_until_reviewed`; skip extraction entirely for reviewed `not_retrieved` exclusions when no validation needs it.
3. Apply the initial off-domain exclusions and no-extract quarantines enumerated above before constructing `DocRecord`, formulas, corpus sections, or topic maps.
4. Extend `DocRecord` in a clean schema cutover. Remove ambiguous `primary_start` and `cited_by_repo`; add `starter_order`, authority/evidence fields, retrieval scopes/exclusions, extraction status, and deterministic citation locations. Derive `cited_in_code`/`cited_in_docs` only from role-qualified occurrences.
5. Add parity assertions: each included/scoped record has exactly one JSONL row, corpus delimiter pair, and formula header; excluded/quarantined records have none; no emitted record has unusable extraction.

### Phase 5 — Make ranking and generated outputs authority-aware

1. Add `rank_records_for_scope(...)` with the eligibility and tie-break contract above; make all topic rendering call it.
2. Render “What to read first” strictly by unique `starter_order`, not source iteration order.
3. Emit Petit’s structured C27 warning in JSONL and concise warning text in its corpus/formula/guide headers. Do not remove the text, equations, or sound scopes.
4. Remove excluded/quarantined documents and `Uncategorized` from physics retrieval surfaces. Report counts/statuses in the generated README rather than mixing quarantine with recommended reading.
5. Add CLI `--check`: build to a temporary directory, byte-compare the five expected artifacts, and exit nonzero with the exact stale filenames without modifying checked-in files.
6. Smoke-test a real regeneration through Poppler and confirm the builder’s printed counts reconcile with manifest states and output parity.

### Phase 6 — Final source/docs/generated cleanup after the smoke test passes

1. Add focused `justfile` recipes only if they reduce command drift: one regeneration recipe and one aggregate authority-manifest/check recipe.
2. Update `AGENTS.md` to point maintainers to `whitepapers/authority_manifest.json` and the builder, document canonical/scope-aware lookup, and retain the prohibition on hand-editing generated files.
3. Regenerate all five `agent_knowledge/*` files only by running the builder.
4. Run the focused and aggregate verification commands below and inspect only the reported invariants/errors; do not patch generated output to make checks pass.

## API and data-model cutover

This is an internal build-pipeline schema break with no Rust public API change.

- Replace `PRIMARY_STARTER_DOCS` with manifest `starter_order`; migrate all starter records in one change and leave no compatibility list.
- Replace filename/basename citation identity with canonical repository-relative paths plus exact aliases; migrate all current source callsites and leave no TODO alias parser.
- Replace generated `primary_start` with `starter_order` and ambiguous `cited_by_repo` with role-qualified citation fields/locations. Update all renderer callsites and generated documentation in the same change; do not emit deprecated duplicate fields.
- Replace inferred authoritative `topics` with validated manifest scopes. If a `topics` compatibility name is retained in JSONL, it must be a direct rendering of reviewed scopes, not heuristic output; the preferred clean cutover is `retrieval_scopes` plus `excluded_scopes`.
- Parameterize `build_agent_knowledge(...)` with repository root, manifest path, output directory, and tool paths/extractors so focused tests can use temporary fixtures. `main(...)` remains the thin production CLI caller.
- `rank_records_for_scope(...)` is the single ranking implementation for guide generation and tests; no renderer-local score copy remains.

## Regression and acceptance tests

The implementation is accepted only when all of these observable invariants hold:

1. **Manifest inventory:** every supported active top-level source has exactly one manifest record; every canonical/archived path exists; paths and aliases are unique; new unreviewed documents fail the build.
2. **Citation parity:** every current `whitepapers/...` reference in code/maintained docs resolves to an indexed canonical record or explicit alias. Generated occurrence sets equal a fresh role-allowlisted scan, including path and line; TODO/audit/plan references never set code/doc citation flags.
3. **Current canonical examples:** `the_physics_of_billiards.html` reports its three current `src/lib.rs` callers; all Petit code comments use the top-level canonical path; the nested alias resolves to that same record; stale publication-index and 30-degree-rule code flags are absent without current occurrences.
4. **Petit scope contract:** the generated record and warning headers identify §7.2/C27 and excluded cue scopes; Petit remains retrievable for reviewed collision/cloth/spin/throw scopes; it is not a generic starter.
5. **Query ranking:** `rank_records_for_scope(..., "cue_squirt")` returns Shepard and Kim as eligible results ahead of, or with complete exclusion of, Petit. `rank_records_for_scope(..., "cloth_sliding_to_rolling")` still returns Petit as eligible.
6. **Off-domain exclusion:** every enumerated off-domain/meta path is absent from JSONL, corpus delimiters, formula headers, starter order, guide lists, and topic maps.
7. **No-extract quarantine:** `rolling_friction_intro.pdf` and the three named billiards scans remain present as raw source and manifest records but absent from retrieval artifacts. No emitted record has `char_count == 0`, `no_text`, or tool-error status.
8. **Failure semantics:** a `pdftotext` command error fails the build; it does not silently quarantine a formerly included source. A quarantined scan that begins extracting text remains quarantined until reviewed rather than auto-promoting.
9. **Starter order:** generated “What to read first” paths exactly equal non-null manifest `starter_order` values sorted ascending; source filename order has no effect.
10. **Output parity:** included/scoped document count equals JSONL row count, corpus begin/end delimiter count, and formula-section count. Excluded/quarantined counts reconcile with the manifest and generated README.
11. **Determinism:** two builds from unchanged raw sources, manifest, citations, builder, and extraction tools produce byte-identical output; `--check` succeeds only when checked-in artifacts match.
12. **No runtime regression:** `tests/whitepaper_validation_suite.rs` remains green, confirming the source-pipeline/citation migration did not change the existing cited physics behavior.

## Risks and edge cases

- **Manifest size and review drift:** explicit coverage is intentionally stricter than an override-only file. Keep records canonical-path sorted, aggregate validation errors, and fail additions rather than introducing a permissive default.
- **Alias collisions:** two records can share a basename or nested support filename. Resolve only exact normalized paths and reject duplicate aliases; never restore basename fallback.
- **Citation punctuation/anchors:** references may end with Markdown punctuation or an HTML fragment. Parse the `whitepapers/...` token deterministically, strip only syntactic delimiters/fragment after capture, normalize separators, and test spaces/parentheses without changing case or basename.
- **Glob overlap:** citation-source role rules can overlap. Validation must require exactly one effective role, with an explicit documented specificity rule or a hard error; silent first-match behavior would recreate accidental authority.
- **Context versus maintained docs:** a root plan can contain technically correct citations without being current implementation authority. Role assignment must be reviewed in the manifest; moving a file between roles is an auditable metadata change.
- **Extraction nondeterminism:** Poppler versions can alter whitespace. Determinism is defined for an unchanged tool environment; CI/dev shell pins the environment. Tool errors remain fatal, and fixture determinism tests avoid PDF-tool variability.
- **One-character/garbled extraction:** `char_count > 0` alone is not sufficient scientific quality. The migration review must quarantine known unusable output and retain `review`/`extraction_policy`; do not invent an arbitrary automatic character threshold as authority.
- **Scope granularity:** broad old topics are insufficient for Petit. Use stable physics scopes (`cue_squirt`, `cloth_sliding_to_rolling`, and so on), validate them centrally, and avoid ad hoc free-text scopes that cannot be queried or tested.
- **Claim locator stability:** generated corpus line numbers change after exclusions. Store source section/equation locators and contradiction source paths in the manifest; use corpus line ranges only as review evidence, not runtime selectors.
- **Quarantine visibility:** quarantined scans must not poison retrieval, but they must not be forgotten. The manifest is their durable inventory; generated README counts may summarize quarantine without listing it as recommended physics.
- **Archived disputed material:** sibling cull work may preserve a source below `whitepapers/_archive/...`. Validate the archived path and enforce `exclude`, but let the sibling plan own the move and disputed evidence.
- **Generated schema consumers:** agents currently search `topics`, `primary_start`, and `cited_by_repo`. Update `AGENTS.md` and generated README in the same cutover; do not carry ambiguous compatibility fields indefinitely.

## Source, documentation, and generated-artifact updates

- Add and review: `whitepapers/authority_manifest.json`.
- Modify builder and tests: `scripts/build_agent_knowledge.py`, `tests/test_build_agent_knowledge.py`.
- Canonicalize current citations: `src/lib.rs` callsites listed above.
- Close completed decision: `TODO.org:134-139`.
- After successful real smoke regeneration, update command/ownership documentation in `AGENTS.md` and optionally focused recipes in `justfile`.
- Regenerate only through the builder: `agent_knowledge/README.md`, `whitepapers_index.jsonl`, `whitepapers_formula_candidates.txt`, `whitepapers_corpus.txt`, and `agent_reading_guide.md`.
- Do not modify raw relevant scans, and do not manually edit any generated artifact.

## Verification commands

Run focused contracts first:

```sh
nix develop -c python -m unittest -v \
  tests.test_build_agent_knowledge.ManifestValidationTests \
  tests.test_build_agent_knowledge.CitationResolutionTests \
  tests.test_build_agent_knowledge.RetrievalPolicyTests \
  tests.test_build_agent_knowledge.RankingTests \
  tests.test_build_agent_knowledge.DeterministicBuildTests
```

If the test module is intentionally kept non-package-style, use the equivalent focused discovery command:

```sh
nix develop -c python -m unittest discover -s tests -p 'test_build_agent_knowledge.py' -v
```

Regenerate from the human-reviewed sources, then verify exact checked-in parity without mutation:

```sh
nix develop -c python scripts/build_agent_knowledge.py
nix develop -c python scripts/build_agent_knowledge.py --check
```

Run the relevant aggregate source-pipeline suite, followed by the existing whitepaper-backed physics contract suite:

```sh
nix develop -c python -m unittest discover -s tests -p 'test*.py' -v
nix develop -c cargo test --test whitepaper_validation_suite
```

The aggregate Python discovery must remain scoped to root `tests/`; `gymnasium/` owns a separate Python project and dependency environment.

## Dependencies and overlap

- `plans/cull-moore-gyroscopic-source.md` owns Moore’s incorrect gyroscopic claim, reversible archive/cull mechanics, and its exact disputed/excluded manifest record. Coordinate merge order so the manifest inventory validates the archived path without duplicating that plan’s evidence or decisions.
- `physics_audits/2026-04-24-cue-cloth-motion.md:105-111` documents the runtime omission of squirt. This plan only prevents Petit C27 from being retrieved as complete cue-squirt authority; a future runtime cue model remains separate.
- `TODO.org:134-139` is existing citation-path cleanup context, not a source of authority. It should be removed when the canonical migration is complete.
- Other physics plans may cite or regenerate `agent_knowledge/*`; they should depend on this manifest contract for source metadata and must not hand-edit generated artifacts. Their runtime equations/tests remain in their own plans.

## Candidate commit message

```text
Establish reviewed whitepaper authority manifest

Canonicalize source citations, scope Petit C27, exclude off-domain retrieval records, and quarantine unreadable scans while deriving agent knowledge deterministically.
```
