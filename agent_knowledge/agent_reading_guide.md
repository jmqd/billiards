# Agent Reading Guide for Billiards Whitepapers

This is the *distilled* agent-facing overview. Use it first, then drop into the
JSONL/corpus files for deeper retrieval.

## What to read first

Use this order if you are trying to quickly grok the current billiards physics model:

1. `whitepapers/a_theoretical_analysis_of_billiard_ball_dynamics_under_cushion_impacts.pdf` — A Theoretical Analysis Of Billiard Ball Dynamics Under Cushion Impacts
1. `whitepapers/collision_of_billiard_balls_in_3d_with_spin_and_friction.pdf` — Collision of Billiard Balls in 3D with Spin and Friction Charles S. Peskin
1. `whitepapers/everything_you_always_wanted_to_know_about_cue_ball_squirt_but_were_afraid_to_ask.pdf` — Everything You Always Wanted to Know About Cue Ball Squirt, But Were Afraid to Ask
1. `whitepapers/motions_of_a_billiard_ball_after_a_cue_stroke.pdf` — Motions of a billiard ball after a cue stroke Hyeong-Chan Kim1
1. `whitepapers/non_smooth_modelling_of_billiard_and_superbilliard_ball_collisions.pdf` — Non Smooth Modelling Of Billiard And Superbilliard Ball Collisions
1. `whitepapers/numerical_simulations_of_the_frictional_collisions_of_solid_balls_on_a_rough_surface.pdf` — Numerical Simulations Of The Frictional Collisions Of Solid Balls On A Rough Surface
1. `whitepapers/pool_and_billiards_physics_principles_by_coriolis_and_others.pdf` — Pool and Billiards Physics Principles by Coriolis and Others
1. `whitepapers/rolling_motion_of_a_ball_spinning_about_a_near_vertical_axis.pdf` — Rolling motion of a ball spinning about a near vertical axis
1. `whitepapers/sliding_and_rolling_the_physics_of_a_rolling_ball.pdf` — Sliding and rolling: the physics of a rolling ball
1. `whitepapers/the_art_of_billiards_play.html` — The Art of Billiards Play
1. `whitepapers/the_physics_of_billiards.html` — The Physics Of Billiards
1. `whitepapers/tp_3_1_90_degree_rule.pdf` — TP 3.1 - 90 degree Rule
1. `whitepapers/tp_3_3_30_degree_rule.pdf` — TP 3.3 - 30° rule
1. `whitepapers/tp_3_4_margin_of_error_based_on_distance_and_cut_angle.pdf` — TP 3.4 - Margin of error based on distance and cut angle
1. `whitepapers/tp_3_5_effective_target_sizes_for_slow_shots_into_a_side_pocket_at_different_angles.pdf` — TP 3.5 - Effective Target Sizes For Slow Shots Into A Side Pocket At Different Angles
1. `whitepapers/tp_3_6_effective_target_sizes_for_slow_shots_into_a_corner_pocket_at_different_angles.pdf` — TP 3.6 - Effective Target Sizes For Slow Shots Into A Corner Pocket At Different Angles
1. `whitepapers/tp_3_7_effective_target_sizes_for_fast_shots_into_a_side_pocket_at_different_angles.pdf` — TP 3.7 - Effective Target Sizes For Fast Shots Into A Side Pocket At Different Angles
1. `whitepapers/tp_3_8_effective_target_sizes_for_fast_shots_into_a_corner_pocket_at_different_angles.pdf` — TP 3.8 - Effective Target Sizes For Fast Shots Into A Corner Pocket At Different Angles
1. `whitepapers/tp_4_2_center_of_percussion_of_the_cue_ball.pdf` — TP 4.2 - Center of percussion of the cue ball
1. `whitepapers/tp_a_24_the_effects_of_follow_and_draw_on_throw_and_ob_swerve.pdf` — TP A.24 - The effects of follow and draw on throw
1. `whitepapers/tp_a_4_post_impact_cue_ball_trajectory_for_any_cut_angle_speed_and_spin.pdf` — TP A.4 - Post-impact cue ball trajectory for any cut angle, speed, and spin
1. `whitepapers/tp_b_1_squirt_angle_pivot_length_and_tip_size.pdf` — TP B.1 - Squirt angle, pivot length, and tip shape

