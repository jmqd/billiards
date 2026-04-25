# Shot DSL mini spec

This is the current declarative shot-and-physics syntax for the billiards DSL.

## Scope

This slice supports:

- named cue-strike transfer configs
- named ball-ball collision configs
- named per-rail response configs
- named rail profiles built from those responses
- named simulation presets that bundle the physics knobs
- optional built-in playing-conditions presets on simulation presets
- one optional declarative shot per document
- lowering to validated physics-domain types and scenario helpers

It still does **not** add shot sugar like `.follow(...)` / `.draw(...)`, bank-intent methods like
`.bank(...)`, or multi-shot scripts.

## Canonical syntax

```text
table brunswick_gc4_9ft
ball cue at (1.0, 4.0)
ball one at (2.0, 4.0)
ball two at (3.6, 4.0)

cue_strike(default)
  .mass_ratio(1.0)
  .energy_loss(0.1)

ball_ball(human)
  .normal_restitution(0.95)
  .tangential_friction(0.06)

rail_response(clean)
  .normal_restitution(0.8)
  .tangential_friction(1.0)

rail_response(dead)
  .normal_restitution(0.6)
  .tangential_friction(1.0)

rails(pinball)
  .default(clean)
  .top(dead)
  .right(dead)

simulation(human_pinball)
  .collision_model(throw_aware)
  .ball_ball(human)
  .rail_model(spin_aware)
  .rails(pinball)
  .conditions(humid_dirty)

shot(cue)
  .heading(90deg)
  .speed(128ips)
  .tip(side: 0.0R, height: 0.0R)
  .using(default)
```

Single-line chaining is also valid:

```text
cue_strike(default).mass_ratio(1.0).energy_loss(0.1)
ball_ball(human).normal_restitution(0.95).tangential_friction(0.06)
rail_response(clean).normal_restitution(0.8).tangential_friction(1.0)
rails(pinball).default(clean).top(dead).right(dead)
simulation(human_pinball).collision_model(throw_aware).ball_ball(human).rail_model(spin_aware).rails(pinball).conditions(humid_dirty)
shot(cue).heading(90deg).speed(128ips).tip(side: 0.0R, height: 0.0R).using(default)
```

## Statements

### `cue_strike(name)`

Defines a named cue→ball transfer config.

Required methods, each exactly once:

- `.mass_ratio(value)`
- `.energy_loss(value)`

Validation is delegated to `CueStrikeConfig::new(...)`.

### `ball_ball(name)`

Defines a named ball-ball collision config.

Required methods, each exactly once:

- `.normal_restitution(value)`
- `.tangential_friction(value)`

Validation:

- `normal_restitution` must lie in `[0, 1]`
- `tangential_friction` must be non-negative

### `rail_response(name)`

Defines a named single-rail response config.

Required methods, each exactly once:

- `.normal_restitution(value)`
- `.tangential_friction(value)`

Validation:

- `normal_restitution` must lie in `[0, 1]`
- `tangential_friction` must be non-negative

### `rails(name)`

Defines a named per-rail profile by referencing `rail_response(...)` configs.

Required methods:

- `.default(response_name)` exactly once

Optional override methods, each at most once:

- `.top(response_name)`
- `.right(response_name)`
- `.bottom(response_name)`
- `.left(response_name)`

Semantics:

- the default response seeds all four rails
- side-specific overrides replace the selected rail only

### `simulation(name)`

Defines a named physics preset for scenario execution.

Required methods, each exactly once:

- `.collision_model(model)`
- `.ball_ball(config_name)`
- `.rail_model(model)`
- `.rails(profile_name)`

Optional methods, each at most once:

- `.conditions(preset_name)`

Supported collision-model literals:

- `ideal`
- `throw_aware`
- `spin_friction`

Supported rail-model literals:

- `mirror`
- `restitution_only`
- `spin_aware`

Supported built-in playing-conditions preset literals:

- `neutral` (default when `.conditions(...)` is omitted)
- `humid_dirty`
- `fast_clean`

### `shot(cue)`

Defines the one declarative shot in the document.

Required methods, each exactly once:

- `.heading(angle)`
- `.speed(speed)`
- `.tip(side: x, height: y)`
- `.using(name)`

Current v1 restriction:

- only `shot(cue)` is supported
- at most one `shot(...)` statement may appear in a document

## Units and shot-speed aliases

Numeric shot values use explicit units:

- angles: `deg`
- cue-ball launch speed: `ips`, `mph`, or `kph`
- cue-tip offsets: `R` (ball-radius units)

In the current DSL, `.speed(...)` is the cue ball's immediate post-strike translational launch speed, before cloth drag, sliding-to-rolling transition losses, cushion losses, or collisions. It also accepts Dr. Dave-style shot-speed aliases:

- `touch` = 1.5 mph = 26.4 ips
- `slow` = 3 mph = 52.8 ips
- `medium-soft` = 5 mph = 88.0 ips
- `medium` = 7 mph = 123.2 ips
- `medium-fast` = 8 mph = 140.8 ips
- `fast` = 12 mph = 211.2 ips
- `power` = 20 mph = 352.0 ips
- `typical-power-break` = 27.5 mph = 484.0 ips
- `exceptional-power-break` = 35 mph = 616.0 ips

Numbered stroke aliases are also accepted: `0`/`touch`, `1`/`slow`, `2`/`medium`, `3`/`fast`, `4`/`power`.
Underscore spellings like `medium_soft` are accepted, but the canonical display form is kebab-case.

Examples:

- `30deg`
- `128ips` (128 ips cue-ball launch speed)
- `10mph` (10 mph cue-ball launch speed)
- `medium`
- `3`
- `0.4R`
- `-0.25R`

## Cue-tip semantics

`.tip(side: ..., height: ...)` uses cue-local ball-radius coordinates.

- `side > 0`: striker's right english
- `side < 0`: striker's left english
- `height > 0`: above center / follow
- `height < 0`: below center / draw

Validation is delegated to `CueTipContact::new(...)`.

## Chain semantics

Method order within a chain is semantically irrelevant.

These are equivalent:

```text
simulation(match).collision_model(throw_aware).ball_ball(human).rail_model(spin_aware).rails(pinball).conditions(humid_dirty)
simulation(match).conditions(humid_dirty).rails(pinball).rail_model(spin_aware).ball_ball(human).collision_model(throw_aware)
```

However, duplicate methods are rejected during lowering.

## Build target

The DSL lowers to a scenario-level value:

- `DslScenario { game_state, shot, ball_ball_configs, rail_responses, rail_profiles, simulations }`

Each `SimulationPreset` carries its resolved built-in `PlayingConditions`, defaulting to neutral when omitted.

where the named config maps already contain validated domain-level physics configs and `shot`, when
present, is already constructed from validated domain types.

## Engine seams

Useful current seams include:

- `parse_dsl_to_scenario(...)`
- `DslScenario::strike_shot_on_table(...)`
- `DslScenario::trace_shot_path_with_rails_on_table(...)`
- `DslScenario::trace_shot_path_with_simulation_on_table(...)`
- `DslScenario::simulate_shot_system_with_simulation_on_table_until_rest(...)`
- `DslScenario::simulate_shot_trace_with_simulation_on_table_until_rest(...)`

These let callers either keep using direct engine parameters or resolve named DSL presets and run
through the physics engine by preset name.
