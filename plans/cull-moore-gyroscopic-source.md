# Quarantine Moore’s invalid homogeneous-sphere gyroscopic source

Date: 2026-07-10

Severity: **Medium today; High if adopted as physics authority**

Priority: **P1 corpus-integrity fix**

## Problem statement

`whitepapers/mechanics_of_billiards_and_analysis_of_willie_hoppe_s_stroke.pdf`, A. D. Moore’s 1942 manuscript with 1947 additional notes, must no longer be available as implementation authority. Its section “The Ball as a Gyroscope” treats a homogeneous billiard ball as though an applied contact torque produced a further, right-angle gyroscopic-precession response. For a homogeneous sphere the inertia tensor is isotropic, so that mechanism is materially wrong:

\[
\mathbf I = I_s\mathbf 1,
\qquad
I_s = \frac{2}{5}mR^2,
\qquad
\boldsymbol\tau
 = \frac{d\mathbf L}{dt}
 = I_s\dot{\boldsymbol\omega}.
\]

Equivalently, the body-frame Euler equation is

\[
\boldsymbol\tau
 = \mathbf I\dot{\boldsymbol\omega}
   + \boldsymbol\omega\times(\mathbf I\boldsymbol\omega),
\]

but `Iω` is parallel to `ω` for an isotropic sphere, so

\[
\boldsymbol\omega\times(\mathbf I\boldsymbol\omega)=\mathbf 0,
\qquad
\dot{\boldsymbol\omega}=\frac{\boldsymbol\tau}{I_s}.
\]

The angular-velocity change is along the applied torque, not rotated another 90 degrees from it. Moore uses the incorrect direction as a causal explanation of cloth motion, cushion response, and path curvature, so a narrow “do not use one coefficient” caveat is insufficient. The source should be retained only as warned historical material and physically separated from the indexed implementation-authority corpus.

## Current behavior and impact

### Observed facts at plan authoring time

- The active source is the 978,891-byte PDF at `whitepapers/mechanics_of_billiards_and_analysis_of_willie_hoppe_s_stroke.pdf` with SHA-256 `17ee449d8fe661f7d513fbda505ff3689cad28c7c93f28d6526b356048fb95f6`.
- `scripts/build_agent_knowledge.py::iter_docs` scans supported **top-level** files from `WHITEPAPERS_DIR` and skips directories. `main` turns every returned source into a `DocRecord`, formula section, corpus section, and topic-map entry. There is currently no reviewed reliability or disputed-claim field in `DocRecord`.
- The generated record at `agent_knowledge/whitepapers_index.jsonl:165` marks the source `primary_start:false`, `cited_by_repo:false`, `cited_in_code:false`, and `cited_in_docs:false`, but still assigns four broad topics: `collisions_and_impacts`, `cue_ball_motion_and_spin`, `strategy_rules_drills`, and `history_and_general_physics`.
- The complete bad text is retrievable from `agent_knowledge/whitepapers_corpus.txt`; seven noisy formula-like candidates are exposed at `agent_knowledge/whitepapers_formula_candidates.txt:1013-1021`; and the source is visibly ranked in the generated History and General Physics list at `agent_knowledge/agent_reading_guide.md:145`.
- The exact source-title/path search found no current Rust, test, hand-written user documentation, DSL, or runtime configuration citation. Before this plan existed, all exact matches were confined to the generated index, corpus, formula candidates, and reading guide. The source is also absent from `scripts/build_agent_knowledge.py::PRIMARY_STARTER_DOCS`.
- `src/lib.rs::raw_advance_within_phase_on_table`, reached through public `advance_within_phase_on_table` and `advance_motion_on_table`, already updates horizontal angular velocity from cloth impulse in the torque-consistent direction. The cull therefore removes future authority risk; it does not repair an observed Moore-style runtime update.
- `scripts/build_agent_knowledge.py::gather_repo_citations` scans Markdown under `plans/`. Consequently, this plan’s necessary historical reference would temporarily mark the PDF doc-cited if the builder were run while the PDF remained active at top level. Implementation must archive the PDF **before** regenerating.

### Impact

There is no current runtime result, public API, serialized state, or current implementation citation to migrate. No Rust caller presently depends on this PDF. The current severity is therefore Medium.