## Quick stats

- Documents indexed: 427
- Documents cited by current repo code/docs (including TODO old→new mappings): 20
- Approx extracted text size: 5,765,368 characters
- Formula-like candidate lines harvested: 2,724

## Code- and doc-cited sources

- `whitepapers/30_degree_rule_for_caroms.html` — 30-Degree Rule for Caroms - December 2009 [code]
- `whitepapers/collision_of_billiard_balls_in_3d_with_spin_and_friction.pdf` — Collision of Billiard Balls in 3D with Spin and Friction Charles S. Peskin [code, docs]
- `whitepapers/motions_of_a_billiard_ball_after_a_cue_stroke.pdf` — Motions of a billiard ball after a cue stroke Hyeong-Chan Kim1 [code, docs]
- `whitepapers/non_smooth_modelling_of_billiard_and_superbilliard_ball_collisions.pdf` — Non Smooth Modelling Of Billiard And Superbilliard Ball Collisions [code, docs]
- `whitepapers/pool_and_billiards_physics_principles_by_coriolis_and_others.pdf` — Pool and Billiards Physics Principles by Coriolis and Others [code, docs]
- `whitepapers/publications_presentations_and_software_index.html` — Publications, Presentations, and Software - Dr. David G. Alciatore [code]
- `whitepapers/tp_4_2_center_of_percussion_of_the_cue_ball.pdf` — TP 4.2 - Center of percussion of the cue ball [code]
- `whitepapers/tp_a_4_post_impact_cue_ball_trajectory_for_any_cut_angle_speed_and_spin.pdf` — TP A.4 - Post-impact cue ball trajectory for any cut angle, speed, and spin [code]
- `whitepapers/a_theoretical_analysis_of_billiard_ball_dynamics_under_cushion_impacts.pdf` — A Theoretical Analysis Of Billiard Ball Dynamics Under Cushion Impacts [docs]
- `whitepapers/amateur_physics_for_the_amateur_pool_player.pdf` — APAPP 4 of 4 [docs]
- `whitepapers/toward_a_competitive_pool_playing_robot.pdf` — C O V E R F E AT U R E Toward a [docs]
- `whitepapers/computational_pool_an_or_optimization_point_of_view.pdf` — Computational pool: an OR-optimization point of view [docs]
- `whitepapers/dynamics_in_carom_three_cushion.pdf` — Dynamics In Carom Three Cushion [docs]
- `whitepapers/everything_you_always_wanted_to_know_about_cue_ball_squirt_but_were_afraid_to_ask.pdf` — Everything You Always Wanted to Know About Cue Ball Squirt, But Were Afraid to Ask [docs]
- `whitepapers/numerical_simulations_of_the_frictional_collisions_of_solid_balls_on_a_rough_surface.pdf` — Numerical Simulations Of The Frictional Collisions Of Solid Balls On A Rough Surface [docs]
- `whitepapers/robotic_billiards_understanding_humans_in_order_to_counter_them.pdf` — Robotic Billiards: Understanding Humans in Order to Counter Them [docs]
- `whitepapers/robotic_pool_an_experiment_in_automatic_potting.pdf` — Robotic Pool An Experiment In Automatic Potting [docs]
- `whitepapers/rolling_friction_intro.pdf` — Rolling Friction Intro [docs]
- `whitepapers/sliding_and_rolling_the_physics_of_a_rolling_ball.pdf` — Sliding and rolling: the physics of a rolling ball [docs]
- `whitepapers/theorie_mathematique_des_effets_du_jeu_de_billard_par_g_coriolis.pdf` — Théorie mathématique des effets du jeu de billard / par G. Coriolis [docs]

