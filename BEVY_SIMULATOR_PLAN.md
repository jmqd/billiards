# Bevy 3D simulator plan

This document captures the recommended design, implementation plan, and gap analysis for adding an interactive 3D simulator / replay viewer to this repository using **Bevy**.

## Decision

Use **Bevy** for the 3D application layer.

Use the existing `billiards` crate as the **simulation source of truth**.

Do **not** introduce Bevy physics as the authoritative simulation engine. The current repo already contains the beginnings of an event-driven billiards simulator with typed units, cue-strike modeling, rail response, and two-ball event simulation. That should remain the canonical physics layer.

In short:

1. `billiards` computes state evolution.
2. A new replay layer converts simulation results into time-indexed tracks.
3. Bevy renders those tracks and provides playback controls.

---

## Goals

### Primary goals

- Render billiards scenarios in a 3D scene.
- Support **play / pause / restart / loop / step / scrub** playback.
- Support authored scenarios such as:
  - a single shot
  - a multi-shot sequence
  - a runout / drill / pattern
- Keep the simulator deterministic and testable.
- Preserve a clean separation between simulation and rendering.

### Non-goals for v1

- Real-time player-controlled aiming/shooting UI.
- Networked multiplayer or full game rules.
- Perfectly realistic table art assets before the playback model exists.
- Replacing the current physics with an external game-physics engine.

---

## Why Bevy

Bevy is a good fit here because:

- the project is already Rust-based
- the current simulation code is already a Rust library
- Bevy gives us cameras, lighting, meshes, materials, input, app state, and desktop packaging
- Bevy is a strong fit for a deterministic viewer that replays precomputed simulation data

Bevy should be treated as:

- **renderer**
- **input/UI shell**
- **asset host**
- **playback runtime**

Not as:

- primary physics engine
- collision authority
- source of billiards math

---

## Current repo baseline

The repo already has several pieces that are directly useful for a replay viewer:

### Present today

- explicit simulation-facing units and vectors in `src/lib.rs`
  - `Seconds`
  - `Inches`
  - `Inches2`
  - `Velocity2`
  - `AngularVelocity3`
  - `BallState`
- cue-strike launch model
  - `strike_resting_ball_on_table(...)`
- on-table motion stepping
  - `advance_motion_on_table(...)`
  - `advance_ball_state(...)`
- two-ball event simulation
  - `simulate_two_on_table_balls(...)`
  - `simulate_two_balls_with_rails_on_table(...)`
- single-ball path tracing with rails
  - `trace_ball_path_with_rails_on_table(...)`
- table / rail / pocket geometry
- static scenario layout and 2D rendering
  - current CLI parses `.billiards` input and renders PNG output
- tests around event sequencing and rail execution
  - `tests/two_ball_simulation.rs`
  - `tests/rail_event_execution.rs`

### Important implication

The repo already has enough physics to justify a **viewer-first** architecture instead of a game-engine-first architecture.

---

## High-level architecture

## Recommended layering

```text
+---------------------------+
| Bevy viewer app           |
| cameras, lighting, UI     |
| playback clock, controls  |
+---------------------------+
              |
              v
+---------------------------+
| Replay / scenario layer   |
| clips, tracks, events     |
| looping, scrubbing        |
+---------------------------+
              |
              v
+---------------------------+
| billiards core            |
| physics, geometry, units  |
+---------------------------+
```

### Architectural rule

The viewer should ask:

> “What should the world look like at time `t`?”

not:

> “Advance the world by the render frame delta.”

That distinction is what makes pause, loop, restart, and scrubbing easy and deterministic.

---

## Core design choice: replay-first, not frame-step-first

### Recommended playback model

Instead of driving simulation directly from Bevy frame time:

1. author or load a scenario
2. run / bake the scenario into a replay clip
3. store time-indexed ball tracks and event markers
4. let Bevy sample the replay clip at any requested playback time

### Benefits

- deterministic playback
- easy pause/resume
- easy looping
- easy scrubbing
- slow motion / fast forward is trivial
- viewer bugs are isolated from physics bugs
- replay clips can be serialized and inspected

### Tradeoff

This is slightly more upfront design work than “simulate every frame,” but it is the lower-regret choice for this repository.

---

## Proposed data model

## Scenario model

A scenario is the authored input.

```rust
pub struct Scenario {
    pub name: String,
    pub table: TableSpec,
    pub initial_layout: ScenarioLayout,
    pub actions: Vec<ScenarioAction>,
}

pub struct ScenarioLayout {
    pub balls: Vec<ScenarioBall>,
}

pub struct ScenarioBall {
    pub ball: BallType,
    pub state: BallState,
}

pub enum ScenarioAction {
    Shot {
        shot: Shot,
        cue: CueStrikeConfig,
        stop: ShotStop,
    },
    Wait {
        duration: Seconds,
    },
    Marker {
        name: String,
    },
    Reset {
        layout: ScenarioLayout,
    },
}

pub enum ShotStop {
    UntilRest,
    Duration(Seconds),
    EventCount(usize),
}
```

