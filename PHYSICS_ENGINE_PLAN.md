# Physics / simulation roadmap

This is a living plan for growing `billiards` from a geometry + diagramming crate into a usable billiards simulator.

## Current foundation

The crate already has useful geometric primitives and a few idealized shot helpers:

- table / pocket / rail geometry
- unit types like `Inches`, `InchesPerSecond`, `RadiansPerSecond`, `CutAngle`
- position translation and ghost-ball-style aiming helpers
- dotted overlay rendering for aim lines
- some physics-adjacent helpers such as `gearing_english()`

That suggests a good implementation strategy:

1. build an **idealized but internally consistent** physics core first
2. lock it down with focused tests
3. only then add richer effects like throw, transferred spin, squirt, swerve, and cushion friction

---

## Guiding principles

### 1. Prefer layered models

Each subsystem should start with a simple model and then grow richer behind explicit APIs.

Examples:

- `CollisionModel::Ideal`
- `CollisionModel::ThrowAware`
- `CollisionModel::SpinFriction`
- `RailModel::Mirror`
- `RailModel::RestitutionOnly`
- `RailModel::SpinAware`

This keeps tests clear and lets downstream code choose realism vs simplicity.

### 2. Favor event-driven transitions over tiny fixed timesteps

For billiards, many important behaviors are piecewise:

- sliding -> rolling
- rolling -> rest
- spinning in place -> rest
- free motion -> impact

Where practical, compute exact or semi-analytic transition times instead of simulating with tiny `dt` loops.

### 3. Keep units explicit

Continue using typed wrappers instead of raw `f64` where possible.

Candidate additions:

- `InchesPerSecondVec2`
- `AngularVelocity3`
- `BallState`
- `SimTimeSeconds`

### 4. Start from ideal textbook behavior

Every richer model should preserve a simpler limiting case.

Examples:

- zero spin + elastic equal-mass collision should reduce to the line-of-centers / tangent-line result
- square rail hit should reflect straight back
- no velocity should remain at rest

---

## Candidate core types

These are suggestions, not commitments.

```rust
pub struct BallState {
    pub position: Position,
    pub velocity_xy: [InchesPerSecond; 2],
    pub angular_velocity_xyz: [RadiansPerSecond; 3],
}

pub enum MotionPhase {
    Sliding,
    Rolling,
    Spinning,
    Rest,
}

pub struct SimBall {
    pub ball: Ball,
    pub state: BallState,
}

pub struct SimConfig {
    pub ball_ball_friction: f64,
    pub ball_cloth_slide_friction: f64,
    pub ball_cloth_roll_friction: f64,
    pub ball_cloth_spin_friction: f64,
    pub ball_ball_restitution: f64,
    pub ball_rail_restitution: f64,
}

pub enum SimEvent {
    SlidingToRolling { ball: BallType },
    RollingToRest { ball: BallType },
    BallBallCollision { a: BallType, b: BallType },
    BallRailCollision { ball: BallType, rail: Rail },
    Pocketed { ball: BallType, pocket: Pocket },
}

pub struct SimulationTrace {
    pub events: Vec<SimEvent>,
    pub final_state: GameState,
}
```

---

## Recommended implementation order

## Phase 1: single-ball cloth motion

### Goal

Simulate one ball from an initial state until rest, including:

- sliding motion
- rolling motion
- spinning in place
- phase transitions

### Why this first

This is the base layer for nearly everything else. Ball-ball and ball-rail interactions produce new linear and angular velocities; this phase determines what happens between those impacts.

### Likely APIs

```rust
pub fn motion_phase(ball: &BallState, config: &SimConfig) -> MotionPhase;
pub fn advance_ball_state(ball: &BallState, dt: f64, config: &SimConfig) -> BallState;
pub fn simulate_ball_until_rest(ball: &BallState, config: &SimConfig) -> Vec<BallState>;
pub fn time_to_roll(ball: &BallState, config: &SimConfig) -> Option<f64>;
```

### First TDD targets

1. A ball with zero linear and angular velocity stays at rest.
2. A rolling ball slows monotonically and stops.
3. A sliding ball eventually transitions to rolling.
4. A spinning-in-place ball eventually stops spinning.

### Good local references

- `whitepapers/motions_of_ball_after_stroke.pdf`
- `whitepapers/sliding_and_rolling.pdf`
- `whitepapers/55. RollingBall.pdf`
- `whitepapers/rolling_friction_intro.pdf`

### Suggested stopping point

Reach a trustworthy **single-ball state integrator** with deterministic tests before adding any collisions.

---

## Phase 2: ideal ball-ball collisions