## Topic map (top docs only)

Each section shows the highest-signal docs first; use `whitepapers_index.jsonl`
for the exhaustive list.

### Aiming And Potting

- `whitepapers/tp_a_4_post_impact_cue_ball_trajectory_for_any_cut_angle_speed_and_spin.pdf` — TP A.4 - Post-impact cue ball trajectory for any cut angle, speed, and spin [starter | code-cited | formula-lines:40]
- `whitepapers/non_smooth_modelling_of_billiard_and_superbilliard_ball_collisions.pdf` — Non Smooth Modelling Of Billiard And Superbilliard Ball Collisions [starter | code-cited | formula-lines:40]
- `whitepapers/everything_you_always_wanted_to_know_about_cue_ball_squirt_but_were_afraid_to_ask.pdf` — Everything You Always Wanted to Know About Cue Ball Squirt, But Were Afraid to Ask [starter | doc-cited | formula-lines:40]
- `whitepapers/a_theoretical_analysis_of_billiard_ball_dynamics_under_cushion_impacts.pdf` — A Theoretical Analysis Of Billiard Ball Dynamics Under Cushion Impacts [starter | doc-cited | formula-lines:40]
- `whitepapers/pool_and_billiards_physics_principles_by_coriolis_and_others.pdf` — Pool and Billiards Physics Principles by Coriolis and Others [starter | code-cited | formula-lines:24]
- `whitepapers/numerical_simulations_of_the_frictional_collisions_of_solid_balls_on_a_rough_surface.pdf` — Numerical Simulations Of The Frictional Collisions Of Solid Balls On A Rough Surface [starter | doc-cited | formula-lines:23]
- `whitepapers/tp_b_1_squirt_angle_pivot_length_and_tip_size.pdf` — TP B.1 - Squirt angle, pivot length, and tip shape [starter | formula-lines:40]
- `whitepapers/tp_a_24_the_effects_of_follow_and_draw_on_throw_and_ob_swerve.pdf` — TP A.24 - The effects of follow and draw on throw [starter | formula-lines:40]
- `whitepapers/tp_3_8_effective_target_sizes_for_fast_shots_into_a_corner_pocket_at_different_angles.pdf` — TP 3.8 - Effective Target Sizes For Fast Shots Into A Corner Pocket At Different Angles [starter | formula-lines:40]
- `whitepapers/tp_3_7_effective_target_sizes_for_fast_shots_into_a_side_pocket_at_different_angles.pdf` — TP 3.7 - Effective Target Sizes For Fast Shots Into A Side Pocket At Different Angles [starter | formula-lines:40]
- `whitepapers/tp_3_6_effective_target_sizes_for_slow_shots_into_a_corner_pocket_at_different_angles.pdf` — TP 3.6 - Effective Target Sizes For Slow Shots Into A Corner Pocket At Different Angles [starter | formula-lines:40]
- `whitepapers/tp_3_5_effective_target_sizes_for_slow_shots_into_a_side_pocket_at_different_angles.pdf` — TP 3.5 - Effective Target Sizes For Slow Shots Into A Side Pocket At Different Angles [starter | formula-lines:40]
- ... 300 more in `whitepapers_index.jsonl`

### Banks Kicks Rails