The latent impact is High: an agent researching sidespin, swerve, spin-axis evolution, or rail response can retrieve Moore from the machine index/corpus/formula surface and use a wrong torque direction to design new runtime physics. Moore applies the mechanism across several phenomena rather than presenting it as an isolated aside. Physical quarantine is needed even though the current code happens not to use it.

## Whitepaper evidence

All corpus ranges below identify the pre-quarantine generated corpus. Regeneration will intentionally remove Moore’s document and shift later line offsets; this plan and the byte-identical archived PDF preserve the audit trail.

### Exact bad claims in Moore

Source: `whitepapers/mechanics_of_billiards_and_analysis_of_willie_hoppe_s_stroke.pdf`, section “The Ball as a Gyroscope”; extracted document begins at `agent_knowledge/whitepapers_corpus.txt:32330-32344`.

- `agent_knowledge/whitepapers_corpus.txt:32976-32987` says a spinning billiard ball is a gyroscope, that a force tending to shift its axis is resisted, and that the axis shifts “at right angles to the force tendency.” This is the incorrect causal-direction rule.
- `agent_knowledge/whitepapers_corpus.txt:32992-32998` invokes two successive “precessions” to explain the initial axis tilt and lean under cloth friction.
- `agent_knowledge/whitepapers_corpus.txt:33025-33029` says the same friction force slows translation, increases spin, and increases axis tilt by gyroscopic precession.
- `agent_knowledge/whitepapers_corpus.txt:33103-33108` uses rapid precession to explain the axis leaving a cushion.
- `agent_knowledge/whitepapers_corpus.txt:33137-33143` uses the alleged precession to increase backward lean and path curvature.

This is materially contradictory physics, not merely obsolete vocabulary: the claimed right-angle response selects the direction of the state update and is reused for cloth, cushion, and curved-path predictions.

### Contradicting rigid-sphere equations already in the corpus

Source: `whitepapers/the_art_of_billiards_play.html`, extracted mechanics sections 7.3 and 7.5.

- `agent_knowledge/whitepapers_corpus.txt:51030-51035` gives the governing laws

  \[
  M\frac{d\mathbf W}{dt}=\mathbf F-Mg\hat{\mathbf z},
  \qquad
  I\frac{d\boldsymbol\Omega}{dt}=\overrightarrow{GE}\times\mathbf F+\mathbf K.
  \]

- `agent_knowledge/whitepapers_corpus.txt:51039-51064` applies the contact force and torque directly to sliding and rolling angular-velocity evolution; it contains no extra right-angle precession term.
- `agent_knowledge/whitepapers_corpus.txt:51225-51235` repeats the torque equation, states `I = (2/5)MR²`, and derives the vertical-spin torque.
- `agent_knowledge/whitepapers_corpus.txt:51235-51240` explicitly concludes that `dΩ_vertical/dt` is parallel/antiparallel to `Ω_vertical` and integrates that direct torque response.

These corpus equations agree with the isotropic rigid-body derivation above. They are corroborating mechanics evidence, not a request to elevate every claim in that HTML source.

### Contradicting experiment and constraint kinematics

Source: `whitepapers/rolling_motion_of_a_ball_spinning_about_a_near_vertical_axis.pdf`.

- `agent_knowledge/whitepapers_corpus.txt:42591-42617` describes near-vertical-axis motion and a 300-frames/s experiment rather than assuming Moore’s precession mechanism.
- `agent_knowledge/whitepapers_corpus.txt:42631-42638` identifies the contact radius `r` and rolling constraint `v = rω`.
- `agent_knowledge/whitepapers_corpus.txt:42663-42674` reports measured linear and angular decay: for the illustrated rolling case, `a = 0.50 ± 0.01 m/s²`, `μ = 0.051 ± 0.001`, and `α = 54 ± 1 rad/s²`, with comparison measurements for ordinary rolling, sliding, and spinning.
- `agent_knowledge/whitepapers_corpus.txt:42731-42735` explains that the spin axis slowly tilts to maintain rolling after a short initial sliding phase.
- `agent_knowledge/whitepapers_corpus.txt:42758-42780` reports a second measured case and gives

  \[
  \cos\theta=\frac rR=\frac{v}{\omega R}.
  \]

The observed axis evolution follows changing velocity, angular speed, contact constraint, and torque. It is not evidence for an additional response perpendicular to the applied torque.

This experiment used a 22 mm-radius golf ball on low-pile carpet and measured `μ≈0.05` in the rolling cases. It is qualitative evidence against Moore’s mechanism, not authority for pool-ball or pool-cloth coefficient calibration.