### Goal

Implement the simplest physically meaningful collision model for two equal billiard balls:

- object ball leaves along the line of centers
- cue ball leaves along the tangent direction in the ideal case
- no throw, no transferred spin, no ball-ball friction effects yet

### Why this second

It composes naturally with Phase 1 and gives an immediately useful simulator for many straight and cut-shot predictions.

### Likely APIs

```rust
pub enum CollisionModel {
    Ideal,
    ThrowAware,
    SpinFriction,
}

pub fn collide_ball_ball(
    a: &BallState,
    b: &BallState,
    model: CollisionModel,
    config: &SimConfig,
) -> (BallState, BallState);
```

### First TDD targets

1. Head-on equal-mass collision transfers forward motion to the struck ball.
2. Straight shot gives zero cut angle.
3. Ideal cut shot sends the object ball along the impact line.
4. Cue-ball outgoing path is perpendicular to the object-ball path in the equal-mass ideal limit.

### Good local references

- `whitepapers/Alciatore_pool_physics_article.pdf`
- `whitepapers/Physics Of Billiards.html`
- `whitepapers/billiards_ball_collisions.pdf`

### Suggested stopping point

A clean, documented `CollisionModel::Ideal` with tests derived from basic textbook geometry.

---

## Phase 3: throw, transferred spin, and gearing

### Goal

Add richer ball-ball contact behavior:

- cut-induced throw (CIT)
- spin-induced throw (SIT)
- transferred spin
- no-slip / gearing conditions

### Why this third

This is where the engine starts to match real shot behavior more closely, but it depends on already having reliable ideal collision handling.

### Likely APIs

```rust
pub struct CollisionOutcome {
    pub a_after: BallState,
    pub b_after: BallState,
    pub throw_angle: Option<Angle>,
    pub transferred_spin: Option<[RadiansPerSecond; 3]>,
}

pub fn collide_ball_ball_detailed(...) -> CollisionOutcome;
```

### First TDD targets

1. Zero side spin reduces to the ideal collision model.
2. A gearing-english condition minimizes or zeroes throw in the idealized limit.
3. Added side spin changes the object-ball departure angle in the expected direction.

### Good local references

- `whitepapers/Alciatore_pool_physics_article.pdf`
- `whitepapers/amateur_physics.pdf`
- `whitepapers/billiards_ball_collisions.pdf`
- `whitepapers/Collision_of_Billiard_Balls_in_3D_with_Spin_and_Friction.pdf`
- `whitepapers/Mathavan_Sports_2014.pdf`
- `whitepapers/art_of_billiards_play_files/bil_praa.html`

### Suggested stopping point

A documented, opt-in non-ideal collision model that still preserves the ideal model as a limiting case.

---

## Phase 4: rail / cushion collisions

### Goal

Simulate ball-rail impacts, first ideally and then with friction / spin effects.

### Implementation ladder

#### 4a. Ideal rail reflection

- mirror reflection of the incoming path
- no spin effects

#### 4b. Restitution-aware rail reflection

- normal speed loss at impact
- simple rebound-speed modeling

#### 4c. Spin-aware rail model

- topspin / draw / running english affecting rebound angle
- friction at the cushion contact point

### Likely APIs

```rust
pub enum RailModel {
    Mirror,
    RestitutionOnly,
    SpinAware,
}

pub fn collide_ball_rail(
    ball: &BallState,
    rail: Rail,
    model: RailModel,
    config: &SimConfig,
) -> BallState;
```

### First TDD targets

1. Square hit reflects straight back.
2. A 45° no-spin bank reflects symmetrically in the ideal model.
3. Running english changes rebound direction in a spin-aware model.

### Good local references

- `whitepapers/Mathavan_IMechE_2010.pdf`
- `whitepapers/Mathavan_Sports_2014.pdf`
- `whitepapers/dynamics_in_carom_three_cushion.pdf`

---

## Phase 5: cue-strike launch model

### Goal

Turn cue input into initial cue-ball state.

Inputs may include:

- cue speed
- horizontal tip offset
- vertical tip offset
- cue elevation
- cue mass / cue properties later, if desired

Outputs:

- cue-ball linear velocity
- topspin / draw
- sidespin
- optional squirt / swerve estimates

### Why this matters

This moves the engine from “simulate from a chosen initial state” to “simulate from a player action.”

### Likely APIs

```rust
pub struct CueStrike {
    pub cue_speed: InchesPerSecond,
    pub horizontal_tip_offset: Inches,
    pub vertical_tip_offset: Inches,
    pub cue_elevation_degrees: f64,
}

pub fn strike_cue_ball(strike: &CueStrike, cue_ball: &Ball, config: &SimConfig) -> BallState;
```

