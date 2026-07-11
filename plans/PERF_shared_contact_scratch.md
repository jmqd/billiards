# Replace shared-contact heap churn with reusable contiguous scratch

Date: 2026-07-11

Status: **Accepted candidate; implementation not started**
Priority: **P1**
Confidence: **High for source-visible allocation removal; medium for the public latency threshold**
Dependencies/order: **Independent.** The committed `shared_contact_resolution/symmetric_3_ball_2_contact` fixture supplies the common zero-time direct baseline. Add the missing larger inline/fallback/control cells and stronger pre-timing output validation before production edits. It may land before or after collision-root storage. No public API migration is required.

## Objective

Remove nested and per-pivot heap allocation from coupled shared ball-ball contact resolution. Use one row-major elimination matrix and a reusable private scratch object whose common contact graphs stay in inline storage, while preserving an unbounded heap fallback for larger graphs.

This is a storage and ownership change only. The collision equations, pivoting policy, active-set policy, thresholds, contact discovery, event selection, and output state semantics must not change.

## Current mechanism and evidence

### Source facts

The current implementation is in `src/lib.rs`:

- `solve_linear_system` (`src/lib.rs:10251-10305`) accepts `Vec<Vec<f64>>` and `Vec<f64>` by value. An $n\times n$ matrix therefore owns one allocation for the outer vector and one allocation for every row.
- At every pivot, `solve_linear_system` copies `matrix[pivot_col][pivot_col..]` with `.to_vec()` (`src/lib.rs:10282`). This adds as many short-lived heap allocations as there are pivots.
- Pivot selection scans rows in increasing index order and replaces the pivot only on strict `candidate_abs > pivot_abs` (`src/lib.rs:10257-10266`). Singular rejection is `pivot_abs <= 1e-12`; elimination skips factors with `abs() <= 1e-15` (`src/lib.rs:10268-10301`). Those details are part of the numerical contract.
- `coupled_normal_shared_ball_ball_contact_deltas_from_state_refs` (`src/lib.rs:10307-10378`) separately allocates contacts, active indices, nested matrix rows, RHS, solver output, and a state-count-sized delta vector. Every active-set retry rebuilds the matrix and RHS (`src/lib.rs:10331-10364`).
- A negative solved impulse is eligible for removal below `-1e-12`; the most negative is selected using `total_cmp`, with iteration order breaking equal-value ties (`src/lib.rs:10356-10363`).
- `resolve_shared_zero_time_ball_ball_contacts_on_table` (`src/lib.rs:10395-10440`) also allocates `state_refs`, collects and sorts/deduplicates pairs, then clones every state that receives a nontrivial delta.
- The system-state counterpart (`src/lib.rs:10443-10498`) clones every on-table state into `snapshot`, builds another reference vector, builds pairs, solves, and then creates replacement states.
- `zero_time_ball_ball_pair_indices_from_state_refs` returns another `Vec<(usize, usize)>` (`src/lib.rs:9732-9774`), and `sorted_deduped_ball_ball_pairs` sorts and deduplicates a by-value `Vec` (`src/lib.rs:9776-9779`).

The common observable graph is small:

- `tests/n_ball_advance.rs:397-449` advances the symmetric three-ball/two-contact fixture and checks the `SharedBallBallContact` event, pair order `[(0, 1), (0, 2)]`, coupled-normal strategy, symmetry, and translational energy.
- `tests/n_ball_advance.rs:452-506` checks exact zero-friction nonideal shared-contact velocities for both `ThrowAware` and `SpinFriction` modes.
- `tests/n_ball_events.rs:307-339` checks the scheduler's shared-contact event time, indices, pair ordering, and resolution label.
- `tests/n_ball_events.rs:343-413` checks expansion from an opening collision into a frozen three-ball line and rejects a synthetic immediate follow-on collision.
- `tests/n_ball_pockets.rs:50-96` exercises the same coupled-normal limit through the public system-state/pocket-aware executor.

### Measured versus hypothesized evidence