## Reproducible torque counterexample

Use a standard homogeneous pool ball and an intentionally simple instantaneous cloth force:

- mass `m = 0.170 kg`;
- radius `R = 0.028575 m`;
- initial angular velocity `ω₀ = (0, 0, 100) rad/s`;
- gravity `g = 9.81 m/s²`;
- sliding coefficient `μ = 0.20`;
- bottom-contact lever arm `r = (0, 0, -R) m`;
- instantaneous friction force `F = (μmg, 0, 0) = (0.33354, 0, 0) N`.

The scalar sphere inertia is

\[
I_s=\frac25mR^2
   =5.55240825\times10^{-5}\ \mathrm{kg\,m^2}.
\]

The torque is

\[
\boldsymbol\tau
 = \mathbf r\times\mathbf F
 = (0,-R\mu mg,0)
 = (0,-9.5309055\times10^{-3},0)\ \mathrm{N\,m}.
\]

Therefore

\[
\boldsymbol\alpha
 = \frac{\boldsymbol\tau}{I_s}
 = (0,-171.6535433,0)\ \mathrm{rad/s^2}.
\]

After `Δt = 0.010 s`, while holding the force fixed for this directional counterexample,

\[
\Delta\boldsymbol\omega
 = \boldsymbol\alpha\Delta t
 = (0,-1.716535433,0)\ \mathrm{rad/s}.
\]

The observable invariants are

\[
\Delta\boldsymbol\omega\times\boldsymbol\tau=\mathbf0,
\qquad
\Delta\boldsymbol\omega\cdot\boldsymbol\tau>0,
\qquad
\Delta\omega_x=0.
\]

The initial `100 rad/s` vertical spin does not create an `x` response because the sphere’s inertia is isotropic. Moore’s further right-angle response would require an unsupported `x` component and would fail these invariants. The force is a local counterexample to the alleged direction rule; it is not offered as a complete finite-duration cloth model.

## Root cause

There are two layers:

1. **Source physics:** Moore conflates a free homogeneous sphere with the familiar precession of a constrained gyroscope/asymmetric rotor. For a sphere, `Iω` is collinear with `ω`; the Euler coupling term vanishes. Moore then propagates that conceptual error into cloth, cushion, and curvature explanations.
2. **Corpus selection:** `classify_topics` uses title, filename, and the first 4,000 extracted characters with broad keywords, while `render_guide` ranks only starter state, apparent repo citation, formula-line count, and title. No current field can say “historically interesting but disputed and forbidden as implementation authority.” Thus an uncited non-starter still enters the machine manifest, full corpus, formula surface, and a visible topic list.

The source-level fix is quarantine. A general reviewed authority and citation-role model belongs to the sibling plan `plans/whitepaper-authority-manifest.md`, not to this plan.

## Archive-versus-cull decision

### Decision: reversible archival quarantine

Move the PDF byte-for-byte to:

`whitepapers/_archive/disputed/mechanics_of_billiards_and_analysis_of_willie_hoppe_s_stroke.pdf`

This is chosen over hard deletion because the manuscript retains historical value, the bad claim must remain auditable, and the original evidence should be reproducible. The current `iter_docs` top-level-only contract skips nested directories, so this location removes the PDF from all generated authority surfaces without destroying it. Preserve the current size and SHA-256 after the move.

Add `whitepapers/_archive/disputed/README.md` with a concise, unequivocal warning: the Moore PDF is historical/disputed, must not be used for implementation or calibration, and is quarantined because its homogeneous-sphere gyroscopic-precession mechanism contradicts `τ = I dω/dt`. Point the note to this plan for the detailed evidence. The warning is not a compatibility alias and must not cause the archived file to re-enter retrieval.

Do **not** leave the PDF at top level with only prose metadata: the current builder has no exclusion field and would still extract and rank it. Do **not** add a top-level symlink or copied replacement. A hard cull is the fallback only if repository licensing or retention policy prohibits archival storage; in that case delete the active file, retain its hash and evidence in this plan, and keep every generated/test invariant below unchanged. Hard cull is not the default because version-control history alone is less discoverable for future audits.

When `plans/whitepaper-authority-manifest.md` is implemented, its `whitepapers/authority_manifest.json` must be the sole machine-reviewed metadata source. Add the exact archived path with disputed authority and `retrieval: exclude` under that plan’s schema; validate that the archived file exists and cannot appear in generated retrieval. Do not create a competing one-off JSON/TOML authority schema here.

