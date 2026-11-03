# Phase 6c: deterministic region variation (Issue #10)

This integrates procedural *region-level* variation into the same typed,
portable, CPU-authored Material Graph used by the 1,067 existing catalog
presets, Flora bark/leaf graphs, Shape Scatter and Height Blend Mask.

## New opt-in nodes

**Region Value**: \`cells\` (2..32 per UV tile axis), \`seed\` (u32), no
inputs, typed **Scalar** output. Every point in the same nearest Voronoi
cell gets the *same hashed scalar value* in [0,1]. This is not a unique
integer region ID: different cells could share a scalar value, especially
after 8-bit PNG export. For masking, connect to the existing typed
**Threshold Mask**, then to Shape Scatter **Density Mask**, Height Blend
Mask **Base Mask**, Mask Mix, color ramps, or other safe Mask consumers.

**Region Border Mask**: same \`cells\` and \`seed\`, plus \`width\` in
0.01..0.5 **cell-distance units**, no inputs, typed **Mask** output.
Its falloff is strongest near Voronoi site boundaries and zero in the
interior. Feed into Height Blend Mask/Mask Mix or PBR output channels to
build tiled crack, seam, blending or bark/stone boundary effects.
Keep the seed and cell count identical to Region Value when both should
describe the same regions.

Both nodes reuse **the existing legacy Voronoi jittered-site placement**,
but change neither the old Voronoi node nor saved project recipes. The
neighbor lookup is a fixed 3×3 window (max nine candidates/pixel), periodic
on both U and V, with no raster allocations and no new dependencies.
The result represents a repeating UV tile, not world-space biomes,
individual editable region objects or a raster segmentation pass.

## Semantics and limitations

Region Border Mask uses the difference between the first and second nearest
sites' distances, smoothed over the width. This is a *practical edge
indicator* (not exact geometric signed distance). Very narrow borders
may alias in single-sample output; existing optional 2×2 antialias export
can help. The underlying Voronoi region selection is spatially deterministic
but is not guaranteed stable across intentionally changed \`cells\` or \`seed\`.

For region-based variations *within* a material, use Region Value with
Threshold Mask and the existing Height Blend Mask/Mask Mix nodes; no
independent PBR output path is introduced. Normal export continues to
derive from final Height. Saved Flora graphs do not change.

Portable imported bitmap/reference resources are **not** included here.
The current graph has no image-asset registry or content-hash invalidation
for external bitmap paths. A bitmap workflow must first define import,
embedded/relative-path ownership, bounded decoding, cache keys and project
save/load behavior; adding absolute file paths or decoding PNG in the
per-pixel evaluator would create broken portability/performance.
