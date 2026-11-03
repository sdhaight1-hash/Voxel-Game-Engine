# Phase 6a — Shape Scatter (Issue #10)

This is the first bounded, **CPU material-authoring** slice of Phase 6. It reuses
the same serializable Graph, typed ports, CPU evaluator, existing PBR outputs,
content-addressed preview cache and exact-UV memoization. It is not a second
texture system or a new GPU renderer.

## Workflow

1. Create a signed-distance shape (Circle, Bézier Stroke, etc.) and explicitly
   convert it with **Distance to Mask**.
2. Feed its **Mask** into **Shape Scatter → Pattern Mask**.
3. Optionally connect a **Mask** to **Density Mask**. An unconnected density
   input means 100% of candidate cells are enabled, not a black mask.
4. Connect Shape Scatter's output **Mask** to Albedo/Height, or process with
   existing scalar, ramp or color nodes. Mask intensity and additive PBR channel
   selection are independent of alpha/leaf-card ownership.

**Cells** is 2..32 in each normalized UV dimension. There is at most one
candidate stamp in each hash-addressed grid cell (at most 1,024 per tile).
**Scale** is local square width measured in cells (0.1..0.85); **Jitter**
is a random center displacement per axis (0..0.25 cells). **Rotation** 0..1
controls the extent of a random angular rotation. **Brightness** 0..1
controls per-stamp attenuation; **Seed** controls the full arrangement.

Each pixel queries its local 3×3 cell neighborhood, not every stamp.
The conservative maximum stamp radius includes rotation and jitter.
There are no per-pixel allocations, global seed-dependent file imports or
growing scatter image cache. Adjacent UV tile edges repeat exactly (the
hash uses periodically wrapped cell coordinates). Multiple masks combine
by max coverage, so overlaps cannot increase opacity above the authored mask.

The **density mask is sampled at candidate centers**, not separately at
every point within a stamp. This avoids introducing a per-pixel density
cutout, but it also means high-frequency density fields can alias. A
different seed changes placement without modifying the supplied pattern.

## Explicit limits / subsequent Phase 6 work

- This is **mask shape splatter**, not color-image splatter, Flood Fill or
  independently editable scattered objects.
- An arbitrary connected pattern is sampled in each stamp's local 0..1 UV;
  the pattern must be shaped in that space. Thin high-frequency features may
  need export supersampling.
- Fixed-neighborhood sampling bounds the number of *candidate evaluations*
  per pixel; graph evaluation of expensive pattern inputs is still CPU work.
- Height-aware PBR layer blending, region IDs and deterministic reference
  bitmap assets belong to subsequent Phase 6 PRs.
- Legacy graphs, 1,067 existing texture presets, the 43 Flora species, and
  existing serialized material/leaf silhouette defaults are not modified.

Verification includes tile wrapping, deterministic seeds, density zero,
invalid ranges/types, JSON roundtrips and legacy-vs-exact-UV memo parity.
Full cargo/desktop CI is the merge gate.