### First TDD targets

1. Center-ball hit produces forward speed and near-zero spin.
2. High hit produces topspin.
3. Low hit produces draw.
4. Left/right hit produces sidespin.

### Good local references

- `whitepapers/motions_of_ball_after_stroke.pdf`
- `whitepapers/Shepard_squirt.pdf`
- `whitepapers/coriolis_billiards.pdf`
- `whitepapers/Design Fabrication and Implementation of Jump-Cue Testing Machi.pdf`

---

## Phase 6: pocket interaction / capture model

### Goal

Model whether a ball approaching a pocket is:

- pocketed cleanly
- jawed / rattled
- rejected
- passed across the face

### Caveat

This looks less directly supported by the current local references than the earlier phases, so some empirical tuning may be necessary.

### Likely APIs

```rust
pub enum PocketOutcome {
    Made,
    Jawed,
    Rejected,
    CrossedFace,
}

pub fn simulate_pocket_entry(ball: &BallState, pocket: Pocket, config: &SimConfig) -> PocketOutcome;
```

### First TDD targets

1. Slow straight-center entry is pocketed.
2. A ball crossing the mouth far from the opening center is not automatically pocketed.
3. A very high-speed near-jaw entry can be rejected in a richer model.

---

## Phase 7: shot planning / AI / search

### Goal

Use the simulation core to evaluate and rank candidate shots.

Examples:

- best potting option from current layout
- best cue-ball leave
- best breakout path
- safety candidates
- simple Monte Carlo under player error

### Likely APIs

```rust
pub struct ShotPlan {
    pub target_ball: BallType,
    pub target_pocket: Pocket,
    pub cue_strike: CueStrike,
    pub expected_score: f64,
}

pub fn enumerate_candidate_shots(state: &GameState) -> Vec<ShotPlan>;
pub fn score_shot_plan(state: &GameState, plan: &ShotPlan, config: &SimConfig) -> f64;
```

### Good local references

- `whitepapers/competitive_pool_playing_robot.pdf`
- `whitepapers/computational_pool.pdf`
- `whitepapers/Long_IEEE_04_article.pdf`
- `whitepapers/Nierhoff_IEEE_15_article.pdf`

---

## Recommended first milestone

If implementation starts soon, the best first milestone is:

### Milestone A: ideal simulation core

Deliver:

1. single-ball motion until rest
2. ideal ball-ball collisions
3. ideal rail collisions
4. basic event trace

That would already support:

- shot-path prediction
- simple cut-shot outcome prediction
- bank path prediction
- animation / trajectory rendering
- a strong base for future spin realism

---

## Concrete first TDD tasks

If work begins later, these are the smallest next slices to implement:

### Task 1: single rolling ball

Add tests for:

- rolling ball decelerates monotonically
- rolling ball eventually stops
- stationary ball remains stationary

### Task 2: ideal head-on collision

Add tests for:

- equal-mass straight collision transfers forward motion to the object ball
- cue ball stops in the perfectly ideal limit

### Task 3: ideal rail reflection

Add tests for:

- square rail hit reflects straight back
- 45° rail hit mirrors correctly

---

## Open design questions

These should be decided before deeper implementation:

1. **Event-driven vs fixed-step engine:**
   - event-driven is likely better for clarity and determinism
   - fixed-step may be easier for animation
   - a hybrid may be best

2. **How much spin realism to expose in the public API?**
   - simple helpers for common cases
   - lower-level state vectors for serious simulation

3. **How much should be deterministic vs empirical?**
   - early phases should stay close to the references
   - later pocket / jaw / cloth-tuning work may need empirical constants

4. **Do we want a solver core independent of `GameState`?**
   - likely yes
   - keep simulation state separate from rendering / UI concerns

---

## Suggested repo direction

When implementation begins, consider eventually splitting `src/lib.rs` into modules such as:

- `physics/units.rs`
- `physics/state.rs`
- `physics/motion.rs`
- `physics/collision_ball.rs`
- `physics/collision_rail.rs`
- `physics/cue_strike.rs`
- `physics/pocket.rs`
- `planning/shot_search.rs`

That refactor is probably better done alongside the first real simulator work rather than immediately.

---

## Summary

The highest-value, lowest-regret path appears to be:

1. single-ball cloth motion
2. ideal ball-ball collisions
3. ideal rail collisions
4. richer throw / spin / transfer behavior
5. cue-strike launch model
6. pocket capture model
7. shot planning / search

This preserves a clean progression from geometry -> ideal physics -> richer physics -> planning.
