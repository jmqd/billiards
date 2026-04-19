# Cloth motion calibration notes

Working note comparing the current repo motion settings with the local whitepapers and the locally
installed `pooltool` defaults.

## Why this note exists

Recent probe sweeps suggest the cue ball can keep meaningful spin and post-contact action longer
than expected. This note checks whether the current cloth-motion parameters are plausibly too weak
at dissipating spin and spin-related sliding mismatch.

## Current repo motion settings used by the probes / examples

The current probe / example motion config uses:

- sliding friction acceleration: `5 ips²`
- rolling resistance deceleration: `5 ips²`
- z-spin angular deceleration: `2 rad/s²`

Because gravity is about `386.09 ips²`, these imply effective cloth coefficients of roughly:

- `mu_s ≈ 5 / 386.09 ≈ 0.013`
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
with the local references than the current `2 rad/s²`.

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

- current repo: `5 ips²` -> `mu_s ≈ 0.013`
- whitepapers / pooltool-like: `~77.2 ips²` -> `mu_s ≈ 0.2`

This means the current repo's sliding friction is roughly:

- **15x weaker** than the usual whitepaper / pooltool value

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

- current repo: `2 rad/s²`
- TP B.2 measured spin-down: `~10 rad/s²`
- pooltool default equivalent: `~10.9 rad/s²`

So the current repo's z-spin decay is roughly:

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

1. **sliding friction is much too low** relative to the whitepapers;
2. **z-spin decay is also much too low** relative to the whitepapers and `pooltool`;
3. **rolling resistance is not the main problem**.

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

Interpretation:

- the current `2 rad/s²` value is indeed causing very long residual side-spin / spin-tail behavior;
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

### Practical takeaway from the first pass

- **z-spin decay** looks clearly too weak and can likely be increased substantially without
  destabilizing the main cue-ball trajectory shape;
- **sliding friction** also looks too weak, but jumping straight from `5` to a literal
  `mu_s ≈ 0.2` / `77.2 ips²` mapping likely over-corrects inside the current reduced solver;
- a more realistic next step is probably a **midrange sliding-friction calibration** rather than a
  direct jump to the full whitepaper coefficient.

## Suggested calibration order

1. Raise **z-spin decay** first; this looks like the cleanest mismatch.
2. Then run a **midrange sliding-friction** calibration pass on the no-side follow / stun / draw
   suite.
3. Only then revisit rolling resistance if the probe results still look wrong.