### Notes

- A single shot is just a scenario with one `Shot` action.
- A runout is a scenario with several `Shot` actions chained together.
- `Reset` allows drills and loopable practice setups.

## Replay model

A replay clip is the baked output that the viewer consumes.

```rust
pub struct ReplayClip {
    pub duration: Seconds,
    pub sample_rate_hz: u32,
    pub ball_tracks: Vec<BallTrack>,
    pub events: Vec<ReplayEvent>,
    pub segments: Vec<ReplaySegment>,
}

pub struct BallTrack {
    pub ball: BallType,
    pub samples: Vec<BallSample>,
}

pub struct BallSample {
    pub t: Seconds,
    pub position: [f32; 3],
    pub orientation: [f32; 4],
    pub visible: bool,
    pub motion_phase: MotionPhase,
}

pub struct ReplayEvent {
    pub t: Seconds,
    pub kind: ReplayEventKind,
}

pub enum ReplayEventKind {
    ShotStarted,
    BallBallCollision,
    BallRailImpact,
    MotionTransition,
    Pocketed,
    ShotEnded,
    Marker(String),
}

pub struct ReplaySegment {
    pub name: String,
    pub start: Seconds,
    pub end: Seconds,
}
```

### Why fixed samples plus events

- samples make rendering and interpolation simple
- events make the timeline intelligible and step-able
- segments let us loop a single shot or a whole runout

---

## Playback semantics

The viewer should own a playback clock resource roughly like:

```rust
pub struct PlaybackClock {
    pub current_time: Seconds,
    pub paused: bool,
    pub speed: f32,
    pub loop_mode: LoopMode,
    pub active_segment: Option<usize>,
}

pub enum LoopMode {
    Off,
    WholeClip,
    ActiveSegment,
    Range { start: Seconds, end: Seconds },
}
```

### Required controls

- play / pause
- restart clip
- toggle looping
- scrub to arbitrary time
- step one frame
- step one event
- select active segment / shot
- playback speed multiplier

### Good first UI choice

Use **`bevy_egui`** for the control panel in v1.

Reason: it is faster to ship a usable playback/debug UI with egui than with fully custom Bevy UI.

---

## Rendering design

## Coordinate mapping

Keep simulation math in inches.

In the viewer, convert sim coordinates into world-space transforms.

Recommended mapping:

- table-space `x` -> world `X`
- table-space `y` -> world `Z`
- ball height above cloth -> world `Y`

Recommended centering:

- put the center of the playing surface at world origin
- subtract half-table width/length during conversion

Example:

```rust
world_x = inches_x - table_width_inches / 2.0;
world_z = inches_y - table_length_inches / 2.0;
world_y = ball_radius_inches + height_inches;
```

Using inches in world space is acceptable for v1. If lighting/asset scale becomes awkward, add a viewer-only inches->meters conversion layer later.

## Ball orientation

This is an important missing piece.

The current physics state contains angular velocity, but not persistent rendered orientation. For believable 3D playback, stripes/numbers must visibly rotate.

### Recommended v1 approach

During replay baking:

- start each ball at identity orientation
- integrate angular velocity over the bake sample interval
- store a quaternion in each `BallSample`

This is good enough for rendering even before the physics engine itself tracks orientation explicitly.

## Visual assets

### v1

Use procedural or simple built-in assets first:

- plane / box meshes for cloth and rails
- sphere meshes for balls
- simple materials per ball color
- basic pocket holes or placeholder cutouts

### later

Replace with higher-fidelity glTF assets:

- modeled table
- textured balls
- room / lights
- decals / markings

Do not block playback architecture on art.

---

## Bevy app design

## Suggested plugins

```text
SimulatorViewerPlugin
  ├── ScenePlugin
  ├── ReplayPlugin
  ├── PlaybackUiPlugin
  ├── CameraPlugin
  └── DebugOverlayPlugin
```

## Suggested responsibilities

### `ScenePlugin`

- spawn table mesh/materials
- spawn balls and map `BallType -> Entity`
- own lights and environment

### `ReplayPlugin`

- load / receive `ReplayClip`
- sample clip for current playback time
- update ball transforms and visibility
- emit viewer-side events for UI markers

### `PlaybackUiPlugin`

- play/pause buttons
- timeline scrubber
- loop toggles
- shot/runout segment selection
- current event readout

### `CameraPlugin`

- top-down orthographic camera
- free perspective camera
- optional follow-ball camera later

### `DebugOverlayPlugin`

- event labels
- optional ball trails
- impact markers
- velocity/spin debug visualization later

---

## Recommended repo structure

