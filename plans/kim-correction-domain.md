# Restrict the Kim table correction to its stationary-object domain

Date: 2026-07-10

Severity: **MEDIUM**  
Priority: **P2**

## Problem statement

The opt-in Kim object-ball/table static-friction correction is currently treated as a general coefficient on every non-ideal ball-ball collision. Kim's derivation instead assumes that the second ball is a static, non-spinning object ball whose displacement during the short impact is small enough for the cloth interaction to be approximated as static friction. Applying the same fixed line-of-centers impulse and horizontal-spin multiplier when that object ball is already translating or spinning is outside the source's validity domain and can put both the table impulse and its torque on the wrong axes.

The immediate fix is a conservative domain restriction, not a new support-contact model: preserve the ordinary ball-ball friction solve for every supported input, but apply the configured Kim correction only when the second input ball satisfies an explicit stationary-object predicate. A nonzero Kim coefficient on an out-of-domain input must produce a machine-readable diagnostic and exactly the same physical outcome as the corresponding configuration with object/table friction disabled.

## Current behavior and impact

### Observed implementation

- `BallBallCollisionConfig::with_object_table_static_friction_coefficient`, `with_kim_object_table_static_friction`, and `kim_table_coupled` (`src/lib.rs:1301-1314`) make the correction opt-in, but do not state or encode a stationary-object precondition.
- `frictional_collision_outcome_on_table_with_config` (`src/lib.rs:11658-11830`) accepts `require_stationary_object_ball`, yet the `ThrowAware` and `SpinFriction` wrappers both pass `false` (`src/lib.rs:11832-11848`). The only existing stationary guard is therefore unreachable in normal non-ideal collision response.
- Reusing that dormant guard as written would not be correct. At `src/lib.rs:11671-11672` it returns the ideal outcome for a moving second ball, which would also discard valid ball-ball restitution/friction/throw/spin behavior. Only the Kim table term must be suppressed.
- For any negative `vertical_impulse_per_mass`, the solver currently computes

  $$
  \delta v_{\mathrm{Kim}}=\tfrac12\mu_s j_z
  $$

  and adds that same scalar correction along the ball-ball normal to both balls (`src/lib.rs:11713-11745`). It also multiplies the object ball's horizontal spin increment by $1+\mu_s$ (`src/lib.rs:11748-11784`). Neither operation examines the object's initial translation, spin, or cloth-contact velocity.
- `CollisionDiagnostics` exposes only the numeric `kim_table_coupled_normal_correction` (`src/lib.rs:1363-1375`). A zero value does not distinguish disabled configuration, a source-domain rejection, and a stationary-domain collision for which the Kim step-function term is inactive.
- The detailed public entry points and tuple-returning wrappers all reach this solver (`src/lib.rs:11876-12001`). Event execution also reaches it through `collide_ball_ball_on_table_with_radius_and_config`, including shared-contact iterations and ordinary/disjoint scheduled collisions (`src/lib.rs:9773-9779`, `9900-9906`, `9995-10014`, and `10801-10820`). Those callers can supply an already-moving second ball.
- The existing stationary head-on contract, `kim_table_coupled_head_on_topspin_impact_recoils_cue_ball_and_reduces_object_speed` (`tests/non_ideal_ball_collisions.rs:1248-1306`), is valid for the source domain and must remain unchanged numerically.

### Impact

For a moving/sliding object, the named Kim approximation can create an external table impulse parallel to the ball-ball normal even when Coulomb cloth friction must initially oppose a lateral cloth-contact velocity. The associated extra torque is then placed about the ball-ball tangent instead of the axis determined by the actual table impulse. This changes total horizontal momentum and object spin in directions that the cited derivation does not support. Because the coefficient is opt-in, default configurations are unaffected, but users who deliberately select the supposedly source-grounded model receive misleading physics without any indication that the source assumptions were violated.

## Affected files, symbols, and callers

### Behavioral implementation

- `src/lib.rs:1172-1179`
  - `KIM_2024_OBJECT_TABLE_STATIC_FRICTION_COEFFICIENT`
  - Add a named stationary-object speed tolerance adjacent to the Kim constant rather than leaving the existing `1e-9` literal buried in the solver.
- `src/lib.rs:1241-1332`
  - `BallBallCollisionConfig`
  - `with_object_table_static_friction_coefficient`
  - `with_kim_object_table_static_friction`
  - `kim_table_coupled`
  - Clarify that the second collision operand is the object-ball role for this opt-in approximation and that out-of-domain inputs skip only the Kim correction.