- `whitepapers/a_theoretical_analysis_of_billiard_ball_dynamics_under_cushion_impacts.pdf` — A Theoretical Analysis Of Billiard Ball Dynamics Under Cushion Impacts [starter | doc-cited | formula-lines:40]
- `whitepapers/tp_3_7_effective_target_sizes_for_fast_shots_into_a_side_pocket_at_different_angles.pdf` — TP 3.7 - Effective Target Sizes For Fast Shots Into A Side Pocket At Different Angles [starter | formula-lines:40]
- `whitepapers/the_art_of_billiards_play.html` — The Art of Billiards Play [starter | formula-lines:40]
- `whitepapers/tp_b_13_rolling_cue_ball_carom_angle_approximations.pdf` — TP B.13 - Rolling CB Carom Angle Approximations [formula-lines:40]
- `whitepapers/tp_b_6_cue_ball_table_lengths_of_travel_for_different_speeds_accounting_for_rail_rebound_and_drag_losses.pdf` — TP B.6 - CB table lengths of travel for different speeds [formula-lines:36]
- `whitepapers/billiard_university_bu_part_iv_table_difficulty.pdf` — Billiard University BU Part Iv Table Difficulty [formula-lines:22]
- `whitepapers/tp_b_11_shallow_angle_contact_point_mirror_kick_system.pdf` — TP B.11 - Shallow-angle contact-point mirror kick system [formula-lines:21]
- `whitepapers/veps_gems_part_xii_corner_5_system_example_and_benchmark.pdf` — VEPS GEMS - Part XII: Corner-5 System Example and Benchmark [formula-lines:19]
- `whitepapers/tp_7_3_ball_rail_interaction_and_the_effects_on_vertical_plane_spin.pdf` — TP 7.3 - Ball-rail interaction and the effects on vertical plane spin [formula-lines:18]
- `whitepapers/veps_gems_part_xi_corner_5_system_intro.pdf` — VEPS GEMS - Part XI: Corner-5 System Intro [formula-lines:13]
- `whitepapers/tp_b_27_sliding_bank_system_comparison.pdf` — TP B.27 - Sliding Bank System Comparison [formula-lines:13]
- `whitepapers/tp_7_2_corner_5_three_rail_diamond_system_formulas.pdf` — TP 7.2 - Corner-5 three-rail diamond system formulas [formula-lines:10]
- ... 161 more in `whitepapers_index.jsonl`

### Collisions And Impacts

- `whitepapers/tp_a_4_post_impact_cue_ball_trajectory_for_any_cut_angle_speed_and_spin.pdf` — TP A.4 - Post-impact cue ball trajectory for any cut angle, speed, and spin [starter | code-cited | formula-lines:40]
- `whitepapers/non_smooth_modelling_of_billiard_and_superbilliard_ball_collisions.pdf` — Non Smooth Modelling Of Billiard And Superbilliard Ball Collisions [starter | code-cited | formula-lines:40]
- `whitepapers/motions_of_a_billiard_ball_after_a_cue_stroke.pdf` — Motions of a billiard ball after a cue stroke Hyeong-Chan Kim1 [starter | code-cited | formula-lines:40]
- `whitepapers/everything_you_always_wanted_to_know_about_cue_ball_squirt_but_were_afraid_to_ask.pdf` — Everything You Always Wanted to Know About Cue Ball Squirt, But Were Afraid to Ask [starter | doc-cited | formula-lines:40]
- `whitepapers/collision_of_billiard_balls_in_3d_with_spin_and_friction.pdf` — Collision of Billiard Balls in 3D with Spin and Friction Charles S. Peskin [starter | code-cited | formula-lines:40]
- `whitepapers/a_theoretical_analysis_of_billiard_ball_dynamics_under_cushion_impacts.pdf` — A Theoretical Analysis Of Billiard Ball Dynamics Under Cushion Impacts [starter | doc-cited | formula-lines:40]
- `whitepapers/pool_and_billiards_physics_principles_by_coriolis_and_others.pdf` — Pool and Billiards Physics Principles by Coriolis and Others [starter | code-cited | formula-lines:24]
- `whitepapers/numerical_simulations_of_the_frictional_collisions_of_solid_balls_on_a_rough_surface.pdf` — Numerical Simulations Of The Frictional Collisions Of Solid Balls On A Rough Surface [starter | doc-cited | formula-lines:23]
- `whitepapers/sliding_and_rolling_the_physics_of_a_rolling_ball.pdf` — Sliding and rolling: the physics of a rolling ball [starter | doc-cited | formula-lines:21]
- `whitepapers/tp_4_2_center_of_percussion_of_the_cue_ball.pdf` — TP 4.2 - Center of percussion of the cue ball [starter | code-cited | formula-lines:10]
- `whitepapers/tp_a_24_the_effects_of_follow_and_draw_on_throw_and_ob_swerve.pdf` — TP A.24 - The effects of follow and draw on throw [starter | formula-lines:40]
- `whitepapers/the_art_of_billiards_play.html` — The Art of Billiards Play [starter | formula-lines:40]
- ... 225 more in `whitepapers_index.jsonl`