### Target structure

```text
crates/
  billiards/          # existing core lib
  billiards_replay/   # scenario + replay baking
apps/
  bevy_viewer/        # Bevy desktop app
```

### Why this is preferable

- keeps Bevy dependencies out of the core physics crate
- keeps compile times and binary size isolated
- preserves clean ownership boundaries

### Practical note

If a workspace split feels like too much for the very first spike, a temporary first viewer can live in:

```text
src/bin/bevy_viewer.rs
```

But the long-term target should still be a separate viewer package.

---

## Implementation plan

## Phase 0: planning and packaging

### Deliverables

- choose Bevy version
- choose `bevy_egui` for playback UI
- decide whether to start with workspace split or temporary `src/bin`
- define initial scenario and replay Rust types

### Exit criteria

- repo direction agreed
- v1 scope agreed
- clip vs live-stepping decision locked: **clip-first**

---

## Phase 1: replay core

### Deliverables

- add `Scenario`, `ScenarioAction`, `ShotStop`
- add `ReplayClip`, `BallTrack`, `ReplayEvent`, `ReplaySegment`
- add serialization support for authored scenarios and baked clips
- add clip-time sampling helpers:
  - sample at exact time `t`
  - clamp/wrap under loop modes
  - step to next/previous event

### Suggested dependencies

- `serde`
- `ron` or `serde_json`

### Tests

- clip starts at expected initial state
- clip ends at expected final state
- loop wrapping returns consistent sample times
- event stepping lands exactly on event timestamps
- quaternion samples remain normalized within tolerance

### Notes

For fast progress, use **RON** for scenario authoring first. The existing `.billiards` DSL is good for static layout, but a Rust-shaped serde format is faster for expressing action sequences.

---

## Phase 2: first replay baker

### Scope

Start with a narrow but complete vertical slice.

### Deliverables

- build a replay clip for a single shot
- initial supported stop conditions:
  - `UntilRest`
  - `Duration(...)`
- initial supported physics scope:
  - cue strike
  - one moving cue ball
  - rail interactions

### How

Reuse existing APIs where possible:

- `strike_resting_ball_on_table(...)`
- `advance_motion_on_table(...)`
- `trace_ball_path_with_rails_on_table(...)`

### Tests

- baked replay clip matches known shot duration behavior
- replayed cue-ball path agrees with sampled simulation positions
- paused sampling is stable across repeated reads

### Exit criteria

- one authored shot can be baked into a deterministic replay clip

---

## Phase 3: Bevy viewer MVP

### Deliverables

- Bevy app boots to a lit 3D table scene
- balls appear at correct world positions
- replay clip drives transforms
- UI supports:
  - play/pause
  - restart
  - whole-clip loop
  - timeline scrubber
  - speed control
- camera supports:
  - top-down
  - perspective orbit or fly camera

### Visual scope

- procedural table
- sphere balls
- basic ball colors
- no requirement for fancy art yet

### Tests / checks

- coordinate conversion unit tests
- manual verification that a known bank shot loops cleanly
- manual verification that pause/scrub does not drift or explode state

### Good demo target

Port the current `src/bin/bank_path_demo.rs` idea into a first 3D replay demo.

---

## Phase 4: richer event-aware playback

### Deliverables

- timeline markers for collisions, rail impacts, and motion transitions
- step-to-next-event / step-to-previous-event
- optional ghost trails and event markers in the scene
- segment looping for single-shot replay inside larger scenarios

### Why this matters

This turns the viewer from “pretty animation” into a useful simulation inspection tool.

---

## Phase 5: scenario chaining and runouts

### Deliverables

- multi-action scenarios
- concatenate multiple shot clips into one replay
- support `Wait`, `Marker`, and `Reset`
- choose which segment / shot to play or loop

### Result

At this point the app can support:

- looped drills
- shot libraries
- multi-shot runout previews

---

## Phase 6: general multi-ball simulation

### This is the largest core-physics gap.

### Deliverables

- general `N`-ball simulation state
- event scheduler beyond the current two-ball helpers
- deterministic tie-breaking for multiple simultaneous candidates
- pocket capture / removal integration once available

### Notes

The current repo already has strong building blocks for:

- single-ball motion
- two-ball event sequencing
- rail-aware two-ball simulation

But a full 8-ball / 9-ball runout viewer eventually needs generalized multi-ball event simulation.

### Exit criteria

- one full rack or runout can be simulated without hand-written two-ball assumptions

---

## Phase 7: pocket model integration

### Deliverables

- pocketed-ball events in replay clips
- ball disappearance / drop animation rules
- optional post-pocket “ball return” behavior only if explicitly desired later

### Dependency

This depends on the broader pocket-capture work already identified in `PHYSICS_ENGINE_PLAN.md`.

---

## Phase 8: polish

### Deliverables