## Exact affected files, symbols, and callers

### Existing artifacts and symbols

| File or symbol | Observed role | Planned treatment |
| --- | --- | --- |
| `whitepapers/mechanics_of_billiards_and_analysis_of_willie_hoppe_s_stroke.pdf` | Active top-level raw source | Move byte-identically to `_archive/disputed/`; no active alias or copy. |
| `scripts/build_agent_knowledge.py::WHITEPAPERS_DIR` | Active corpus root | No API change. |
| `scripts/build_agent_knowledge.py::iter_docs` | Enumerates supported top-level files and skips directories | Keep behavior; add a regression test that the archive cannot enter its result. |
| `scripts/build_agent_knowledge.py::DocRecord` | Current generated record model lacks reliability state | Do not extend locally; the sibling authority-manifest plan owns the general data model. |
| `scripts/build_agent_knowledge.py::gather_repo_citations` | Scans repo prose, including this plan | Do not generalize here; archive before regeneration so this plan cannot elevate an active Moore record. Citation-role cleanup belongs to the sibling manifest plan. |
| `scripts/build_agent_knowledge.py::main` | Extracts every `iter_docs` source and writes five generated artifacts | Regenerate after the move; no hand edits. |
| `scripts/build_agent_knowledge.py::render_guide` | Ranks topic entries | Receives no Moore `DocRecord` after quarantine. |
| `src/lib.rs::raw_advance_within_phase_on_table` | Existing private cloth-motion update | No behavior change; protect its torque-consistent observable through the public motion API. |
| `advance_within_phase_on_table` / `advance_motion_on_table` | Public callers of the private update | Add only a focused regression test; no callsite migration. |
| `agent_knowledge/whitepapers_index.jsonl:165` | Active Moore machine record | Removed by regeneration. |
| `agent_knowledge/whitepapers_formula_candidates.txt:1013-1021` | Moore formula-candidate section | Removed by regeneration. |
| `agent_knowledge/whitepapers_corpus.txt:32330-33472` | Moore delimiter, metadata, and extracted text | Entire document section removed by regeneration. |
| `agent_knowledge/agent_reading_guide.md:145` | Visible History and General Physics entry | Removed by regeneration; ranking/counts reconcile. |
| `agent_knowledge/README.md` | Generated active-document count | Regenerated from 427 to 426. |
| `AGENTS.md` | Human/agent instructions for using generated knowledge | After smoke verification, document that `_archive/disputed` is historical only and never implementation authority. |

The `agent_knowledge/*` line ranges are current evidence locations, not stable post-regeneration locations.

### New test and archive files

- `tests/test_agent_knowledge_builder.py`: focused standard-library `unittest` coverage for the active/archive boundary, generated absence, and one-to-one artifact counts.
- `whitepapers/_archive/disputed/README.md`: human warning at the point where the retained raw source is discoverable.
- `whitepapers/_archive/disputed/mechanics_of_billiards_and_analysis_of_willie_hoppe_s_stroke.pdf`: byte-identical archived source.

## API and data-model cutover

There is **no Rust API cutover**. No public type, DSL field, configuration value, serialized format, runtime symbol, or simulation callsite changes. Existing generated-artifact consumers continue using the same five paths and schemas.

The corpus data cutover is clean:

```text
active top-level PDF
  -> DocRecord + formula section + corpus section + topic ranking

becomes

archived disputed PDF
  -> warning/provenance only
  -> no active DocRecord
  -> no formula section
  -> no corpus section
  -> no topic ranking
```

Do not retain a deprecated record, retrieval alias, symlink, or hidden formula/corpus fragment. Current callers of generated artifacts need no migration because their file formats remain unchanged; they observe one fewer active document. If the reviewed authority manifest already exists at implementation time, migrate the source directly from its active manifest entry to the archived/excluded entry in the same commit and use that manifest’s validator. If it does not yet exist, the nested archive plus regression test is the complete current cutover, and `plans/whitepaper-authority-manifest.md` later adopts the archived path without moving it again.

## Explicit non-goals