The committed `shared_contact_resolution/symmetric_3_ball_2_contact` quick baseline is **60.38–63.22 us/call** on the current tree. The fixture prebuilds an already-touching three-ball/two-contact state, asserts the `SharedBallBallContact` variant and pairs `[(0, 1), (0, 2)]`, uses 40 samples and ten-second measurement time, and black-boxes the complete public advance. Its geometry makes the event zero-time, but the committed pre-timing assertion does not yet check the time, indices, resolution label, or output invariants. It proves that the coupled public path is now directly measurable.

Measured repository-wide evidence in `plans/performance_engineering.md` reports 405,780 calls through `alloc::raw_vec::finish_grow` on the profiled pipeline. That aggregate does **not** attribute all calls to shared contacts, and the quick direct timing does not attribute a fraction to allocation.

The shared-contact allocation count above is a source fact. Its timing impact remains a **hypothesis** until paired direct measurements and allocation attribution pass. The existing three-ball end-to-end pinball benchmark is dominated by pocket prediction and remains diagnostic only.

## Scope

### In scope

1. A private inline-or-heap contiguous buffer implemented in this crate, without adding a dependency.
2. A private `SharedContactSolveScratch` that owns reusable contact, pair, active-index, row-major matrix, RHS/impulse, and sparse-delta storage.
3. Row-major Gauss-Jordan elimination over borrowed slices; no `Vec<Vec<_>>` and no pivot-row clone.
4. Reuse of the same matrix/RHS capacity across active-set retries and, in multi-event simulation loops, across successive event resolutions.
5. Removing whole-state snapshots/reference vectors when direct immutable borrowing until the solve completes is sufficient.
6. Public-API Criterion fixtures, focused correctness coverage, differential numerical evidence, and allocation/profile gates.

### Non-goals

- Do not change restitution, collision normals, impulse equations, friction support, or coupled-normal applicability.
- Do not change `1e-12`, `1e-15`, `SHARED_BALL_BALL_CONTACT_STATE_EPSILON`, or `SIMULTANEOUS_EVENT_TOLERANCE_SECONDS`.
- Do not change pivot selection, elimination order, active-contact removal, pair sorting, event priority, event type, event time, or event tie semantics.
- Do not impose a maximum contact count. Inline capacity is an optimization, not a physics limit.
- Do not make private solver internals public for benchmarking.
- Do not combine this work with collision-root storage, scheduler redesign, overlap recovery, cache invalidation, or non-planar contact physics.
- Do not add `smallvec`, `arrayvec`, allocator crates, or any other production or development dependency.

## Proposed private data model

### Inline-or-heap buffer

Add one small private utility local to the shared-contact implementation, conceptually:

```rust
enum InlineOrHeap<T: Copy + Default, const N: usize> {
    Inline { values: [T; N], len: usize },
    Heap(Vec<T>),
}
```

Required operations are `clear`, `len`, `is_empty`, `push`, `remove`, `as_slice`, `as_mut_slice`, `resize_with_default`, `sort_unstable`, and `dedup`. The implementation must obey these invariants:

- Inline mode performs no heap allocation through length `N`.
- Pushing item `N + 1` moves the initialized prefix into one contiguous `Vec<T>` with initial capacity at least `max(2 * N, N + 1)`, then appends the new item. This single fallback allocation must cover the planned five-contact star's 10 transient pairs, five contacts/indices/RHS values, and 25 matrix values.
- Once a scratch buffer enters heap mode, `clear` retains that vector and its capacity so a later solve can reuse it. Do not automatically shrink back to inline mode.
- `as_slice` and `as_mut_slice` expose only initialized elements.
- Use `[T::default(); N]` with `T: Copy + Default`; do not introduce `unsafe`/`MaybeUninit` for a tiny internal optimization.
- Growth beyond the first fallback remains ordinary `Vec` growth, so arbitrary contact graphs remain supported.

Use named constants rather than scattered numbers:

