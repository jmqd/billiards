# Cloth motion calibration notes

Working note comparing the current repo motion settings with the local whitepapers and the locally
installed `pooltool` defaults.

## Why this note exists

Recent probe sweeps suggest the cue ball can keep meaningful spin and post-contact action longer
than expected. This note checks whether the current cloth-motion parameters are plausibly too weak
at dissipating spin and spin-related sliding mismatch.

## Current repo motion settings used by preview / example tooling

The current preview / example motion config uses:

- sliding friction acceleration: `15 ips²`
- rolling resistance deceleration: `5 ips²`
- z-spin angular deceleration: `10.9 rad/s²`

The previous baseline used during the initial calibration sweeps was `2 rad/s²` for z-spin decay.

Because gravity is about `386.09 ips²`, these imply effective cloth coefficients of roughly:

- `mu_s ≈ 15 / 386.09 ≈ 0.039`
- `mu_r ≈ 5 / 386.09 ≈ 0.013`

## Local whitepaper references

### Sliding friction

The local corpus repeatedly cites a typical ball-cloth sliding-friction coefficient near:

- `mu_s ≈ 0.2`

For example, the extracted corpus includes multiple TP A.4 / draw-shot derivations with:

- `μs := 0.2` typical ball-cloth coefficient of sliding friction

At billiard scale, this corresponds to a sliding deceleration magnitude near:

- `mu_s g ≈ 0.2 * 386.09 ≈ 77.2 ips²`

That is much larger than the current `5 ips²`.

### Rolling resistance

`whitepapers/tp_b_2_rolling_resistance_spin_resistance_and_ball_turn.pdf` gives a typical rolling
resistance coefficient of:

- `mu_r := 0.01`

This corresponds to:

- `mu_r g ≈ 0.01 * 386.09 ≈ 3.86 ips²`

That is reasonably close to the current `5 ips²`.

### z-spin decay / spin resistance

The local Petit / Dr. Dave references model z-spin decay during both sliding and rolling as linear in
time:

- `whitepapers/art_of_billiards_play_files/bil_praa.html`
  - §7.5, Eqs. `(M13)` through `(M14")`
  - `t_spin_stop = (2/5) R^2 ||Ω_i vertical|| / (fz g)`

`TP B.2` also gives a direct experimental clue:

- measured spin-down rate approximately `α_meas ≈ 10 rad/s²`

So a first-pass z-spin angular deceleration on the order of `10 rad/s²` is much more consistent
with the local references than the previous `2 rad/s²` baseline.

## Local `pooltool` comparison

A local install of `pooltool` is available at:

- `/Users/jmq/.pyenv/versions/3.11.4/lib/python3.11/site-packages/pooltool`

Relevant defaults from `pooltool/objects/ball/params.py`:

- `u_s = 0.2`
- `u_r = 0.01`
- `u_sp_proportionality = 10 * 2 / 5 / 9`

Its z-spin decay in `pooltool/physics/evolve/__init__.py` is:

- `alpha = 5 * u_sp * g / (2 * R)`

With the default ball radius, this works out to about:

- `alpha_z ≈ 10.9 rad/s²`

So `pooltool` lines up closely with the local whitepaper picture:

- `mu_s ≈ 0.2`
- `mu_r ≈ 0.01`
- `alpha_z ≈ 10 to 11 rad/s²`

## Main comparison

### Sliding friction

- current preview / example defaults: `15 ips²` -> `mu_s ≈ 0.039`
- whitepapers / pooltool-like: `~77.2 ips²` -> `mu_s ≈ 0.2`

This means the current preview / example sliding friction is still roughly:

- **5x weaker** than the usual whitepaper / pooltool value

This matters for:

- how quickly follow / draw mismatch is converted toward rolling,
- how long a post-contact sliding bend survives,
- how long rail-exit horizontal-spin mismatch survives before cloth re-stabilizes the ball.

### Rolling resistance

- current repo: `5 ips²` -> `mu_r ≈ 0.013`
- whitepapers / pooltool-like: `~3.86 ips²` -> `mu_r ≈ 0.01`

This is only a modest difference.

So rolling resistance is **not** the strongest candidate for overactive spin behavior.

### z-spin decay

- current repo: `10.9 rad/s²`
- prior repo baseline: `2 rad/s²`
- TP B.2 measured spin-down: `~10 rad/s²`
- pooltool default equivalent: `~10.9 rad/s²`