- `src/lib.rs:1340-1376`
  - `CollisionOutcome`
  - `CollisionDiagnostics`
  - Add an explicit public Kim correction status and the pre-impact object speeds used by the domain predicate while retaining the numeric correction field.
- `src/lib.rs:11470-11479`
  - `validated_ball_ball_object_table_static_friction_coefficient`
  - Continue validating the configured coefficient before selecting a status.
- `src/lib.rs:11658-11830`
  - `frictional_collision_outcome_on_table_with_config`
  - Replace the dead whole-model `require_stationary_object_ball` branch with a Kim-specific stationary-domain predicate and effective coefficient.
- `src/lib.rs:11832-11848`
  - `throw_aware_collision_outcome_on_table_with_config`
  - `spin_friction_collision_outcome_on_table_with_config`
  - Remove the obsolete boolean argument; both models must use the same Kim-domain policy.
- `src/lib.rs:11850-11916`
  - Documentation and dispatch for `collide_ball_ball_detailed_on_table_with_config` and `collide_ball_ball_detailed_on_table_with_radius_and_config`.
  - Correct the Kim source path here.
- `src/lib.rs:11918-12001`
  - Detailed, tuple-returning, radius-explicit, and analyzed collision wrappers.
  - No signature change is required, but their documentation must direct callers that need the skip reason to the detailed outcome.
- Event consumers at `src/lib.rs:9773-9779`, `9900-9906`, `9995-10014`, and `10801-10820` inherit the restriction through the shared response function; they do not need a parallel implementation.

### Tests

- `tests/non_ideal_ball_collisions.rs:1179-1212`
  - Existing default-off/named-opt-in configuration contract.
- `tests/non_ideal_ball_collisions.rs:1214-1245`
  - Existing default human-tuned diagnostics contract.
- `tests/non_ideal_ball_collisions.rs:1248-1306`
  - Existing source-domain stationary head-on behavior.
- Add the moving-object, spinning-in-place, threshold, and status cases beside these Kim tests; reuse the file's existing `on_table`, `inches2`, `assert_close`, and `assert_near` helpers.

### Source and generated knowledge metadata

- `src/lib.rs:11866-11867` currently attributes the object/table correction to `whitepapers/motions_of_a_billiard_ball_after_a_cue_stroke.pdf`; that is the wrong paper for this solver.
- The correct authority is `whitepapers/collision_of_two_spinning_billiard_balls_and_the_role_of_table_friction.pdf`.
- `PHYSICS_TODOS.md:128-137` already cites the correct paper but describes only default-off policy, not the stationary-object restriction.
- `agent_knowledge/whitepapers_index.jsonl:63` currently marks the actual Kim collision paper uncited. The files under `agent_knowledge/` are generated by `scripts/build_agent_knowledge.py`; regenerate them after the source citation changes rather than editing them by hand.

## Whitepaper evidence and governing equations

### Kim's actual collision paper

The relevant source is Hyeong-Chan Kim, *Collision of two spinning billiard balls and the role of table*, `whitepapers/collision_of_two_spinning_billiard_balls_and_the_role_of_table_friction.pdf` (article pp. 1, 4, 6-8, and 14-16; corpus document starts at `agent_knowledge/whitepapers_corpus.txt:14264-14276`).

Observed source scope:

- The abstract says that a spinning cue ball approaches a **static object ball** (`agent_knowledge/whitepapers_corpus.txt:14278-14287`).
- The formulation repeats that the cue ball, with center velocity $\vec U_i$ and angular velocity $\vec\omega_i$, collides with a static object ball; the object-ball initial $\vec V$ and $\vec\Omega$ terms are absent from the initial relative-velocity equations (`14407-14433`, `14555-14585`).
- Kim justifies static object/table friction because the object starts still and moves only an extremely small distance through the supporting cloth fibers during the roughly $250$-$300\,\mu\mathrm{s}$ collision (`14362-14372`, `14472-14495`).
- Under that setup, the object/table contact velocity is approximated along the line of centers, $\hat V_=\approx\hat x$, and the table friction is

  $$
  \vec f_g'\approx-f_g'\hat V_=,\qquad f_g'=\mu_s F_N'.
  $$

  The approximation and its direction appear at corpus `14496-14516`. It is not a general solution for an object with arbitrary pre-impact cloth slip.
