# Diagram rendering roadmap

Date: 2026-06-29

## Goal

Move billiards diagrams from a PNG-first, pixel-mutating renderer to a backend-neutral diagram pipeline where SVG is the primary 2D artifact, raster output is optional, and the same layout/replay data can feed interactive viewers such as Bevy or another game engine.

## Current state observed in the repo

- `GameState` owns `ball_positions` plus a private `lines_to_draw: Vec<Overlay>`.
- `Overlay` is currently a render-time enum inside `src/lib.rs` with variants for dashed lines, smooth polylines, ghost balls, event markers, and text labels.
- `GameState::draw_2d_diagram_with_options(...)` directly returns encoded PNG bytes.
- Rendering loads PNG assets for the table and balls from `src/assets.rs` and composites them onto an `image::RgbaImage`.
- `src/assets.rs` hard-codes the current 1089 x 1938 table image and pixel anchors: `LEFTMOST`, `RIGHTMOST`, `TOPMOST`, `BOTTOMMOST`.
- `src/drawing.rs` is a raster mutator layer: anti-aliased lines, polygons, circles, bitmap text, and ghost balls all draw directly into `RgbaImage`.
- The CLI in `src/main.rs`, demo binaries, `xtask validation-suite`, and tests assume PNG output.
- `BEVY_SIMULATOR_PLAN.md` already identifies the right simulation/rendering separation for playback: keep simulation in inches, convert to viewer/world coordinates at the renderer boundary.

That means the hack is not just PNG encoding. The deeper issue is that layout, styling, coordinate mapping, asset choice, and raster drawing are coupled in one path.

## Design principles

1. **Scene first, backend second.** Produce a typed diagram scene from `GameState` and trace data, then render that scene to SVG, PNG, Bevy entities, or a future engine.
2. **Use physical coordinates internally.** Keep scene geometry in table-space inches or normalized table coordinates, not pixels. Pixel dimensions become one backend option, not the source of truth.
3. **Make SVG the canonical 2D target.** SVG preserves semantic groups, layers, text, styles, IDs, accessibility labels, and animation hooks. PNG should become an export/rasterization target.
4. **Separate static diagrams from replay.** A final-layout diagram and an animated shot playback share assets and coordinate transforms, but replay needs time-indexed tracks rather than only final `GameState` overlays.
5. **Prefer procedural vector assets first.** Replace the baked table PNG and ball sprites with vector table/ball primitives before chasing high-fidelity art.
6. **Preserve deterministic tests.** Unit tests should assert scene primitives and coordinate transforms. Golden image tests can exist, but should not be the primary correctness check.

## Direction A: SVG-first static renderer

This is the smallest useful evolution and the best first step.

### Shape

Add a `diagram` module with an intermediate representation:

```rust
pub struct DiagramScene {
    pub table: TableVisual,
    pub view_box: ViewBox,
    pub layers: Vec<DiagramLayer>,
}

pub struct DiagramLayer {
    pub id: DiagramLayerId,
    pub elements: Vec<DiagramElement>,
}

pub enum DiagramElement {
    Table(TableVisual),
    Ball(BallGlyph),
    Path(PathStroke),
    Circle(CircleGlyph),
    Text(TextGlyph),
    GhostBall(GhostBallGlyph),
}
```

The existing `GameState` render entrypoint becomes two steps:

```rust
impl GameState {
    pub fn to_diagram_scene(&self, options: &DiagramSceneOptions) -> DiagramScene;
    pub fn render_diagram(&self, backend: &mut impl DiagramBackend, options: &DiagramSceneOptions) -> Result<()>;
}
```

`SvgBackend` serializes the scene to SVG:

```rust
pub trait DiagramBackend {
    type Output;

    fn begin(&mut self, scene: &DiagramScene) -> Result<()>;
    fn draw_element(&mut self, layer: DiagramLayerId, element: &DiagramElement) -> Result<()>;
    fn finish(self) -> Result<Self::Output>;
}
```

### Why this works

- The current overlay enum already looks like a scene graph, but it is private and pixel-specific at render time.
- The SVG backend can draw the current table, balls, aim lines, trace lines, labels, and markers without requiring physics changes.
- Arbitrary zoom is mostly free because the SVG `viewBox` is in stable table coordinates.
- Static diagrams become inspectable and stylable: `<g id="balls">`, `<g id="trace-cue">`, `<path data-event="rail-impact">`, etc.

### First deliverables