- Do not change current sidespin, swerve, rolling, cushion, collision, or spin-decay runtime equations as part of this source cull.
- Do not claim every historical observation, photograph, or stroke discussion in Moore is false. The whole document is quarantined as implementation authority because a materially wrong mechanism is reused across multiple physics sections; historical preservation is intentional.
- Do not calibrate pool-cloth coefficients from the golf-ball/low-pile-carpet experiment.
- Do not cull or reclassify `whitepapers/the_art_of_billiards_play.html` in this plan. Its separately scoped cue-impact limitation and useful cloth/collision material require claim-level treatment elsewhere.
- Do not redesign topic classification, citation scanning, starter ordering, extraction quality, or general reliability ranking. Those belong to `plans/whitepaper-authority-manifest.md`.
- Do not clean up unrelated off-domain or misclassified whitepapers.
- Do not hand-edit generated `agent_knowledge/*` files.
- Do not add a one-off authority metadata format parallel to the reviewed manifest.

## Phased implementation

### Phase 1 — Add observable regression contracts first

1. Add `tests/test_agent_knowledge_builder.py` using Python’s standard-library `unittest` and import `scripts.build_agent_knowledge`.
2. Add `test_disputed_moore_source_is_quarantined_from_authority_inputs`:
   - assert the original top-level path does not exist;
   - assert the exact archived path exists;
   - assert its size is 978,891 bytes and its SHA-256 is `17ee449d8fe661f7d513fbda505ff3689cad28c7c93f28d6526b356048fb95f6`;
   - assert neither the original nor archived path is returned by `iter_docs`;
   - assert the basename is absent from `PRIMARY_STARTER_DOCS`;
   - if `whitepapers/authority_manifest.json` exists under the sibling plan’s schema, assert the path is disputed/excluded and not active.
3. Add `test_generated_document_surfaces_match_active_inventory`:
   - parse every JSONL record;
   - assert record count equals `len(list(iter_docs()))` and record paths are unique;
   - assert the Moore basename/path/title is absent from index, corpus, formula candidates, and reading guide;
   - assert corpus BEGIN and END delimiter counts both equal the index count;
   - assert the number of formula `path: whitepapers/...` section headers equals the index count;
   - assert generated README and reading-guide indexed counts equal the index count;
   - assert the guide’s cited count equals the number of JSONL records with `cited_by_repo:true`.
4. Add `homogeneous_sphere_cloth_torque_changes_omega_along_torque` to `tests/advance_ball_state.rs`, adjacent to existing sliding angular-update tests:
   - construct an on-table state with `ω=(0,0,100) rad/s` and translational slip chosen so cloth friction points along `+x`;
   - configure sliding acceleration to `0.20g`, zero vertical-spin decay for the isolated interval, and advance `0.010 s` without crossing the sliding boundary;
   - exercise public `advance_within_phase_on_table` or `advance_motion_on_table`, not the private raw helper;
   - assert `Δω=(0,-1.716535433...,0) rad/s` within numeric tolerance after SI/inch conversion;
   - assert `Δω×τ=0`, `Δω·τ>0`, and `Δω_x=0` within tolerance. A Moore-style perpendicular component must fail.
5. Run the torque test before moving the source; it should pass and prove current runtime behavior is already correct. Run the builder tests before the move/regeneration; the quarantine/generated-absence assertions should fail for the intended reasons.

### Phase 2 — Perform the reversible authority cutover

1. Create `whitepapers/_archive/disputed/`.
2. Move, rather than copy, the exact PDF from the active top-level path to the chosen archive path. Verify size and SHA-256 immediately.
3. Add the archive warning README described above. It must say historical/disputed, not implementation or calibration authority, and link to this plan’s mechanics/evidence.
4. Do not leave a symlink, alias, duplicate PDF, extracted text copy, or top-level warning stub with a supported document extension.
5. If `whitepapers/authority_manifest.json` already exists, add/update the one Moore record using its canonical schema: archived path, disputed authority, `retrieval: exclude`, no starter position, no allowed implementation scope, and an excluded gyroscopic-precession claim. Do not modify the generic schema in this plan.
6. Run the focused Python quarantine test. At this point the archive assertions pass while generated-absence/count assertions intentionally remain red until regeneration.

### Phase 3 — Regenerate and reconcile every derived artifact

Run the supported generator once after the source is no longer active:

```sh
nix develop -c python scripts/build_agent_knowledge.py
```

This must rewrite, from source rather than by hand:

- `agent_knowledge/README.md`;
- `agent_knowledge/whitepapers_index.jsonl`;
- `agent_knowledge/whitepapers_formula_candidates.txt`;
- `agent_knowledge/whitepapers_corpus.txt`;
- `agent_knowledge/agent_reading_guide.md`.