- Kim's Eqs. (9)-(10) add the table contribution along that assumed $x$ direction and its corresponding torque:

  $$
  M\frac{d\vec V}{dt}\approx F_\perp
  \left[(1+\mu_s\mu\sin\theta\,\Theta(-\theta))\hat x
  +\mu\cos\theta\hat y+\mu\sin\theta\,\Theta(\theta)\hat z\right],
  $$

  $$
  I\frac{d\vec\Omega}{dt}\approx\mu R F_\perp
  \left\{\sin\theta[1+\mu_s\Theta(-\theta)]\hat y-\cos\theta\hat z\right\}.
  $$

  See `agent_knowledge/whitepapers_corpus.txt:14525-14553`; impulse forms and the fixed-sign step function continue at `14602-14646` (Eqs. (15)-(21)).
- The first-order closed forms in Eqs. (52)-(54) give the equal normal-direction speed correction and the object horizontal-spin factor used by the code. In the paper's coordinates, the $\mu_s$ parts are proportional to

  $$
  \delta V_x=\frac{1+e^*}{4}\,\mu\mu_s U_i\cos\phi\sin\theta_0,
  $$

  and

  $$
  \Delta\Omega_{B,y}=\frac{\mu(1+e^*)U_i\cos\phi}{2\nu R}
  (1+\mu_s)\sin\theta_0,
  \qquad \nu=\frac25.
  $$

  See corpus `14988-15023`. The analysis again states that the cue ball collides with a static object ball (`15025-15027`) and applies the table term only in the topspin branch (`15048-15066`).

These equations substantiate the existing correction only inside the stationary-object approximation. They do not license replacing kinetic cloth friction for a moving/sliding ball with a static coefficient or retaining the fixed $\hat x$ direction.

### Why a moving object needs a different solver

For an on-table ball in the repository's coordinates, the actual cloth-contact velocity is already defined as

$$
\vec c_B=(v_{B,x}-R\omega_{B,y},\;v_{B,y}+R\omega_{B,x}),
$$

in `try_cloth_contact_velocity_on_table` (`src/lib.rs:4883-4929`). A general sliding support impulse must initially oppose $\vec c_B$, and its torque must be computed from

$$
\Delta\vec L_B=(-R\hat z)\times\vec J_{\mathrm{table}},
$$

rather than by multiplying the ball-ball spin increment about a predetermined axis. Doménech's rough-support formulation independently resolves sphere-sphere friction and sphere-support friction from their respective contact kinematics and distinguishes gross slip from no-slip (`whitepapers/non_smooth_modelling_of_billiard_and_superbilliard_ball_collisions.pdf`, article pp. 755-756; corpus `agent_knowledge/whitepapers_corpus.txt:35426-35520`, especially Eqs. (16)-(21) and (26)-(31)). That is evidence for the future general-solver boundary, not a formula to splice into this narrow fix.

### Incorrect currently cited paper

`whitepapers/motions_of_a_billiard_ball_after_a_cue_stroke.pdf` studies cue-stick/ball impact, outgoing struck-ball motion, and squirt. Its abstract and introduction explicitly define that scope (`agent_knowledge/whitepapers_corpus.txt:33802-33856`), and its collision section is “COLLISION BETWEEN A BILLIARD BALL AND A RIGID CUE-STICK” (`33862-33887`). It does not derive Kim's object-ball/table Eqs. (52)-(54). Keep that paper for cue-strike citations, including the independent `BallState` discussion at `src/lib.rs:2781-2782`; remove it only from the ball-ball response authority list.

## Numeric reproducer: moving 20 ips object ball

Use the same local basis as the solver:

- ball radius: $R=1.125\,\mathrm{in}$;
- ball-ball normal: $\hat n=+\hat y$;
- solver tangent: $\hat t=+\hat x$;
- cue ball A: $\vec v_A=(0,30)\,\mathrm{in/s}$ and natural topspin $\omega_{A,x}=-30/R=-26.6666667\,\mathrm{rad/s}$;
- object ball B: $\vec v_B=(20,0)\,\mathrm{in/s}$ and $\vec\omega_B=0$;
- $e=1$, ball-ball $\mu=0.06$, and Kim object/table $\mu_s=0.30$.

The current solver obtains

$$
j_n=\frac{1+e}{2}(30)=30\,\mathrm{in/s},
$$

$$
(s_t,s_z)=(-20,-30)\,\mathrm{in/s},\qquad |s|=36.055512755\,\mathrm{in/s}.
$$