So the current repo's z-spin decay is now broadly aligned with the whitepaper / `pooltool`
picture, while the previous baseline was roughly:

- **5x weaker** than the whitepaper / pooltool picture

This matters for:

- side-spin lingering during rolling,
- long spin-in-place tails,
- rail-generated / carried running english surviving too long.

## Important interpretation

There are really two different attenuation issues:

1. **Horizontal-spin / translational mismatch attenuation**
   - driven mainly by the sliding-friction term
   - this controls how aggressively follow / draw / overspin / draw-like rail exits settle toward
     rolling

2. **Pure z-spin attenuation**
   - driven by the vertical-axis spin-decay term
   - this controls how long residual side spin survives after the translational state is already
     rolling or spinning in place

That means:

- if the concern is exaggerated **follow / draw post-contact bend**, the main suspect is probably
  **sliding friction**, not z-spin decay;
- if the concern is exaggerated lingering **side spin / rail english / spin-in-place tails**, the
  main suspect is probably **z-spin decay**.

## Current conclusion

Yes: the current solver is very plausibly under-damping cloth-driven spin effects.

Most likely ranking:

1. **sliding friction is still meaningfully low** relative to the whitepapers, even after the
   midrange bump to `15 ips²`;
2. **rolling resistance is not the main problem**;
3. the earlier **z-spin decay mismatch** was real, but the preview / example defaults now use the
   calibrated `10.9 rad/s²` value.

## Probe-backed calibration pass

The following probe sweeps were run with `src/bin/shot_probe.rs`:

- baseline no-side follow / stun / draw suite:
  - `/tmp/billiards-cut-probes-baseline-2026-04-14-c`
- stronger sliding-friction suite, keeping roll and z-spin decay fixed:
  - `/tmp/billiards-cut-probes-slide20-2026-04-14`
  - `/tmp/billiards-cut-probes-slide77-2026-04-14`
- side-spin stun suite for z-spin calibration:
  - baseline: `/tmp/billiards-cut-probes-side-spin-baseline-2026-04-14`
  - faster z-spin decay: `/tmp/billiards-cut-probes-side-spin-alpha10p9-2026-04-14`

### What the z-spin calibration changed

Using the side-spin stun suite with `side_offset = 0.25R`:

- raising z-spin decay from `2` to `10.9 rad/s²` reduced the mean total shot elapsed time from
  about `19.67 s` to about `4.51 s`;
- cue-path geometry and bend metrics changed only slightly in that suite.

That calibration has now been adopted as the preview / example default.

A checked-in gallery scenario also improved in the expected way:

- `examples/scenarios/right_spin_stun_side_pocket.billiards`
  - previous long-tail behavior had the cue still spinning until about `t = 18.788 s`
  - with the calibrated z-spin decay, the cue now reaches `Rolling -> Rest` at about
    `t = 4.555 s`

Interpretation:

- the previous `2 rad/s²` value was indeed causing very long residual side-spin / spin-tail
  behavior;
- increasing z-spin decay fixes that tail much more than it changes the main post-contact path.

### What the sliding-friction calibration changed

Using the no-side follow / stun / draw suite:

- increasing sliding friction from `5` to `20 ips²` materially shortened cue paths and total shot
  times, especially for draw / force-follow shots;
- increasing it all the way to `77.2 ips²` produced very strong damping and qualitatively changed
  several shot outcomes, including extra cue scratches / pockets in the fixed side-pocket geometry.

Important nuance:

- these probe shots have non-zero travel **before** the cue reaches the object ball, so stronger
  sliding friction also reduces pre-impact cue speed for stun / draw shots;
- therefore a sliding-friction sweep is not only a post-impact bend calibration, it also changes
  the cue-ball arrival state at contact.

### Gentler sliding-friction candidates

Two smaller sliding-friction candidates were also checked against the no-side follow / stun / draw
suite:

- `10 ips²`
- `15 ips²`

Both materially shorten draw / force-follow travel compared with the current `5 ips²` default.
For example, mean cue-path length changed approximately as follows:

- force-follow: `85.65 in` baseline -> `69.66 in` at `10` -> `67.23 in` at `15`
- draw: `79.56 in` baseline -> `61.29 in` at `10` -> `53.09 in` at `15`

On this fixed probe family:

- baseline produced `1 / 75` cue scratch / pocket outcome,
- `10 ips²` produced `2 / 75`,
- `15 ips²` produced `1 / 75`.

A separate clean-worktree verification pass then checked the `15 ips²` candidate against the key
scenario regressions:

- `right_spin_stun_side_pocket_example_runs_end_to_end`
- `long_cut_top_right_rail_example_runs_end_to_end`
- `two_rail_bank_scratch_example_runs_end_to_end`

and rerendered:

- `/tmp/billiards-right-spin-stun-slide15.png`
- `/tmp/billiards-long-cut-slide15.png`
- `/tmp/billiards-two-rail-slide15.png`

That pass kept all three key scenarios behaving sensibly, so `15 ips²` has now been adopted as the
current preview / example sliding-friction default.

### Practical takeaway from the first pass

- **z-spin decay** was clearly too weak, and the preview / example defaults now use the calibrated
  `10.9 rad/s²` value;
- **sliding friction** also looked too weak; a midrange bump to `15 ips²` shortened follow / draw
  travel materially while preserving the checked scenario regressions, so that value is now the
  current preview / example default;
- even after that bump, the current reduced solver still sits well below the literal
  `mu_s ≈ 0.2` / `77.2 ips²` whitepaper mapping.

## Focused phase-timing audit: Rolling -> Spinning -> Rest

A separate code audit compared the current pure-z-spin transition logic against both the local
TP B.2 / Petit-style notes and the installed `pooltool` event scheduler.

Current status: **no additional z-spin timing fix looks justified right now**.

Why:

- the solver already uses linear z-spin decay during sliding, rolling, and spinning, matching the
  local `(M13)` / `(M14')` picture and `pooltool`'s
  `alpha = 5 * u_sp * g / (2 * R)` implementation;
- the rolling-phase transition logic already matches the `pooltool` rule:
  - let `t_roll = |v| / a_roll`
  - let `t_spin = |ωz| / αz`
  - if `t_spin > t_roll`, transition `Rolling -> Spinning`
  - otherwise transition `Rolling -> Rest` at the linear stop time;
- with the current preview / example defaults (`15 ips²`, `5 ips²`, `10.9 rad/s²`), the checked
  `right_spin_stun_side_pocket` scenario currently ends with `cue Rolling -> Rest` at about
  `t = 3.708 s`, with no separate `Spinning -> Rest` tail.

Interpretation:

- the earlier long-tail mismatch was primarily the old under-damped z-spin coefficient;
- after the `10.9 rad/s²` calibration, the remaining pure-z-spin phase timing is broadly aligned
  with the local whitepaper / `pooltool` picture;
- if future examples still look wrong, the more likely remaining culprit is not the vertical-axis
  decay law itself but other shot-state inputs into the post-contact motion.

## Suggested calibration order

1. Keep the calibrated **z-spin decay** default at `10.9 rad/s²`.
2. Keep the current **midrange sliding-friction** default at `15 ips²` unless a later pass finds a
   better tradeoff.
3. Only then revisit rolling resistance if the probe results still look wrong.

## Bug-hunt follow-up: straight follow / draw regression boundary

A focused follow-up check was run against the exact checked-in straight follow / draw side-pocket
examples while varying only the preview/example sliding-friction setting.

Observed boundary:

- `16` through `19 ips²` still preserve the current qualitative example outcomes;
- `20 ips²` is the first nearby bump that materially changes them.

At `20 ips²`:

- `examples/scenarios/straight_follow_side_pocket.billiards`
  reaches `cue Sliding -> Rolling` **before** the cue/object collision, so the cue arrives in a
  different contact state than the intended force-follow example;
- `examples/scenarios/straight_draw_side_pocket.billiards`
  no longer scratches in `center-left` and instead rolls to rest on the table.

That result matters because the local whitepaper / `pooltool` picture still argues that literal
ball-cloth `μs ≈ 0.2` is much stronger than the current reduced preview setting, but the reduced
solver is being tuned to preserve plausible example behavior, not to copy the raw coefficient
numerically.

So the present no-code conclusion is:

- the sliding-evolution math already matches the local Coulomb / `pooltool` shape closely enough
  for this scope;
- the remaining mismatch is primarily a **calibration tradeoff**, not a fresh implementation bug;
- the current preview/example default should stay at **`15 ips²`** for now.