- `DiagramScene` built from `GameState`.
- `SvgBackend` with procedural table, rails, pockets, diamonds, balls, paths, ghost balls, markers, and text.
- CLI output format selection by extension: `.svg` writes SVG, `.png` keeps existing behavior temporarily.
- `xtask validation-suite --format svg|png|both`.
- Tests that assert:
  - center spot maps to the same table-space location as today;
  - ball diameter derives from `BallSpec` / table scale, not a magic sprite size;
  - SVG contains stable layer IDs and expected primitive counts;
  - transparent/no-background scenes omit table visuals but keep balls and overlays.

### Risks

- Recreating ball sprites procedurally will initially look less polished than PNG sprites. Accept this for v1; visual fidelity can improve after architecture is clean.
- Text rendering in SVG is font-dependent. For labels, use simple SVG text for readability; for exact visual golden tests, assert semantic text elements rather than glyph pixels.
- If old PNG output must remain during transition, it should be a backend adapter, not the core representation.

## Direction B: Backend-neutral scene with retained PNG as compatibility export

This is a slightly larger but cleaner cutover: extract scene generation first, then make both SVG and PNG backends consume it.

### Shape

1. Introduce `DiagramScene` and convert existing `GameState` overlays into scene primitives.
2. Move the current raster logic behind `RasterPngBackend`.
3. Replace hard-coded `diamond_to_pixel(...)` calls with a shared `DiagramTransform`:

```rust
pub struct DiagramTransform {
    pub table: TableSpec,
    pub viewport: DiagramViewport,
}

impl DiagramTransform {
    pub fn table_inches_to_scene(&self, point: Inches2) -> ScenePoint;
    pub fn scene_to_raster_px(&self, point: ScenePoint, raster: RasterViewport) -> PixelPoint;
    pub fn scene_to_svg(&self, point: ScenePoint) -> SvgPoint;
}
```

### Why this works

- Existing PNG tests can be migrated gradually.
- The same primitive stream feeds SVG and raster output, so behavioral fixes land once.
- It exposes the real seams: coordinate transforms, visual styles, primitives, asset sources, and output encoders.

### First deliverables

- A shared transform with no dependency on the current PNG table asset.
- A `RasterPngBackend` that initially may still use PNG table/ball assets, but receives scene primitives.
- Deprecate `draw_2d_diagram()` in favor of `render_diagram(DiagramOutputFormat::Png)` only if compatibility is desired. Project context says backwards compatibility is not a goal, so a clean replacement API is acceptable.

### Risks

- If the raster backend keeps the old asset dimensions too long, pixel assumptions can leak back into the scene model.
- Dual backends add short-term implementation cost. The payoff is that SVG, PNG, and later Bevy share one data path.

## Direction C: Replay/animation model for SVG and Bevy

This is the step that enables animation, scrubbing, and game-engine viewers.

### Shape

Separate static diagram state from playback tracks:

```rust
pub struct ReplayScene {
    pub static_scene: DiagramScene,
    pub duration: Seconds,
    pub tracks: Vec<ReplayTrack>,
    pub events: Vec<ReplayEventMarker>,
}

pub struct ReplayTrack {
    pub subject: ReplaySubject,
    pub samples: Vec<ReplaySample>,
}

pub struct ReplaySample {
    pub t: Seconds,
    pub position: Inches2,
    pub velocity: Velocity2,
    pub angular_velocity: AngularVelocity3,
    pub orientation: Option<Orientation3>,
    pub phase: MotionPhase,
}
```

SVG animation backend options:

- Static SVG with full trace paths and event markers.
- Animated SVG using `<animateMotion>` or generated CSS/SMIL-style keyframes for ball positions.
- Interactive HTML+SVG wrapper for play/pause/scrub/step controls.

Bevy backend options:

- Convert `ReplayScene` to Bevy entities and components.
- Use the same `ReplayTrack` samples for deterministic playback.
- Map table-space inches to Bevy world coordinates as already described in `BEVY_SIMULATOR_PLAN.md`.

### Why this works

- Animation should not be inferred from final layout plus drawn paths. It needs a first-class timeline.
- The current trace pipeline already computes segments and events. The missing piece is a stable sampled replay artifact.
- SVG and Bevy can consume the same replay data even though they render differently.

### First deliverables

- `ReplayScene::from_scenario_trace(...)` that samples every on-table ball at a configurable step.
- Event timeline metadata with IDs, labels, ball IDs, event kind, and timestamp.
- Static SVG trace output with semantic event markers.
- Optional HTML+SVG viewer with play/pause/scrub using a small generated script.
- Bevy prototype consuming the same replay JSON or Rust structs.