The Coulomb cap is active because $\mu j_n=1.8\,\mathrm{in/s}<|s|/7$, so

$$
(j_t,j_z)=1.8\frac{(-20,-30)}{36.055512755}
=(-0.998460353,-1.497690530)\,\mathrm{in/s}.
$$

The unconditional Kim branch then adds

$$
\delta v_{\mathrm{Kim}}=\tfrac12(0.30)(-1.497690530)
=-0.224653579\,\mathrm{in/s}
$$

to both balls along $+\hat y$. The current planar results are therefore approximately

$$
\vec v_A'=(0.998460353,-0.224653579)\,\mathrm{in/s},
$$

$$
\vec v_B'=(19.001539647,29.775346421)\,\mathrm{in/s}.
$$

The table term removes $0.449307159\,\mathrm{in/s}$ of total $y$ momentum and no $x$ momentum. It also adds an object-only spin increment

$$
\mu_s\frac{5|j_z|}{2R}=0.998460353\,\mathrm{rad/s}
$$

about $+\hat x$. But B's pre-impact cloth-contact velocity is $+20\,\mathrm{in/s}$ along $x$, so actual kinetic cloth friction must initially point toward $-\hat x$, with torque $(-R\hat z)\times(-|J|\hat x)$ about $+\hat y$, not $+\hat x$.

Under the proposed conservative restriction, the same input must retain the ordinary ball-ball impulse but set the Kim term to zero. It must exactly match the $\mu_s=0$ baseline:

$$
\vec v_A'=(0.998460353,0)\,\mathrm{in/s},\qquad
\vec v_B'=(19.001539647,30)\,\mathrm{in/s},
$$

with no object-only $0.998460353\,\mathrm{rad/s}$ spin addition. This is a fallback to the supported ball-ball model, not a claim that table friction is physically absent.

## Root cause

1. The configuration stores a scalar `object_table_static_friction_coefficient` without encoding the derivation's object-state precondition.
2. The response converts Kim's stationary-object first-order terms into unconditional algebra guarded only by the sign of `vertical_impulse_per_mass`.
3. The dormant `require_stationary_object_ball` boolean is attached to the entire frictional response instead of specifically to the Kim correction, and both active wrappers disable it.
4. Diagnostics report only the resulting scalar correction, not whether the requested model was applied, physically inactive, disabled, or rejected as out of domain.
5. The ball-ball solver documentation cites Kim's cue-stroke paper instead of his ball-ball/table collision paper, hiding the static-object assumption from maintainers.

## Proposed domain and diagnostic contract

### Stationary-object predicate

Define one private predicate, named for the approximation rather than for general motion classification, and evaluate it on the pre-impact second operand `b`:

$$
\|\vec v_B\|\le\varepsilon_{\mathrm{Kim}}
\quad\land\quad
R\|\vec\Omega_B\|\le\varepsilon_{\mathrm{Kim}},
\qquad
\varepsilon_{\mathrm{Kim}}=10^{-9}\,\mathrm{in/s}.
$$

Both terms are expressed in linear-speed units. Requiring center speed and angular surface speed separately is intentional:

- a translating ball in pure roll can have zero cloth-contact slip but is not Kim's static object;
- a ball with zero center speed but nonzero spin is also not Kim's static, non-spinning object;
- exact `BallState::resting_at` inputs satisfy the predicate;
- the named constant preserves the scale already contemplated by the current `ball_speed(b)>1e-9` guard without borrowing configurable motion-phase thresholds that are unavailable to the collision response.

The second operand remains the Kim object-ball role. Do not silently reorder operands in this change: event pair ordering and friction/spin signs are existing contracts, and role normalization would be a separate behavioral change requiring its own derivation and tests.

### Correction selection

After validating $\mu_s$, compute a single status with this precedence:

1. `DisabledByConfiguration` when $\mu_s=0$;
2. `SkippedObjectNotStationary` when $\mu_s>0$ but the domain predicate fails;
3. `AppliedStationaryObject` when the predicate passes and `vertical_impulse_per_mass<0` activates Kim's topspin step-function branch;
4. `InactiveForImpulseDirection` when the predicate passes but the vertical impulse does not activate that branch.