```rust
const SHARED_CONTACT_INLINE_CONTACTS: usize = 4;
const SHARED_CONTACT_INLINE_PAIRS: usize = 2 * SHARED_CONTACT_INLINE_CONTACTS;
const SHARED_CONTACT_INLINE_PARTICIPANTS: usize = 6;
const SHARED_CONTACT_INLINE_MATRIX_VALUES: usize =
    SHARED_CONTACT_INLINE_CONTACTS * SHARED_CONTACT_INLINE_CONTACTS;
```

Four contacts cover the established three-ball/two-contact case and the proposed five-ball chain. Pair scratch needs eight entries because the current resolver first appends the event's four pairs and then appends the same four pairs rediscovered from touching states before `sort_unstable`/`dedup`; a four-entry pair buffer would allocate on the intended inline fixture. Six inline participants cover both. A five-contact, six-ball star transiently supplies up to ten pair entries and deliberately crosses the pair/contact/matrix inline limits, proving the fallback. If release assembly shows harmful spills or the repository's `large_stack_arrays` lint rejects this layout, reduce only the capacities after re-running the same fixture matrix; never remove the fallback or change equations to fit storage.

### Solver scratch

Add a private structure equivalent to:

```rust
struct SharedContactSolveScratch {
    pairs: InlineOrHeap<(usize, usize), SHARED_CONTACT_INLINE_PAIRS>,
    contacts: InlineOrHeap<SharedIdealBallBallContact, SHARED_CONTACT_INLINE_CONTACTS>,
    active_contact_indices: InlineOrHeap<usize, SHARED_CONTACT_INLINE_CONTACTS>,
    matrix: InlineOrHeap<f64, SHARED_CONTACT_INLINE_MATRIX_VALUES>,
    rhs: InlineOrHeap<f64, SHARED_CONTACT_INLINE_CONTACTS>,
    deltas: InlineOrHeap<IndexedOnTableKinematicDelta, SHARED_CONTACT_INLINE_PARTICIPANTS>,
}

#[derive(Clone, Copy, Default)]
struct IndexedOnTableKinematicDelta {
    ball_index: usize,
    delta: OnTableKinematicDelta,
}
```

`SharedIdealBallBallContact`, `OnTableKinematicDelta`, and the indexed delta can derive `Default` solely to support initialized inline arrays. Their mathematical fields and use remain unchanged.

Deltas should be sparse and stored in increasing ball-index order. Accumulate into an existing indexed entry or insert a new participant deterministically; do not allocate a `state_refs.len()`-sized vector when only contact participants can change.

## Implementation sequence and exact data flow

1. **Complete the benchmark matrix and save the unchanged baseline.** Keep the committed zero-time three-ball fixture. Add the larger inline, heap-fallback, and same-ball-count control cells below. Validate each event and complete output outside Criterion's timed closure.
2. **Introduce `InlineOrHeap` and `SharedContactSolveScratch`.** Keep them private in `src/lib.rs`; do not create a general collections module for one user.
3. **Flatten the linear solver.** Replace `solve_linear_system(Vec<Vec<f64>>, Vec<f64>) -> Option<Vec<f64>>` with a private in-place function of the form:

   ```rust
   fn solve_linear_system_in_place(
       matrix: &mut [f64],
       rhs: &mut [f64],
       n: usize,
   ) -> Option<()>;
   ```

   Require `matrix.len() == n * n` and `rhs.len() == n`. Index `(row, col)` as `row * n + col`. Row swaps must visit columns in increasing order and swap the corresponding RHS values. Normalize the pivot row from `pivot_col..n`, then eliminate rows `0..n` and columns `pivot_col..n` in exactly the current order. Read each pivot scalar into a local before mutating the other row; this avoids aliasing without cloning the row. Keep every comparison and threshold unchanged.