For an otherwise unchanged corpus, reconcile the exact before/after values:

| Observable | Before | After |
| --- | ---: | ---: |
| Active indexed documents | 427 | 426 |
| Cited active records | 20 | 20 |
| Extracted characters | 5,765,368 | 5,697,603 |
| Formula-like lines | 2,724 | 2,717 |
| JSONL records | 427 | 426 |
| Formula document sections | 427 | 426 |
| Corpus BEGIN delimiters | 427 | 426 |
| Corpus END delimiters | 427 | 426 |
| `collisions_and_impacts` records | 237 | 236 |
| `cue_ball_motion_and_spin` records | 356 | 355 |
| `strategy_rules_drills` records | 316 | 315 |
| `history_and_general_physics` records | 28 | 27 |

The exact deltas come from Moore’s current record: 67,765 extracted characters, 1,136 extracted lines, seven formula-like candidates, four topics, and no cited/starter flags. The reading-guide History and General Physics entry must disappear. For its four affected guide topics, the displayed “more” totals should reconcile with the one-record reduction; no Moore entry may survive in a different section.

The plan itself contains the old active path and is scanned as prose by the current citation gatherer. Because there is no longer an active `DocRecord` to receive that basename, this historical plan citation must not increase the generated cited-record count. If unrelated corpus or authority-manifest work is combined in the same branch, do not blindly preserve the absolute table values: first account for those explicit changes, then require Moore’s isolated deltas (`-1` record, `-67,765` characters, `-7` formula lines, `-1` in each listed topic, and `0` cited active records) and all one-to-one invariants.

Run the generator a second time from unchanged inputs and verify all five generated files are byte-identical to the first run.

### Phase 4 — Cleanup and agent-facing guidance after the smoke test passes

1. Add a short `AGENTS.md` statement that `whitepapers/_archive/disputed/` contains historical material excluded from implementation authority and must not be indexed or cited to justify runtime physics.
2. Keep the detailed physics rationale in this plan and the short warning beside the archive; do not copy the derivation into generated artifacts.
3. When the sibling manifest is present, ensure its Moore record is the sole machine-readable authority classification and its generic tests cover excluded archived records. Retain this plan’s source-specific path/hash and torque regression.
4. Confirm no generated artifact was edited after the last successful generator run.

## Regression and acceptance tests

### Authority-corpus invariants

- There is exactly one retained Moore PDF, only at the archived path, with the original byte size and SHA-256.
- Neither the active path nor archived path is returned by `iter_docs`.
- No active JSONL record, corpus delimiter section, formula section, guide entry, starter entry, or topic ranking contains Moore’s filename or title.
- Index records, formula sections, and corpus BEGIN/END delimiters are one-to-one with active `iter_docs` inputs.
- Generated README/guide counts agree with the machine index.
- In the isolated change, active documents are 426, active cited records remain 20, and formula-like lines are 2,717.
- A future recursive scan or canonical-path alias must fail tests if it reintroduces the archived file.

### Mechanics invariants

For the numeric example:

- `Δω_y = -1.716535433... rad/s` after 10 ms;
- `Δω_x = 0` and isolated `Δω_z = 0` within tolerance;
- `||Δω×τ||` is zero within tolerance;
- `Δω·τ > 0`;
- the public motion advance remains the path under test.

These assertions defend the observable torque direction. They do not merely search source text or assert an implementation detail.

### No-runtime-impact invariant

Run the existing focused motion suite before and after the archive/regeneration. Apart from the new regression test, runtime test results and public interfaces must be unchanged. There is no Rust citation or callsite removal to perform. Any runtime output change is out of scope and must be investigated rather than accepted as a consequence of the source move.

## Risks and edge cases

