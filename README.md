# Billiards

A tool and library for producing billiards diagrams and describing the
specifications of a game of pool/billiards. I might eventually add physics
simulations for pool.

For example, this diagram was created with the following code.

![diagram of a game of nine ball](./img/nine-ball-example-hanger.png)

``` rust
    let table_spec = TableSpec::new_9ft_brunswick_gc4();
    let game_state = GameState {
        table_spec: table_spec.clone(),
        ball_positions: vec![
            Ball {
                ty: BallType::Cue,
                position: CENTER_SPOT.clone(),
                spec: BallSpec::default(),
            },
            Ball {
                ty: BallType::Nine,
                position: Position {
                    x: Diamond::from("3.65"),
                    y: Diamond::from("7.625"),
                },
                spec: BallSpec::default(),
            },
        ],
        ty: GameType::NineBall,
        cueball_modifier: CueballModifier::AsItLays,
    };

    let img = game_state.draw_2d_diagram();
    write_png_to_file(&img, None);
```

## Thanks

Thanks to Dr. Dave Billiards for providing the empty diagram of the pool table
on his website, which I used as a base image to generate diagrams from.