Only `AppliedStationaryObject` may use the configured coefficient in `kim_table_coupled_normal_correction` and `object_horizontal_spin_scale`. In every other status, use an effective Kim coefficient of zero. In particular, `SkippedObjectNotStationary` must not return the ideal outcome: restitution, ball-ball tangential impulse, throw, and shared ball-ball spin transfer remain exactly as the non-Kim solver computes them.

### Public diagnostics

Add a public, non-payload enum such as:

```rust
pub enum KimTableCorrectionStatus {
    DisabledByConfiguration,
    AppliedStationaryObject,
    InactiveForImpulseDirection,
    SkippedObjectNotStationary,
}
```

Add these fields to `CollisionDiagnostics`:

- `kim_table_correction_status: KimTableCorrectionStatus`;
- `kim_object_center_speed_before: f64` in inches/second;
- `kim_object_spin_surface_speed_before: f64` in inches/second.

Retain `kim_table_coupled_normal_correction` as the observable applied amount. This cleanly distinguishes configuration, domain, and force-direction decisions without allocating or embedding strings in the hot response. Document that tuple-returning collision helpers intentionally discard diagnostics; callers that need to detect an unsupported moving-object request must use a detailed/analyzed outcome.

## Explicit non-goals

- Do not implement kinetic object/cloth friction for a moving object ball in this change.
- Do not implement a simultaneous or sequential general ball-ball/support-contact impulse solver, static-to-kinetic transitions, compliant cloth fibers, or a table-normal restitution model.
- Do not add vertical center-of-mass translation, cue-ball hop, object-ball table impact, or event routing. Those are owned by `plans/ball-collision-vertical-impulse.md`.
- Do not alter the ordinary ball-ball normal impulse, `/7` no-slip cap, tangential throw impulse, transferred ball-ball spin, restitution, or `ThrowAware`/`SpinFriction` aliasing.
- Do not make Kim coupling default-on or change the configured value $\mu_s=0.30$.
- Do not broaden Kim's approximation to a translating ball merely because its cloth-contact point happens to have zero slip.
- Do not auto-swap collision operands to find a stationary ball.
- Do not remove Kim's cue-stroke paper from the corpus or from valid cue-strike/BallState citations.
- Do not hand-edit files under `agent_knowledge/`; they remain generator-owned.

A future implementation may supersede `SkippedObjectNotStationary` only after it computes the actual support contact velocity, chooses static versus kinetic friction consistently, applies a vector table impulse opposite slip, applies $(-R\hat z)\times\vec J_{\mathrm{table}}$, and couples that solve to the vertical ball-ball impulse and support reaction. Merely selecting $\mu_s$ from a moving ball's speed is not sufficient.

## Phased implementation plan

### Phase 1: Add failing behavioral contracts

1. Add the exact $R=1.125\,\mathrm{in}$, moving-$20\,\mathrm{in/s}$ counterexample to `tests/non_ideal_ball_collisions.rs` using explicit constant $e=1$, $\mu=0.06$, and both $\mu_s=0.30$ and $\mu_s=0$ configurations.
2. Assert the Kim-configured moving-object outcome equals the $\mu_s=0$ baseline for every linear-velocity and angular-velocity component, while retaining nonzero ordinary tangential and vertical ball-ball impulse diagnostics.
3. Assert the new status is `SkippedObjectNotStationary`, both measured object speed diagnostics are correct, and `kim_table_coupled_normal_correction==0`.
4. Extend the existing stationary head-on Kim test with `AppliedStationaryObject` and zero pre-impact object-speed diagnostics, without changing its existing recoil, object-speed, cue-spin, or object-spin expectations.
5. Add focused predicate boundary cases: an object with zero center velocity but nonzero spin is skipped; an object at or below the named speed-equivalent tolerance is accepted; an object just above either center-speed or angular-surface-speed threshold is skipped.
6. Add status-precedence cases for a disabled coefficient and for a stationary object whose vertical impulse leaves the Kim step-function term inactive.

### Phase 2: Implement the domain restriction at the source

1. Introduce the named $10^{-9}\,\mathrm{in/s}$ Kim stationary-domain tolerance and a small private helper that returns the pre-impact center speed, angular surface speed, and domain result for `b`.
2. Add `KimTableCorrectionStatus` and the new diagnostic fields.
3. Remove `require_stationary_object_ball` from `frictional_collision_outcome_on_table_with_config`; delete the early ideal return and the boolean-dependent throw branch. Preserve the current moving-object throw-angle branch by testing `ball_speed(b)` directly where it is still needed.
4. Validate the configured coefficient as today, compute status once using the precedence above, and derive an `effective_object_table_static_friction_coefficient` that is nonzero only for `AppliedStationaryObject`.
5. Use that effective coefficient for both the normal correction and `object_horizontal_spin_scale`. Do not duplicate the predicate around each formula.
6. Populate the status and pre-impact speed diagnostics in the single `CollisionOutcome` construction.
7. Simplify `throw_aware_collision_outcome_on_table_with_config` and `spin_friction_collision_outcome_on_table_with_config` to call the private solver without a boolean. No public collision signature changes are needed.

