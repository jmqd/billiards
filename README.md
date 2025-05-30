# Billiards

A tool and library for producing billiards diagrams and describing the
specifications of a game of pool/billiards. I might eventually add physics
simulations for pool.

For example, this diagram was created with the following code.

<img src="./img/nine-ball-example-hanger.png" alt="Diagram of a game of Nine Ball." style="width:50%"/>

``` rust
    let mut game_state = GameState {
        table_spec: TableSpec::brunswick_gc4_9ft(),
        ball_positions: vec![
            Ball {
                ty: BallType::Cue,
                position: CENTER_SPOT.clone(),
                spec: BallSpec::default(),
            },
            Ball {
                ty: BallType::Nine,
                position: Position {
                    x: Diamond::from("3.93"),
                    y: Diamond::from("7.93"),
                    ..Default::default()
                },
                spec: BallSpec::default(),
            },
        ],
        ..Default::default()
    };

    game_state.freeze_to_rail(
        Rail::Left,
        Diamond::six(),
        Ball {
            ty: BallType::Eight,
            position: Position::default(),
            spec: BallSpec::default(),
        },
    );

    let img = game_state.draw_2d_diagram();
    write_png_to_file(&img, None);
```

## Thanks

Thanks to Dr. Dave Alciatore of Colorado State University for providing the
blank pool table diagram, which I used as a base image.