4. **Fill scratch in place.** Clear and refill contacts and active indices in input-pair order. On each active-set retry, resize the contiguous matrix to `active_len * active_len` and RHS to `active_len`, overwrite every used element, and call the in-place solver. RHS is the impulse vector after a successful solve; do not copy it into a second vector.
5. **Preserve active-set determinism.** Scan solved impulses in active-index order, filter with the current `< -1e-12`, and retain the exact current `.min_by(|(_, a), (_, b)| a.total_cmp(b))` winner—including its equal-impulse iterator tie behavior—before removing that active-list position. Do not use `swap_remove`; add an equal-negative-impulse test so a hand-written reduction cannot silently reverse the tie.
6. **Build sparse deltas.** Clear the scratch delta list, visit active contacts in their existing order, clamp with the current `max(0.0)`, and apply the same component operations. Emit/retain entries ordered by ball index so application order is deterministic.
7. **Move zero-time pairs into scratch.** Append caller-provided pairs and discovered zero-time pairs to `scratch.pairs`, then preserve the current tuple `sort_unstable` plus `dedup`. Contact order entering the matrix must remain the resulting ascending pair order.
8. **Remove avoidable state snapshots.** Contact discovery and solving are read-only. For the planar resolver, borrow directly from `&[OnTableBallState]` until sparse deltas are complete. For system states, read through `NBallSystemState::as_on_table` and skip pocketed entries; do not clone every on-table state merely to construct a reference vector. End all immutable borrows before applying deltas. Applying one delta may still construct the required new exact-unit state; that output work is not solver scratch.
9. **Thread scratch through private executors.** Add scratch parameters to the private scheduler/resolution path and migrate every in-tree caller in the same change. Public one-step functions such as `advance_to_next_n_ball_event_on_table` keep their signatures and construct a default scratch on the stack. Multi-event simulation functions construct one scratch before their event loop and reuse it for every resolution, retaining any heap fallback capacity. Do not expose scratch publicly and do not leave old internal wrapper paths.
10. **Delete the old nested solver and obsolete allocation paths.** No compatibility alias, dead `Vec<Vec<_>>` implementation, or production reference solver remains.

## Benchmark plan

### Existing committed fixture

Keep `shared_contact_resolution/symmetric_3_ball_2_contact` in `benches/physics.rs`. Its current geometry is the zero-time fixture from `zero_time_shared_contact_states`: the moving ball is already touching the two object balls at `y = -sqrt(3)R`, rather than starting 7.5 inches earlier as in `advancing_shared_simultaneous_contacts_transfers_motion_into_the_cluster`. The pre-timing event assertion currently checks the sorted pairs but not the time, indices, resolution label, symmetry, energy, or complete output.

Before using it as the primary gate, strengthen the setup validation outside `b.iter` to assert:

- event time is exactly zero;
- indices are `[0, 1, 2]`;
- pairs are `[(0, 1), (0, 2)]`;
- resolution is `coupled_normal`;
- all output states are finite and satisfy the existing symmetry/energy invariants.

Keep the current benchmark ID, 40-sample/ten-second group configuration, immutable prebuilt inputs, and complete `NBallOnTableAdvance` black-boxing. Do not rename it into a second `shared_contact/coupled_normal` alias. The quick range above is historical scale evidence only; acceptance uses a fresh saved paired baseline.

### Missing public-API matrix extensions

Add siblings in the existing `shared_contact_resolution` group:

1. `linear_5_ball_4_contact`
   - Place five balls at `(2Ri, 0)` for `i = 0..4`, exactly touching adjacent neighbors.
   - Give them decreasing positive $x$ velocities `[5, 4, 3, 2, 1]` in/s and matching rolling horizontal spin (`omega_y = vx / R`, other spin components zero). Every adjacent pair is closing at time zero; nonadjacent pairs are separated.
   - Assert a time-zero `SharedBallBallContact` with pairs `[(0,1), (1,2), (2,3), (3,4)]`.
   - This exercises the full proposed $4\times4$ inline matrix.

2. `branched_6_ball_5_contact_fallback`
   - Put one resting ball at the origin. Put five outer balls at radius `2R` and angles `2πi/5`, each moving radially inward at 1 in/s with no-slip horizontal spin (`wx = -vy/R`, `wy = vx/R`) and zero vertical spin.
   - Five equally spaced outer balls do not overlap because their adjacent chord is `4R sin(π/5) > 2R`.
   - Assert a time-zero shared event with exactly the sorted center/outer pairs and finite output states.
   - Five contacts exceed the proposed inline contact and 16-value matrix capacities, proving the unbounded contiguous fallback through a public API.