### Phase 3: Migrate public API documentation and internal callers

1. Document the second-operand stationary/non-spinning requirement on `BallBallCollisionConfig`, the Kim builder methods, and the detailed collision entry points.
2. Document each status and the units/meaning of the new diagnostic speeds.
3. Search all `CollisionDiagnostics` construction and exhaustive pattern sites. The current repository has one observed constructor, in `frictional_collision_outcome_on_table_with_config`; update any additional compile-time callsites revealed by the change without adding compatibility aliases.
4. Confirm all tuple-returning and analyzed wrappers continue to route through the detailed radius-explicit implementation. Event callers require no duplicate predicate and must inherit the safe skip automatically.
5. Treat adding a field to the public diagnostics struct as a clean cutover: update repository consumers directly; do not add a deprecated parallel status API or string diagnostic.

### Phase 4: Focused behavioral smoke test

Run the newly added moving-object contract and the retained stationary contract before documentation/generated-artifact cleanup. The smoke test is successful only when the moving case preserves the ordinary non-Kim pair response, reports the domain rejection, and the stationary case retains the existing Kim numbers.

### Phase 5: Correct sources and finish generated/documentation cleanup

1. In the ball-ball response rustdoc at `src/lib.rs:11866-11867`, replace `whitepapers/motions_of_a_billiard_ball_after_a_cue_stroke.pdf` with `whitepapers/collision_of_two_spinning_billiard_balls_and_the_role_of_table_friction.pdf`; describe the cited result as a stationary-object first-order approximation.
2. Leave the cue-stroke paper's independent `BallState` citation at `src/lib.rs:2781-2782` intact.
3. Update `PHYSICS_TODOS.md:128-137` so the completed Kim policy records both default-off and stationary-object-only application, plus the new moving/stationary regression evidence.
4. Run `scripts/build_agent_knowledge.py` through the repository's Nix environment. Accept generated updates under `agent_knowledge/` from the generator only. The actual collision paper should become code-cited; the cue-stroke paper should remain cited because valid cue-strike/BallState references still exist.
5. Do not change `scripts/build_agent_knowledge.py`'s cue-stroke starter entry solely to force a metadata result; citation metadata must follow actual repository references.

## Regression and acceptance tests

The implementation is accepted when all of the following observable contracts hold:

1. **Moving 20 ips counterexample:** with the exact numeric fixture above and Kim enabled, status is `SkippedObjectNotStationary`, correction is zero, and both post-impact states equal the $\mu_s=0$ outcome component-for-component to $10^{-9}$.
2. **Base physics retained:** the moving case still reports $j_t\approx-0.998460353\,\mathrm{in/s}$ and $j_z\approx-1.497690530\,\mathrm{in/s}$; it must not collapse to the ideal response.
3. **Momentum fallback invariant:** because no external table impulse is represented in the skipped case, planar pair momentum remains $(20,30)\,\mathrm{in/s}$ within $10^{-9}$.
4. **No extra torque:** the moving object receives only the shared ball-ball spin increment; the object-only Kim addition $0.998460353\,\mathrm{rad/s}$ is absent.
5. **Stationary behavior retained:** `kim_table_coupled_head_on_topspin_impact_recoils_cue_ball_and_reduces_object_speed` continues to assert a $0.09\,\mathrm{in/s}$ normal-speed correction for $U=10\,\mathrm{in/s}$, $e=1$, $\mu=0.06$, $\mu_s=0.30$, and the existing $(1+\mu_s)$ object-spin scale; status is `AppliedStationaryObject`.
6. **Default-off behavior retained:** `Default`, `ideal()`, and `human_tuned()` still configure $\mu_s=0$, report `DisabledByConfiguration`, and apply no Kim correction.
7. **Static means no translation and no spin:** pure rolling motion, lateral sliding, center-stationary spin, and motion above either speed-equivalent threshold are rejected. Exact rest and values within tolerance remain eligible.
8. **Direction inactivity is not a domain error:** a stationary object in a non-activating impulse direction reports `InactiveForImpulseDirection`, with zero correction.
9. **Model parity:** `ThrowAware` and `SpinFriction` select the same Kim status and outcome for equivalent inputs.
10. **Source reliability:** the ball-ball solver rustdoc cites the 2024 collision/table paper, not the cue-stroke/squirt paper; regenerated knowledge metadata reflects the real citations without manually edited generated files.