- improved table and ball assets
- sounds for impacts
- saved cameras
- screenshot / video export
- scenario browser
- DSL integration if we want authored `.billiards` scenes to launch directly into replay mode

---

## Gap analysis

## Summary table

| Area | Current status | Gap severity | Why it matters | Proposed fix |
|---|---|---:|---|---|
| Core physics ownership | Good | Low | Physics already exists in Rust and should remain authoritative | Keep sim in `billiards`; do not move to engine physics |
| Replay/timeline abstraction | Missing | Critical | Needed for pause/loop/scrub and deterministic playback | Add `Scenario` + `ReplayClip` layer |
| 3D viewer app | Missing | Critical | Needed to render scenes interactively | Add Bevy app |
| Ball orientation tracking | Missing | High | Needed for visible spin/stripe rotation | Integrate orientation during replay baking |
| General `N`-ball simulation | Missing | Critical for runouts | Two-ball APIs are not enough for full racks | Add generalized event scheduler |
| Pocket capture in sim | Planned, not complete | High | Needed for realistic made-ball playback | Implement pocket model and replay events |
| Scenario authoring for shots/runouts | Missing | High | Static layout alone is not enough | Add scenario format, likely serde + RON first |
| Asset pipeline for 3D | Missing | Medium | Needed for visual quality, but not for architecture | Use procedural assets first, glTF later |
| Playback UI | Missing | High | Needed for pause/loop/step/scrub | Use `bevy_egui` in v1 |
| Serialization of replay data | Missing | Medium | Helpful for testing, caching, debugging | Add serde support |
| Full-game rules / officiating | Missing | Low for viewer v1 | Nice later, not required for shot playback | Defer |

## Detailed gap notes

### 1. Replay layer does not exist yet

This is the biggest architectural gap for a usable simulator viewer.

Without it, the viewer would need to couple directly to simulation stepping, which would make pause/loop/scrub much harder and less deterministic.

### 2. Current simulation APIs are still partial

The repo has meaningful simulation support, but not yet a complete full-rack engine.

Current strengths:

- one-ball motion
- cue launch
- two-ball event sequencing
- rail-aware variants

Main missing step for full runout playback:

- generalized multi-ball event scheduling and resolution

### 3. Pocket geometry exists, but pocket behavior is not yet a full playback feature

For an authentic 3D simulator, balls need a lifecycle:

- on table
- entering pocket
- pocketed / removed from play

That needs a simulation event, not just geometry.

### 4. Current authoring path is layout-oriented, not replay-oriented

Today the CLI consumes a static `.billiards` description and renders a PNG. That is perfect for diagramming, but not enough for:

- shot sequences
- loopable drills
- runouts
- playback segmentation

### 5. Rendering assets are 2D-specific today

Current assets under `src/assets.rs` are about 2D diagram output. That does not block the viewer, but it does mean the 3D app should start with procedural placeholders.

---

## Recommended first milestone

## Milestone 1: one-shot 3D replay viewer

Deliver:

- one authored shot
- replay baking
- Bevy scene with table + balls
- play/pause/restart/loop/scrub
- top-down and perspective cameras
- visible ball motion and orientation

This milestone intentionally does **not** require:

- full-rack simulation
- pocket capture
- polished art
- rules engine

Why this milestone first:

- it proves the architecture
- it provides immediate visual value
- it de-risks the Bevy integration
- it avoids blocking on the largest remaining physics gaps

---

## Suggested first implementation slices

1. **Define replay types**
   - no rendering yet
   - pure Rust tests only
2. **Bake one cue-ball-only rail shot into a replay clip**
   - use existing motion/path APIs
3. **Render that clip in Bevy**
   - play/pause/loop/scrub
4. **Add event markers and segment loop**
5. **Expand to two-ball replay from existing event simulation**
6. **Only then start generalized `N`-ball simulation work**

---

## Open decisions to lock before implementation

1. **Package layout now or later?**
   - preferred: workspace split
   - acceptable first spike: `src/bin/bevy_viewer.rs`

2. **Scenario file format for v1?**
   - preferred: RON via serde
   - later: maybe extend `.billiards` DSL

3. **Playback sample rate?**
   - suggested initial default: `120 Hz`

4. **Viewer UI library?**
   - recommended: `bevy_egui`

5. **World units in Bevy?**
   - suggested v1: inches
   - later if needed: viewer-only conversion to meters

---

## Bottom line

The right plan is:

- choose **Bevy** for 3D rendering and UI
- keep **`billiards`** as the authoritative simulation engine
- add a **replay/timeline layer** before serious viewer work
- target a **one-shot replay MVP** first
- treat **general multi-ball simulation** and **pocket capture** as the main remaining physics gaps for full runout playback

That path gets us to a usable, loopable, pause-able 3D simulator without throwing away the physics work already done.
