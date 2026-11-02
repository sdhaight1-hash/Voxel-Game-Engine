# Typed material node ports — initial Phase 2 adapter

This extends the **existing** `Graph`, `NodeKind` and Rust CPU material sampler.
It does not create a parallel texture pipeline or alter the four saved material
output channel numbers (0 Albedo, 1 Roughness, 2 Metallic, 3 Height).

## Authoring a useful typed graph

In **Nodes**, place an old **Noise** generator, then add **Luminance**,
**Levels**, and **Material Output**. Connect:

```text
Noise (Color) -> Luminance (Color -> Scalar)
              -> Levels (Scalar -> Scalar) -> Material Output (legacy RGB)
```

Luminance explicitly applies the existing color-to-grayscale weights.
Levels exposes three independent normalized parameters: `black`,
`white` and `gamma`; black must be strictly below white. Scalar values
broadcast as equal RGB components when connected to a legacy Color socket.
A bare Color→Levels connection is rejected with a socket-type error rather
than silently selecting a channel or doing an undocumented conversion.

The simpler `Scalar` node supplies a constant scalar useful for masks and
parameter experimentation. Its output can also connect directly to the
existing material output without changing that output's serialization.

## Scalar masks and visual type feedback

A `Threshold Mask` node accepts **Scalar** input (typically from `Luminance`,
`Levels` or `Scalar`). It exposes two serializable parameters:

- `threshold` (0..1): where the mask changes from black to white.
- `softness` (0..1): 0 produces a hard cutoff; positive values apply a
  deterministic cubic smoothstep to normalized edge coverage.

Its output is typed `Mask`. Safe `Mask→Scalar` and `Mask→Color` cables
are supported, preserving the existing RGB evaluation boundary. **Mask does
not silently modify texture alpha.** In foliage, connect a mask to the
existing Leaf Cutout material graph to drive that role's opacity handling.

The Nodes editor now shows typed internal socket/wire colors:
Color (blue), Scalar (mint) and Mask (amber). The four PBR Material Output
sockets retain their original channel-identification colors. In the node
inspector, expand **Socket types and parameter definitions** to read the
stable port IDs, parameter IDs, ranges, integer steps, defaults and units.
This is a read-only reference, not a second control panel.

All legacy parameterized `NodeKind` variants now expose parameter metadata,
including Checker (even tile counts), Bricks (even row counts), Noise, Voronoi,
Strata, Blend, Color, Colorize and every `LibraryNode` operation. The shared
Library operations still expose their existing normalized `amount` and
`seed` controls; metadata does **not** imply that every library operation
has unique bespoke parameters.

## Contract and compatibility

- `NodeKind` keeps the legacy serde enum representation, numeric node IDs,
  link `input` indices and RGB evaluation behavior. New variants are additive.
- `NodeKind::type_id()`, `input_definition`, `output_definition` and
  `parameters()` provide stable, named authoring metadata; parameter
  descriptors include control type, valid range, unit and default where
  implemented. Definitions are not serialized into saved graphs.
- All 76 preexisting `LibraryNode` operations continue exposing their
  original Color sockets. Existing roughness/metallic/height channels remain
  Color-typed **at the legacy boundary**; this deliberately avoids changing
  the interpretation of previously saved RGB textures.
- Valid automatic conversions are Scalar→Color and Mask→Scalar/Color.
  Color→Scalar, vectors→colors, and arbitrary Gradient/Curve casts are not
  implicit. Vector2, Vector3, Gradient and Curve are named types only here,
  **not functional authoring values** yet.
- Graph wiring and loading both enforce type rules. An invalid new link does
  not delete the existing cable or create a cycle.
- Foliage and Asset role graphs share `Graph` and can retain typed nodes
  without any new material owner, geometry engine or file format.

## Validation

Run `cargo test --all-targets`. Regression coverage includes old graph
JSON/pixel hashes, legacy Asset/Foliage/Flora imports, the 1,039 built-in
material recipes, all 43 Flora species, typed conversions, rejected invalid
links, and scalar Levels roundtrip/evaluation.

Not yet covered in this initial slice: rich parameters for every legacy
library operation, gradient/curve/vector values with user editing, type-coded
wire visuals, groups, subgraphs, caching or vector/SDF generators. These must
retain the same strict compatibility contract when implemented.