### Cue Ball Motion And Spin

- `whitepapers/tp_a_4_post_impact_cue_ball_trajectory_for_any_cut_angle_speed_and_spin.pdf` — TP A.4 - Post-impact cue ball trajectory for any cut angle, speed, and spin [starter | code-cited | formula-lines:40]
- `whitepapers/non_smooth_modelling_of_billiard_and_superbilliard_ball_collisions.pdf` — Non Smooth Modelling Of Billiard And Superbilliard Ball Collisions [starter | code-cited | formula-lines:40]
- `whitepapers/motions_of_a_billiard_ball_after_a_cue_stroke.pdf` — Motions of a billiard ball after a cue stroke Hyeong-Chan Kim1 [starter | code-cited | formula-lines:40]
- `whitepapers/everything_you_always_wanted_to_know_about_cue_ball_squirt_but_were_afraid_to_ask.pdf` — Everything You Always Wanted to Know About Cue Ball Squirt, But Were Afraid to Ask [starter | doc-cited | formula-lines:40]
- `whitepapers/collision_of_billiard_balls_in_3d_with_spin_and_friction.pdf` — Collision of Billiard Balls in 3D with Spin and Friction Charles S. Peskin [starter | code-cited | formula-lines:40]
- `whitepapers/a_theoretical_analysis_of_billiard_ball_dynamics_under_cushion_impacts.pdf` — A Theoretical Analysis Of Billiard Ball Dynamics Under Cushion Impacts [starter | doc-cited | formula-lines:40]
- `whitepapers/pool_and_billiards_physics_principles_by_coriolis_and_others.pdf` — Pool and Billiards Physics Principles by Coriolis and Others [starter | code-cited | formula-lines:24]
- `whitepapers/numerical_simulations_of_the_frictional_collisions_of_solid_balls_on_a_rough_surface.pdf` — Numerical Simulations Of The Frictional Collisions Of Solid Balls On A Rough Surface [starter | doc-cited | formula-lines:23]
- `whitepapers/sliding_and_rolling_the_physics_of_a_rolling_ball.pdf` — Sliding and rolling: the physics of a rolling ball [starter | doc-cited | formula-lines:21]
- `whitepapers/tp_4_2_center_of_percussion_of_the_cue_ball.pdf` — TP 4.2 - Center of percussion of the cue ball [starter | code-cited | formula-lines:10]
- `whitepapers/tp_b_1_squirt_angle_pivot_length_and_tip_size.pdf` — TP B.1 - Squirt angle, pivot length, and tip shape [starter | formula-lines:40]
- `whitepapers/tp_a_24_the_effects_of_follow_and_draw_on_throw_and_ob_swerve.pdf` — TP A.24 - The effects of follow and draw on throw [starter | formula-lines:40]
- ... 344 more in `whitepapers_index.jsonl`

### History And General Physics