- **Future recursive indexing:** `_archive/disputed` is safe because current `iter_docs` is top-level-only, but a future recursive refactor could ingest it. The explicit archive-boundary test and eventual manifest exclusion are both required.
- **Plan citation promotion:** running the builder before the move can make the currently active PDF appear doc-cited because `gather_repo_citations` scans this plan. Preserve the required ordering: test baseline, move, then regenerate.
- **Raw filesystem discovery:** agents can still find an archived PDF with a broad file glob. The directory name, adjacent warning, `AGENTS.md` rule, and eventual manifest exclusion must all agree that it is historical only.
- **Generated line drift:** removing 1,136 extracted lines shifts later corpus evidence. Treat the pre-cull line ranges in this plan as audit evidence, not stable generated identifiers. The archive hash is the durable source identity.
- **Concurrent corpus edits:** exact absolute counts are valid for this isolated cull. When combined with another source change, reconcile explicit deltas and one-to-one invariants rather than weakening tests.
- **Coordinate signs:** the mechanics test must derive torque from `r×F` in the same right-handed frame as the engine. Prefer cross/dot invariants plus the analytic vector over a prose “left/right” assertion.
- **Spin decay contamination:** isolate the 10 ms horizontal-torque test by configuring zero vertical-spin decay; otherwise an unrelated `Δω_z` makes the full-vector cross product nonzero.
- **Experimental domain mismatch:** the near-vertical-axis experiment is on a golf ball and carpet. Cite it for qualitative constraint/axis behavior only.
- **Historical overreach:** quarantine means “not implementation authority,” not “all observations are worthless.” Preserve the file and warning so later historians can inspect it without silently feeding it to physics retrieval.
- **Manifest sequencing:** this cull can land before `plans/whitepaper-authority-manifest.md`; do not block the physical quarantine on the broader architecture. If the manifest lands first, integrate with it rather than adding parallel metadata.

## Verification commands

Run focused checks first:

```sh
nix develop -c cargo test --test advance_ball_state homogeneous_sphere_cloth_torque_changes_omega_along_torque
nix develop -c python -m unittest discover -s tests -p 'test_agent_knowledge_builder.py'
```

Regenerate from the quarantined source layout and inspect the generator’s focused counts:

```sh
nix develop -c python scripts/build_agent_knowledge.py
```

Expected isolated output includes:

```text
indexed_docs=426
cited_docs=20
formula_lines=2717
topic[collisions_and_impacts]=236
topic[cue_ball_motion_and_spin]=355
topic[strategy_rules_drills]=315
topic[history_and_general_physics]=27
```

Run the builder tests again, verify the active-record line count, archive hash, and exact absence from the four lookup artifacts:

```sh
nix develop -c python -m unittest discover -s tests -p 'test_agent_knowledge_builder.py'
wc -l agent_knowledge/whitepapers_index.jsonl
shasum -a 256 whitepapers/_archive/disputed/mechanics_of_billiards_and_analysis_of_willie_hoppe_s_stroke.pdf
grep -R -n 'mechanics_of_billiards_and_analysis_of_willie_hoppe_s_stroke\|MECHANICS OF BILLIARDS, AND ANALYSIS OF WILLIE HOPPE' \
  agent_knowledge/whitepapers_index.jsonl \
  agent_knowledge/whitepapers_formula_candidates.txt \
  agent_knowledge/whitepapers_corpus.txt \
  agent_knowledge/agent_reading_guide.md
```

`wc -l` must report `426`; the hash must match the value above; and the final exact-absence search must return no matches.

Then run the relevant aggregate suites:

```sh
nix develop -c cargo test --test advance_ball_state
nix develop -c cargo test --test whitepaper_validation_suite
nix develop -c cargo test
```

Finally, run `nix develop -c python scripts/build_agent_knowledge.py` a second time and compare SHA-256 checksums of all five `agent_knowledge/*` outputs with the first regenerated set. They must be byte-identical.

## Dependencies and overlap

- **`plans/whitepaper-authority-manifest.md` — “Establish a reviewed whitepaper authority manifest.”** That sibling plan owns the general `whitepapers/authority_manifest.json` schema, inventory validation, reliability/retrieval fields, starter ordering, alias resolution, and citation scan roles. This plan owns Moore’s exact physics evidence, archive move, disputed/excluded record, generated deltas, and torque regression. Either order is supported; do not duplicate the manifest schema here.
- No existing physics audit or plan currently owns Moore’s whole-document quarantine. The scoped limitation in `physics_audits/2026-04-24-cue-cloth-motion.md` concerns a different source/claim and is not part of this cull.
- Future sidespin, swerve, rail, or spin-axis work depends on this quarantine only in the negative sense: it must not cite Moore as implementation authority.

## Candidate commit message

```text
Quarantine Moore's invalid gyroscopic billiards source

Archive the disputed 1942 PDF outside active retrieval, regenerate the
agent-knowledge corpus, and lock the homogeneous-sphere torque direction
and generated inventory invariants.
```
