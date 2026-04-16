# Shot DSL mini spec

This is the first-pass declarative shot syntax for the existing billiards DSL.

## Scope

This slice adds:

- named cue-strike transfer configs
- one optional declarative shot per document
- lowering from DSL syntax to validated physics-domain types:
  - `CueStrikeConfig`
  - `CueTipContact`
  - `Shot`

It does **not** yet add shot sugar like `.follow(...)` / `.draw(...)`, bank-intent methods like
`.bank(...)`, or multi-shot scripts.

## Canonical syntax

```text
table brunswick_gc4_9ft
ball cue at center

cue_strike(default)
  .mass_ratio(1.0)
  .energy_loss(0.1)

shot(cue)
  .heading(30deg)
  .speed(128ips)
  .tip(side: 0.0R, height: 0.4R)
  .using(default)
```

Single-line chaining is also valid:

```text
cue_strike(default).mass_ratio(1.0).energy_loss(0.1)
shot(cue).heading(30deg).speed(128ips).tip(side: 0.0R, height: 0.4R).using(default)
```

## Statements

### `cue_strike(name)`

Defines a named cue→ball transfer config.

Required methods, each exactly once:

- `.mass_ratio(value)`
- `.energy_loss(value)`

Semantics:

- `mass_ratio` is the effective cue-mass / ball-mass ratio used by the current strike model.
- `energy_loss` is the strike-model energy-loss fraction.

Validation is delegated to `CueStrikeConfig::new(...)`.

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

## Units

The current DSL requires explicit units on shot values:

- angles: `deg`
- cue speed: `ips`
- cue-tip offsets: `R` (ball-radius units)

Examples:

- `30deg`
- `128ips`
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
shot(cue).heading(30deg).speed(128ips).tip(side: 0.0R, height: 0.4R).using(default)
shot(cue).using(default).tip(side: 0.0R, height: 0.4R).speed(128ips).heading(30deg)
```

However, duplicate methods are rejected during lowering.

## Build target

The DSL now lowers to a scenario-level value:

- `DslScenario { game_state, shot }`

where `shot` is optional and, when present, is already constructed from validated domain types.

## Engine seam

The first concrete engine seams are:

- `parse_dsl_to_scenario(...)`
- `DslScenario::strike_shot_on_table(...)`
- `DslScenario::trace_shot_path_with_rails_on_table(...)`

These yield either the immediate post-strike `OnTableBallState` or a traced single-ball preview
path that can be fed directly into the existing motion, rail, rendering, and path-sampling APIs.