- `whitepapers/non_smooth_modelling_of_billiard_and_superbilliard_ball_collisions.pdf` — Non Smooth Modelling Of Billiard And Superbilliard Ball Collisions [starter | code-cited | formula-lines:40]
- `whitepapers/motions_of_a_billiard_ball_after_a_cue_stroke.pdf` — Motions of a billiard ball after a cue stroke Hyeong-Chan Kim1 [starter | code-cited | formula-lines:40]
- `whitepapers/a_theoretical_analysis_of_billiard_ball_dynamics_under_cushion_impacts.pdf` — A Theoretical Analysis Of Billiard Ball Dynamics Under Cushion Impacts [starter | doc-cited | formula-lines:40]
- `whitepapers/pool_and_billiards_physics_principles_by_coriolis_and_others.pdf` — Pool and Billiards Physics Principles by Coriolis and Others [starter | code-cited | formula-lines:24]
- `whitepapers/the_art_of_billiards_play.html` — The Art of Billiards Play [starter | formula-lines:40]
- `whitepapers/the_physics_of_billiards.html` — The Physics Of Billiards [starter | formula-lines:28]
- `whitepapers/theorie_mathematique_des_effets_du_jeu_de_billard_par_g_coriolis.pdf` — Théorie mathématique des effets du jeu de billard / par G. Coriolis [doc-cited | formula-lines:40]
- `whitepapers/amateur_physics_for_the_amateur_pool_player.pdf` — APAPP 4 of 4 [doc-cited | formula-lines:40]
- `whitepapers/collision_of_two_spinning_billiard_balls_and_the_role_of_table_friction.pdf` — Collision of two spinning billiard balls and the role of table [formula-lines:40]
- `whitepapers/application_of_high_speed_imaging_to_determine_the_dynamics_of_billiards.pdf` — Application of high-speed imaging to determine the dynamics of billiards [formula-lines:14]
- `whitepapers/the_amazing_world_of_billiards_physics.pdf` — The Amazing World of Billiards Physics May, 2007 [formula-lines:8]
- `whitepapers/mechanics_of_billiards_and_analysis_of_willie_hoppe_s_stroke.pdf` — MECHANICS OF BILLIARDS, AND ANALYSIS OF WILLIE HOPPE'S STROKE [formula-lines:7]
- ... 16 more in `whitepapers_index.jsonl`

### Robotics And Computation

- `whitepapers/motions_of_a_billiard_ball_after_a_cue_stroke.pdf` — Motions of a billiard ball after a cue stroke Hyeong-Chan Kim1 [starter | code-cited | formula-lines:40]
- `whitepapers/a_theoretical_analysis_of_billiard_ball_dynamics_under_cushion_impacts.pdf` — A Theoretical Analysis Of Billiard Ball Dynamics Under Cushion Impacts [starter | doc-cited | formula-lines:40]
- `whitepapers/numerical_simulations_of_the_frictional_collisions_of_solid_balls_on_a_rough_surface.pdf` — Numerical Simulations Of The Frictional Collisions Of Solid Balls On A Rough Surface [starter | doc-cited | formula-lines:23]
- `whitepapers/robotic_billiards_understanding_humans_in_order_to_counter_them.pdf` — Robotic Billiards: Understanding Humans in Order to Counter Them [doc-cited | formula-lines:40]
- `whitepapers/computational_pool_an_or_optimization_point_of_view.pdf` — Computational pool: an OR-optimization point of view [doc-cited | formula-lines:29]
- `whitepapers/robotic_pool_an_experiment_in_automatic_potting.pdf` — Robotic Pool An Experiment In Automatic Potting [doc-cited | formula-lines:21]
- `whitepapers/toward_a_competitive_pool_playing_robot.pdf` — C O V E R F E AT U R E Toward a [doc-cited | formula-lines:2]
- `whitepapers/publications_presentations_and_software_index.html` — Publications, Presentations, and Software - Dr. David G. Alciatore [code-cited | formula-lines:1]
- `whitepapers/effects_of_material_properties_of_cue_on_ball_trajectory_in_billiards.pdf` — JSME-TJ [formula-lines:40]
- `whitepapers/collision_of_two_spinning_billiard_balls_and_the_role_of_table_friction.pdf` — Collision of two spinning billiard balls and the role of table [formula-lines:40]
- `whitepapers/tp_b_29_simulation_of_a_cb_striking_two_frozen_obs_along_their_line_of_centers.pdf` — TP B.29 - Simulation of a CB striking two frozen OBs along their line of centers [formula-lines:26]
- `whitepapers/application_of_high_speed_imaging_to_determine_the_dynamics_of_billiards.pdf` — Application of high-speed imaging to determine the dynamics of billiards [formula-lines:14]
- ... 12 more in `whitepapers_index.jsonl`

