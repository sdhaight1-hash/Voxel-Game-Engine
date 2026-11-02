# Editable Color Ramp — Issue #10 / Phase 4a

The Nodes editor has a new **Color Ramp** filter. It samples a typed
**Scalar** input in 0..1 and returns **Color** RGB. Use
`Noise → Luminance → Color Ramp → Material Output: Albedo` to convert
grayscale variations into a colored material, or use
`Scalar → Color Ramp` for a fixed sampled shade.

## Controls and serialization

- Starts with four editable woodland-color stops.
- Holds **two to eight** sorted stops. The first and last are pinned at
  normalized positions 0 and 1; interior positions and all colors are
  editable in the existing inspector. Add inserts at the midpoint of the
  largest interval; Remove is allowed only on interior stops.
- The inspector includes a live 64-sample color ramp strip. The renderer
  linearly interpolates existing display RGB color components, matching
  the earlier Colorize authoring convention; it does **not** introduce
  sRGB/linear conversions into legacy graphs.
- The bounded representation retains `NodeKind: Copy` and the existing
  Nodes UI design without dynamic node storage or new dependencies.
- Serialized graph nodes store the eight-slot ramp and active count
  directly in the new `ColorRamp` variant. Legacy enum variants and
  old graph JSON remain unchanged.
- Save validation rejects nonfinite, duplicate, unordered or off-endpoint
  active stop positions and counts outside 2..8.
- Typed sockets reject direct Color→Scalar implicit conversions; add
  Luminance explicitly for older RGB/noise generator nodes.

## Tree and season ownership

The new ramp can be used inside Bark and Leaf authoring graphs: Bark uses
its RGBA pixels; Leaf uses graph luminance as shading while the species
and runtime seasonal palette remain responsible for foliage color.

**This is a static authoring/bake gradient, not the GPU's four-season
gradient table.** Editing `seasonal_palettes` must not rebake a static
ramp texture or touch tree geometry, occupancy, wind or GPU shader
compilation. The foliage regression test explicitly verifies this
separation. All 1,039 existing material presets and old pixel hashes
remain in the validation suite.

## Deliberately deferred

Curve remap, draggable graphical handles, shared graph groups and
per-instance overrides are separate Phase 4 slices. Ramp stops are
currently authored in the ordinary inspector and evaluated on the CPU,
not a new runtime material representation.