3. `single_pair_6_ball_control`
   - Use one time-zero touching/closing pair and four widely separated resting balls.
   - Assert an ordinary pair event and unchanged distant states.
   - This controls for public scheduler/output cost at the same ball count and is not expected to enter the coupled shared solver.

Use at least 40 Criterion samples and ten seconds measurement time per cell. Inputs are immutable and prebuilt; do not clone them inside `b.iter`. Black-box input references and the entire returned advance.

Keep existing collision predictor and three-ball end-to-end filters as secondary guardrails. The end-to-end case is diagnostic only because pocket prediction dominates it.

## Allocation gates

Allocation evidence is separate from Criterion timing. Use a dependency-free counting allocator in a dedicated benchmark/test binary, wrapping `std::alloc::System` with atomic allocation counters, or use macOS Instruments Allocations with stack attribution if the counting harness perturbs timings. Do not count fixture construction. Warm the executable once, reset counters, then execute 10,000 public advances from immutable prebuilt fixtures and black-box every output.

Required gates:

- `symmetric_3_ball_2_contact`: **zero** allocations attributed to matrix rows, pivot-row copies, contacts, active indices, RHS/impulses, or sparse solver deltas. Public event/result vectors and exact-unit allocations must be reported separately. Total allocations per public operation must be at least **50% lower** than the saved baseline.
- `linear_5_ball_4_contact`: same zero solver-scratch allocation requirement; the entire contact solve remains inline.
- `branched_6_ball_5_contact_fallback`: at most one growth allocation per scratch field that actually crosses its inline capacity on the first event; in a reused multi-event scratch, a second same-size resolution performs **zero additional scratch growth allocations**.
- No stack trace may remain under `solve_linear_system_in_place` for a pivot-row clone or a matrix row allocation.

If unrelated exact-unit/result allocations make the total 50% gate impossible but the named solver allocations reach zero, report both counts and apply the stop criteria below rather than weakening attribution.

## Correctness and numerical-equivalence verification

### Permanent tests

Retain all existing shared-contact tests and add cases that fail on plausible storage bugs:

1. The five-ball chain above asserts exact event pair order, finite outputs, no overlap, momentum conservation within the repository's existing collision tolerance, and deterministic equality across repeated calls.
2. The six-ball star asserts the fallback path, sorted contact order, all five outer symmetry relations after resolution, finite values, and no panic/growth limit.
3. A contact graph whose first active solve contains a negative impulse asserts that the same most-negative contact is removed and that surviving contacts remain in original order. This guards against `swap_remove`, reversed `total_cmp`, or a changed tie rule.
4. A singular contact matrix continues to return the same unsupported-contact error; fallback storage must not turn solver failure into zeros or NaNs.
5. A system-state fixture with pocketed entries confirms they are ignored, on-table indices remain original system indices, and only participating states change.
6. Repeat the common and fallback fixtures twice from identical inputs and compare event variants, indices, pairs, and every output position/velocity/angular-velocity component.

### Differential gate against the unchanged baseline

Before implementation, save complete public outputs for the four benchmark fixtures plus the existing symmetric, zero-friction nonideal, frozen-line, and pocket-aware fixtures. Record event discriminant, event time bits, ordered indices/pairs, resolution, and `f64::to_bits()` for every output position, velocity, vertical velocity, and angular-velocity component.

Because the proposed implementation preserves contact order and floating-point operation order, the candidate must be **bitwise identical** on the same host/toolchain. Existing tolerance-based conservation tests remain valuable but do not replace this differential gate. Any bit difference requires locating an operation-order change; do not waive it merely because values are close. Cross-toolchain/cross-architecture CI may continue using established semantic tolerances rather than checked-in platform-specific bit snapshots.

Run focused suites after implementation:

- `tests/n_ball_advance.rs`
- `tests/n_ball_events.rs`
- the shared-contact cases in `tests/n_ball_pockets.rs`
- the new scratch/solver unit tests

## Statistical acceptance