### Strategy Rules Drills

- `whitepapers/motions_of_a_billiard_ball_after_a_cue_stroke.pdf` — Motions of a billiard ball after a cue stroke Hyeong-Chan Kim1 [starter | code-cited | formula-lines:40]
- `whitepapers/a_theoretical_analysis_of_billiard_ball_dynamics_under_cushion_impacts.pdf` — A Theoretical Analysis Of Billiard Ball Dynamics Under Cushion Impacts [starter | doc-cited | formula-lines:40]
- `whitepapers/pool_and_billiards_physics_principles_by_coriolis_and_others.pdf` — Pool and Billiards Physics Principles by Coriolis and Others [starter | code-cited | formula-lines:24]
- `whitepapers/numerical_simulations_of_the_frictional_collisions_of_solid_balls_on_a_rough_surface.pdf` — Numerical Simulations Of The Frictional Collisions Of Solid Balls On A Rough Surface [starter | doc-cited | formula-lines:23]
- `whitepapers/sliding_and_rolling_the_physics_of_a_rolling_ball.pdf` — Sliding and rolling: the physics of a rolling ball [starter | doc-cited | formula-lines:21]
- `whitepapers/tp_b_1_squirt_angle_pivot_length_and_tip_size.pdf` — TP B.1 - Squirt angle, pivot length, and tip shape [starter | formula-lines:40]
- `whitepapers/the_art_of_billiards_play.html` — The Art of Billiards Play [starter | formula-lines:40]
- `whitepapers/rolling_motion_of_a_ball_spinning_about_a_near_vertical_axis.pdf` — Rolling motion of a ball spinning about a near vertical axis [starter | formula-lines:37]
- `whitepapers/tp_3_4_margin_of_error_based_on_distance_and_cut_angle.pdf` — TP 3.4 - Margin of error based on distance and cut angle [starter | formula-lines:23]
- `whitepapers/robotic_billiards_understanding_humans_in_order_to_counter_them.pdf` — Robotic Billiards: Understanding Humans in Order to Counter Them [doc-cited | formula-lines:40]
- `whitepapers/computational_pool_an_or_optimization_point_of_view.pdf` — Computational pool: an OR-optimization point of view [doc-cited | formula-lines:29]
- `whitepapers/robotic_pool_an_experiment_in_automatic_potting.pdf` — Robotic Pool An Experiment In Automatic Potting [doc-cited | formula-lines:21]
- ... 304 more in `whitepapers_index.jsonl`

### Technical Proofs