### Risks

- Oversampling full-rack breaks can get large. Store event-bounded segments plus adaptive samples, not blindly dense per-frame arrays.
- Ball orientation is not fully represented today. Use `angular_velocity` integration for visual orientation as the Bevy plan already recommends, but mark it as replay-derived visual state until physics owns orientation.
- SVG animation is good for documentation and sharing; it is not a substitute for Bevy when lighting, 3D orientation, camera, or high frame-rate interaction matter.

## Direction D: Interactive SVG diagram viewer

This is a web-facing path between static SVG and a full game engine.

### Shape

Generate a self-contained `index.html` or embeddable component that wraps SVG with:

- pan and arbitrary zoom;
- layer toggles: table, balls, aim lines, traces, labels, event markers;
- hover/click details for events;
- time scrubber for `ReplayScene`;
- selectable ball traces;
- export buttons for SVG and PNG snapshots.

### Why this works

- The validation suite already writes an HTML gallery. It can evolve into a real diagnostic viewer instead of only a page of PNGs.
- SVG element IDs and data attributes make event inspection straightforward.
- It gives fast value for physics review without committing to Bevy UI work.

### First deliverables

- `xtask validation-suite --format html-svg` that embeds SVG instead of `<img src="*.png">`.
- Keyboard/mouse pan-zoom controls.
- Layer checkboxes.
- Event log lines linked to SVG markers.
- Screenshot/export path only after SVG output is stable.

### Risks

- Browser behavior and fonts can vary. Keep semantic tests at the scene/SVG structure layer.
- A rich HTML viewer can grow into app code. Keep it generated and small until there is a clear product surface.

## Direction E: Full Bevy/game-engine viewer

This is the high-ceiling direction for animation, 3D, camera control, and eventual gameplay/simulation UI.

### Shape

Build a renderer crate or module that consumes `ReplayScene`:

```rust
pub trait ReplayRenderer {
    fn load_scene(&mut self, scene: ReplayScene);
    fn set_time(&mut self, t: Seconds);
    fn set_playback_state(&mut self, state: PlaybackState);
}
```

Bevy implementation:

- table, rails, pockets, diamonds as procedural meshes first;
- ball entities with material/color/number metadata;
- replay samples drive transforms;
- orientation derived by integrating angular velocity between samples;
- UI for play/pause/restart/loop/step/scrub;
- camera presets: top-down diagram, angled table, ball-follow, shot-line.

### Why this works

- Bevy should not own physics output or diagram semantics. It should render a replay artifact.
- A retained replay model makes playback deterministic and scrubbable.
- The same replay data can later feed another game engine if Bevy is not the final target.

### First deliverables

- A small Bevy app that opens a baked replay file and plays it top-down.
- No dependency on the CLI PNG/SVG path.
- Shared coordinate transform tests between SVG top-down and Bevy top-down positions.

### Risks

- Bevy is a bigger dependency and application runtime. Keep it optional and downstream from the core crate.
- 3D visual quality can distract from architecture. Use procedural assets first.
- If Bevy consumes live simulation instead of replay samples, scrub/loop/restart become harder and nondeterminism can leak into rendering.

## Recommended path

### Phase 1: Extract diagram scene and SVG backend

Do this first.

- Add `DiagramScene`, `DiagramElement`, `DiagramLayerId`, `DiagramStyle`, and `DiagramTransform`.
- Implement `GameState::to_diagram_scene(...)`.
- Write an `SvgBackend` using procedural vector table and ball drawing.
- Add CLI output extension detection: default to `.svg` for new docs/examples, still allow `.png` where explicitly requested.
- Update validation suite to support SVG output.
- Keep old PNG renderer only as a temporary reference until the SVG scene reaches feature parity.

Success criteria:

- A static `.billiards` layout renders to SVG with table, balls, ghost balls, aim lines, trace lines, labels, and markers.
- Browser zoom shows no pixelation for table geometry, balls, traces, or text.
- Rendering tests assert scene primitives and SVG structure without decoding PNG.

### Phase 2: Unify or retire PNG

- Move PNG behind a backend that consumes `DiagramScene`, or drop it from primary workflows if SVG satisfies examples and validation.
- Stop loading the table PNG for normal diagram output.
- Replace pixel-anchor tests with transform tests.
- Keep PNG snapshot export as a convenience, not a design constraint.

Success criteria:

- No core diagram layout code depends on the 1089 x 1938 PNG asset.
- PNG output, if retained, is a rasterization/export backend with the same scene semantics as SVG.

### Phase 3: Add replay scene

- Add `ReplayScene` and `ReplayTrack` generated from `ScenarioShotTrace`.
- Store sampled positions, phases, velocities, and event metadata.
- Add static SVG trace output from replay data.
- Add optional SVG/HTML animation with scrubber.

Success criteria:

- One replay artifact can produce:
  - a final static diagram;
  - a full trace diagram;
  - an animated SVG/HTML viewer;
  - a Bevy prototype input.
- Event log entries and visual markers share stable IDs.

### Phase 4: Interactive viewers

- Evolve `xtask validation-suite` into an SVG-based physics review gallery.
- Add pan/zoom, layer toggles, event selection, and time scrub.
- Prototype Bevy playback from the same replay data.

Success criteria:

- Physics review can inspect arbitrary zoom levels and event timing without regenerating PNGs.
- Bevy playback matches top-down SVG positions at sampled timestamps within a tested tolerance.

## API sketch

```rust
pub enum DiagramOutputFormat {
    Svg,
    Png,
    HtmlSvg,
}

pub struct DiagramExportOptions {
    pub format: DiagramOutputFormat,
    pub background: DiagramBackground,
    pub viewport: DiagramViewport,
    pub theme: DiagramTheme,
}

impl GameState {
    pub fn to_diagram_scene(&self, options: &DiagramExportOptions) -> DiagramScene;
}

pub trait DiagramBackend {
    type Output;
    fn render(scene: &DiagramScene, options: &DiagramExportOptions) -> Result<Self::Output, DiagramRenderError>;
}

pub struct SvgBackend;
pub struct RasterPngBackend;
```

Replay side:

```rust
impl ScenarioShotTrace {
    pub fn to_replay_scene(
        &self,
        scenario: &DslScenario,
        options: &ReplaySceneOptions,
    ) -> ReplayScene;
}
```

## Migration notes

- Replace `draw_2d_diagram()` with explicit export APIs. Backwards compatibility is not required in this repo, so avoid long-lived shims.
- Rename PNG-specific helpers such as `write_png_to_file(...)` to format-neutral output helpers.
- Move `Overlay` out of `src/lib.rs` and into a diagram module with public, tested semantics.
- Convert `src/drawing.rs` from the core renderer into either:
  - a temporary raster backend helper; or
  - a deleted module after SVG becomes primary.
- Replace `assets::diamond_to_pixel(...)` with `DiagramTransform`; keep old tests as transform parity checks only during migration.
- Prefer data-driven style structs over ad hoc pixel widths. Style values can still include stroke widths, but interpret them in scene/SVG units and scale intentionally.

## Testing strategy

- Unit-test table/inches/diamond to scene coordinate transforms.
- Unit-test primitive generation from `GameState`:
  - ball count;
  - layer order;
  - trace path count;
  - event marker IDs;
  - ghost-ball placement;
  - transparent background behavior.
- Snapshot-test small SVG snippets only for stable structure, not whitespace.
- Keep a small number of visual smoke tests in the validation gallery.
- For replay, assert:
  - samples are monotonic by timestamp;
  - first and final samples match simulation states;
  - event markers refer to valid sample intervals;
  - Bevy/SVG coordinate transforms agree for known timestamps.

## Open decisions

1. **Scene coordinate unit:** inches are best for physics alignment; normalized table units are best for SVG ergonomics. Recommendation: store inches in the scene model and let backends choose `viewBox` scaling.
2. **Ball art:** procedural vector balls first, later optional SVG symbols or asset packs.
3. **SVG animation style:** start with static SVG plus HTML scrubber generated from replay JSON. It is more controllable than relying only on SVG-native animation.
4. **PNG compatibility:** keep it only as an export target if existing workflows need raster files. Do not let it constrain core design.
5. **Crate layout:** keep core scene/replay types in the main crate; put Bevy viewer behind a separate binary or optional crate to avoid forcing runtime dependencies on library users.

## Bottom line

The next-level architecture is not "add SVG next to PNG". It is:

1. `GameState` / trace data -> typed `DiagramScene` or `ReplayScene`.
2. Scene/replay -> SVG, PNG export, HTML/SVG viewer, Bevy, or another backend.
3. Coordinates remain physical; rendering choices stay at the backend boundary.

That gives SVG quality now, arbitrary zoom immediately, animation through replay tracks, and a clean path to Bevy without making the game engine the source of truth for diagrams.
