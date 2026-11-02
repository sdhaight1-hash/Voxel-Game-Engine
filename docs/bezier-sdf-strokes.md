# Bézier and tapered signed-distance strokes — Issue #10 / Phase 5b

This extends the typed **Distance** primitives introduced in Phase 5a.
The Nodes editor has an **SDF Bézier Stroke** generator with 4 cubic
control points, independent start/end radii, a curve-sampling slider
(8–64 segments), and a **Repeat across U/V seams** toggle.

## Authoring and usage

The four points (start anchor, two handles, end anchor) are authored in
normalized 0..1 tile UV coordinates. They form a cubic Bézier centerline.
The start/end radii interpolate linearly along the path, allowing a
tapered twig, vein, blade, leaf midrib or stylized bark fracture.

The stroke evaluates to a **signed distance**: negative inside the
stroke, positive outside. Compose it with Circle, Ellipse, Capsule and
the SDF Boolean node, then connect **Distance to Mask** before any
Color/Scalar/PBR or Leaf Cutout operation.

When UV tiling is enabled, the path follows the **shortest periodic
displacement between successive control points**, not the long direct
line across the tile. The CPU evaluator checks neighboring copies of
each curve segment, centered on the segment's unwrapped tile location.
This keeps edges continuous at U/V = 0/1, including sequences crossing
more than one tile internally. With tiling disabled, control points
describe an ordinary bounded path without periodic replication; seams
are *not* guaranteed to match.

## Sampling and limitations

For predictable cost, the CPU engine approximates the curve with 8–64
line segments, then finds the nearest centerline segment and subtracts
the interpolated local radius. It is an **approximate SDF**, not the
exact Euclidean distance to a tapered cubic offset curve. Higher
subdivision counts increase accuracy and CPU sampling cost. It stays
inside the existing lazy coordinate sampler so spatial Warp/Translate
nodes preserve the proper UV evaluation behavior. No raster resampling,
new GPU shader, external dependency, canvas path editor, or automatic
pixel-footprint antialiasing is introduced.

Signed-distance values retain their full f32 range until explicit
Distance-to-Mask coverage conversion. Reroute and legacy RGB nodes do
not implicitly accept Distance. Saved parameters are stored within a
new additive NodeKind variant; older graphs, 1,039 presets, existing
Flora assets, bark/leaf material ownership and runtime seasonal
palettes remain unchanged.

Tests include: taper sign, straight-line capsule approximation,
periodic seam continuity, longer unwrapped control chains, invalid
parameter bounds, Graph JSON round trips, legacy/memoized four-channel
render byte parity, a coordinate-warped Bézier + Circle SDF blend and
silhouette-preserving leaf opacity masks.

The next vector-pattern work still includes path handles in the canvas,
user-defined multi-curve contours, polygon Boolean tools, seeded edge
distortion, high-quality antialiasing and explicit meter-scale transforms.