- `whitepapers/tp_a_4_post_impact_cue_ball_trajectory_for_any_cut_angle_speed_and_spin.pdf` — TP A.4 - Post-impact cue ball trajectory for any cut angle, speed, and spin [starter | code-cited | formula-lines:40]
- `whitepapers/tp_4_2_center_of_percussion_of_the_cue_ball.pdf` — TP 4.2 - Center of percussion of the cue ball [starter | code-cited | formula-lines:10]
- `whitepapers/tp_b_1_squirt_angle_pivot_length_and_tip_size.pdf` — TP B.1 - Squirt angle, pivot length, and tip shape [starter | formula-lines:40]
- `whitepapers/tp_a_24_the_effects_of_follow_and_draw_on_throw_and_ob_swerve.pdf` — TP A.24 - The effects of follow and draw on throw [starter | formula-lines:40]
- `whitepapers/tp_3_8_effective_target_sizes_for_fast_shots_into_a_corner_pocket_at_different_angles.pdf` — TP 3.8 - Effective Target Sizes For Fast Shots Into A Corner Pocket At Different Angles [starter | formula-lines:40]
- `whitepapers/tp_3_7_effective_target_sizes_for_fast_shots_into_a_side_pocket_at_different_angles.pdf` — TP 3.7 - Effective Target Sizes For Fast Shots Into A Side Pocket At Different Angles [starter | formula-lines:40]
- `whitepapers/tp_3_6_effective_target_sizes_for_slow_shots_into_a_corner_pocket_at_different_angles.pdf` — TP 3.6 - Effective Target Sizes For Slow Shots Into A Corner Pocket At Different Angles [starter | formula-lines:40]
- `whitepapers/tp_3_5_effective_target_sizes_for_slow_shots_into_a_side_pocket_at_different_angles.pdf` — TP 3.5 - Effective Target Sizes For Slow Shots Into A Side Pocket At Different Angles [starter | formula-lines:40]
- `whitepapers/tp_3_4_margin_of_error_based_on_distance_and_cut_angle.pdf` — TP 3.4 - Margin of error based on distance and cut angle [starter | formula-lines:23]
- `whitepapers/tp_3_1_90_degree_rule.pdf` — TP 3.1 - 90 degree Rule [starter | formula-lines:17]
- `whitepapers/tp_3_3_30_degree_rule.pdf` — TP 3.3 - 30° rule [starter | formula-lines:16]
- `whitepapers/tp_4_1_distance_required_for_stun_and_normal_roll_to_develop.pdf` — TP_4-1 [formula-lines:40]
- ... 341 more in `whitepapers_index.jsonl`

### Uncategorized

- `whitepapers/walker.pdf` — Walker
- `whitepapers/variable_focus_three_dimensional_laser_digitizing_system.pdf` — Variable Focus Three Dimensional Laser Digitizing System
- `whitepapers/the_french_fryin_legion_s_new_secret_weapon.pdf` — The French Fryin Legion S New Secret Weapon
- `whitepapers/the_best_least_squares_line_fit.pdf` — The Best Least Squares Line Fit
- `whitepapers/ryan_pooh_poohed_those_low_carb_diets.pdf` — Ryan Pooh Poohed Those Low Carb Diets
- `whitepapers/ryan_demonstrated_the_basics_of_torsion_mechanics.pdf` — Ryan Demonstrated The Basics Of Torsion Mechanics
- `whitepapers/richard_and_jen_ace_stochastics.pdf` — Richard And Jen Ace Stochastics
- `whitepapers/racking_up_the_physics_of_pool.pdf` — Racking Up The Physics Of Pool
- `whitepapers/put_a_bounce_in_your_step.pdf` — Put A Bounce In Your Step
- `whitepapers/pool_mythology_what_you_accept_as_truth_just_might_deserve_a_second_look.pdf` — Pool Mythology What You Accept As Truth Just Might Deserve A Second Look
- `whitepapers/multipulley_belt_drive_mechanics_creep_theory_vs_shear_theory.pdf` — Multipulley Belt Drive Mechanics Creep Theory Vs Shear Theory
- `whitepapers/model_development_and_control_implementation_for_a_magnetic_levitation_apparatus.pdf` — Model Development And Control Implementation For A Magnetic Levitation Apparatus
- ... 12 more in `whitepapers_index.jsonl`

## Notes

- `whitepapers_corpus.txt` is the full plain-text dump with per-document delimiters.
- `whitepapers_formula_candidates.txt` is a grep-like list of formula/equation candidates.
- `whitepapers_index.jsonl` is the machine-readable manifest for tools/agents.
