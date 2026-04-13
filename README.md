# Billiards

A tool and library for producing billiards diagrams and describing the
specifications of a game of pool/billiards. I might eventually add physics
simulations for pool.

For example, the following diagram was created using the simple Domain Specific
Language (DSL) included in this project. The DSL allows you to describe table
setups and ball positions in a clean, human-readable text format.

<img src="./img/nine-ball-example-hanger.png" alt="Diagram of a game of Nine Ball." style="width:50%"/>

```text
# This can be in a file like table.billiards

# The coordinate system uses "diamonds", with the origin at bottom-left.
# `x` increases to the right and `y` increases upward in table space.

# Create a standard 9ft table (default)
table brunswick_gc4_9ft

# Place the cue ball at the center spot
ball cue at center

# Place the 9-ball at a specific coordinate
ball nine at (3.93, 7.93)

# Freeze the 8-ball to the left rail at diamond 6
ball eight frozen left (6.0)
```

## Thanks

Thanks to Dr. Dave Alciatore of Colorado State University for providing the
blank pool table diagram, which I used as a base image.