Use the same release profile, Rust toolchain, feature set, host, power mode, and Criterion configuration for baseline and candidate. Save the unchanged Criterion baseline. Run at least **three paired process-level comparisons**, interleaving baseline and candidate runs to reduce macOS thermal/frequency bias. Inspect outliers and rerun any discordant pair.

Use Criterion's bootstrap 95% confidence interval for the after/before time ratio:

- `shared_contact_resolution/symmetric_3_ball_2_contact`: upper bound must be `<= 0.80`.
- `linear_5_ball_4_contact`: upper bound must be `<= 0.85`.
- `branched_6_ball_5_contact_fallback`: upper bound must be `<= 0.85` on the one-step public fixture, or `<= 0.80` on a separately added public multi-event fixture that demonstrably reuses fallback capacity.
- `single_pair_6_ball_control`: no regression greater than 3%; equivalently, the ratio's upper confidence bound must be `<= 1.03`.
- Existing ball-ball predictor point/throughput filters: after/before ratio 95% CI upper bound `<= 1.03`.

Timing acceptance is necessary but not sufficient. Numerical equivalence and allocation gates must pass independently.

## Risks and mitigations

- **Changed rounding from row-major code.** Preserve loop nesting, pivot comparison, normalization order, and elimination order exactly; require bitwise differential outputs.
- **Changed tie behavior.** Do not use `>=`, `swap_remove`, unstable active-index reordering, or a different negative-impulse reduction.
- **Borrowing live states while mutating.** Finish all contact discovery and delta computation before the first mutation; keep sparse deltas in scratch across that boundary.
- **Stack pressure.** Inspect release assembly/frame size for public advance and solver functions. Reduce inline capacities only if measured spills/frame growth defeat the timing gate; keep the fallback.
- **Fallback accidentally becoming a limit.** Test five contacts with six balls and a larger private diagonally dominant solve; all growth paths remain `Vec` backed.
- **Benchmark dominated by scheduler/exact units.** Use allocation attribution and compare the inline chain/star scaling. Do not benchmark private solver functions as the primary evidence.
- **Persistent scratch retention.** Retaining a very large adversarial capacity is bounded by the largest graph seen by that simulation. Do not add shrink heuristics in this candidate; profile first.

## Stop criteria, rejection, and rollback

Reject or revert the implementation while keeping the benchmark/tests if any of these occurs:

1. Event kind, event time, pair/index order, active set, error behavior, or output bits differ from baseline.
2. Any fixed maximum replaces the arbitrary-size fallback, or the six-ball/five-contact public fallback case fails.
3. Common inline fixtures still allocate matrix rows, pivot copies, contacts, active indices, RHS, or solver deltas.
4. The primary Criterion confidence bounds do not meet the thresholds after three paired runs.
5. The single-pair control or existing collision predictor regresses by more than 3%.
6. Release assembly shows materially larger frames/spills and no public-API timing win.
7. The only way to win is to alter solver thresholds, operation order, restitution, or event semantics.

Rollback is deletion of the private scratch/flat-solver path and restoration of the old implementation; public APIs and serialized outputs do not change, so no compatibility layer or data migration is needed.

## arm64 SIMD and GPU decision

**Manual arm64 SIMD: reject for this candidate.** The common matrices are $2\times2$ through $4\times4`; pivot selection branches, active-set retries, short row tails, and scalar RHS updates dominate. NEON setup/masking would exceed the useful arithmetic, and changing reduction grouping risks observable rounding changes. Contiguous rows may allow LLVM to use paired loads/stores or limited auto-vectorization on a larger fallback matrix; that is welcome but not an acceptance criterion. Revisit explicit NEON only if profiles show frequent contact graphs with row widths of at least 8-16 and the scalar flat-buffer implementation remains dominant.

**GPU: reject.** A shared-contact solve is tiny, latency-sensitive, branchy, and embedded in a sequential event loop. Transfer, dispatch, synchronization, and divergent graph sizes would overwhelm the computation and complicate determinism. No GPU API or buffer ownership should be introduced.

## Candidate commit message

`perf(physics): reuse contiguous shared-contact solver scratch`