Do not add tests that grep source text or generated metadata. Citation correctness is verified by source review and deterministic regeneration, while executable tests defend physics and diagnostic behavior.

## Risks and edge cases

- **Tolerance units:** comparing raw angular speed directly with linear speed would make the predicate radius-dependent in the wrong units. Always compare $R\|\Omega_B\|$ with the inches/second tolerance.
- **Rolling is not stationary:** zero cloth-contact velocity alone must not admit an object translating in pure roll.
- **Spinning at a fixed center is not stationary:** include all three angular components in $\|\Omega_B\|$, including side spin, even though side spin does not contribute to the planar cloth-contact velocity formula.
- **Status precedence:** an out-of-domain request must report `SkippedObjectNotStationary` even when the current vertical impulse would otherwise make the correction numerically zero; otherwise unsupported use remains hidden. A zero configured coefficient reports disabled first.
- **Operand role:** the Kim formula currently treats `b` as the object. Automatically swapping arbitrary event operands could change tangent signs, spin transfer, and which state receives the extra torque; keep the conservative documented role for this change.
- **State-only event APIs:** scheduler and tuple wrappers do not carry collision diagnostics. Their required invariant is safe non-application; callers that need an explicit reason must use the detailed API until a separate event-diagnostics channel exists.
- **Existing vertical limitation:** the retained stationary Kim behavior remains a planar first-order approximation and still omits the paired vertical translation. This plan must not claim otherwise; `plans/ball-collision-vertical-impulse.md` owns that correction.
- **Future general solver:** adding kinetic support contact later must replace, not bypass, the skip branch and must have vector impulse/torque tests. Keep the status enum exhaustive so such a cutover requires an explicit new status rather than silently relabeling the stationary approximation.
- **Generated churn:** knowledge artifacts may shift corpus line numbers after regeneration. Review generated changes for expected citation-status updates, but continue to cite source path, article equation/page, and the pre-change audit corpus locations in this plan.

## Verification commands

Run in this order after implementation:

```sh
cargo test --test non_ideal_ball_collisions kim_table_coupling_skips_moving_object_and_reports_domain
cargo test --test non_ideal_ball_collisions kim_table_coupled_head_on_topspin_impact_recoils_cue_ball_and_reduces_object_speed
cargo test --test non_ideal_ball_collisions kim_object_table_static_friction_is_default_off_but_named_opt_in
cargo test --test non_ideal_ball_collisions
cargo test --test ball_collisions --test ball_collision_timing --test non_ideal_ball_collisions
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
nix develop -c python scripts/build_agent_knowledge.py
```

After regeneration, inspect the two Kim entries and generated reading guide: the collision/table paper must be marked code-cited, while the cue-stroke paper remains available and cited only by its legitimate callsites. Re-run the focused Kim tests if regeneration changes any Rust or test source unexpectedly; normally it should touch only `agent_knowledge/`.

## Dependencies and sibling-plan overlap

- **`plans/ball-collision-vertical-impulse.md`:** owns the richer 3-D collision outcome, vertical translation, object support reaction, and airborne/table-contact event routing. This plan neither duplicates nor blocks that work. If that sibling implements a physically general support-contact solver before this plan lands, the moving-object skip may be replaced only by that solver's explicit vector impulse/torque contract; Kim's static formula must still remain restricted and correctly labeled.
- Shared-contact/event-scheduler work may call the same pair response repeatedly. It should consume the single domain policy here rather than reproduce a second stationary predicate.
- `PHYSICS_TODOS.md:128-137` is historical overlap on default-off policy and the valid stationary head-on fixture; it did not establish the missing moving-object restriction.

## Candidate commit message

```text
Restrict Kim table correction to stationary objects

Skip the opt-in static object/table term for translating or spinning
object balls, expose the domain decision in collision diagnostics, retain
the stationary Kim regression, and cite Kim's actual 2024 ball-collision
paper instead of the cue-stroke paper.
```
