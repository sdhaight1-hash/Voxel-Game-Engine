# Signed-distance material patterns — Issue #10 / Phase 5a

These CPU nodes add **editable vector-like patterns** to the existing
procedural material evaluator. They do not introduce a parallel vector
renderer, shader compiler or raster cache.

## Nodes

- **SDF Circle** — center (0..1 UV) and radius (0.001..0.5 UV).
- **SDF Ellipse** — center and independent X/Y radii; signed zero contour
  is exact while distances away from the contour use a fast first-order
  approximation rather than an iterative closest-point solver.
- **SDF Capsule** — rounded line segment from two normalized UV points;
  uses the shortest wrapped endpoint path, including seam-crossing strokes.
- **SDF Boolean** — Union, Intersection or Subtract with 0..0.25 UV
  smoothing. Zero smoothing is exact min/max distance composition; a
  positive smoothing value blends near overlapping boundaries.
- **Distance to Mask** — explicit signed Distance -> coverage Mask
  conversion. Zero edge width means hard inside/outside coverage;
  positive width applies a smoothstep transition with 50% coverage at
  signed distance zero. Mask outputs can feed the existing Color (albedo,
  height, etc.) or Scalar pipeline under the established safe conversions.

## Typed sockets and compositing

Shapes and Boolean operators output **Distance** with *negative values
inside* and *positive outside*, in tile-normalized UV units. A Distance
wire may feed only a Distance input; it cannot feed Color, Scalar,
Mask, output PBR channels, or pre-existing RGB Filter nodes without the
explicit **Distance to Mask** node. This preserves values before
threshold/coverage conversion and prevents silent 0..1 clipping.

Example:

```
SDF Circle  --------- A [SDF Boolean: Subtract] ---> Distance to Mask ---> Albedo
SDF Ellipse --------- B /                                              \--> Height
```

The same mask graph can be assigned as a **Leaf Cutout** slot. Its
coverage can remove additional pixels but cannot override the original
species silhouette. It also works as authored Bark/Leaf graph shading,
independently of seasonal GPU palette blending and wind.

## Coordinates and tile behavior

All positions, radii, edge widths and smoothing widths are normalized to
the 0..1 tile domain — **not physical meters, leaf geometry units or
world positions**. Each primitive repeats at U/V boundaries. Circle and
ellipse use nearest periodic deltas. Capsule checks neighboring copies
of the shortest wrapped segment so edges remain continuous at both
seams. Operations combine distance fields at the input UV unchanged,
including through existing lazy graph sampling. Editing a parameter
affects only its graph/texture dependencies; it does not change hexel
occupancy, tree growth or seasonal color uniforms.

This first slice has no tessellated path geometry and does not promise
screen-space pixel-perfect antialiasing. Positive edge width controls
author-defined *normalized-UV softness*. A future phase can add
pixel-footprint-aware antialiasing, Bézier paths, taper, signed-distance
height transfer, and SDF-aware cache classes.

## Compatibility and tests

The feature is additive: old Graph and Asset JSON, the 1,039 legacy
presets and frozen pixel hashes remain unchanged. Scalar/Mask/RGB
nodes and the exact-UV multi-channel memo continue to use the
original f32 sample protocol. Tests cover signs, exact boundaries,
periodic seams, Boolean operations, coverage, type rejection, invalid
parameters, 64/256/512 render parity after JSON round trips, legacy
single-channel vs memoized RGBA parity, normal-map derivation, and
silhouette-safe Oak leaf-card cutout integration.
