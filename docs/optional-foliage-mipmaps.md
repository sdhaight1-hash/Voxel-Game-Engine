# Optional foliage mipmaps — Issue #10 / Phase 8f

This is a **preview-only**, off-by-default filtering experiment for
distant foliage card shimmer. It works with the existing baked 64px/128px
RGBA foliage and the Legacy, Ordered and real 4× MSAA alpha-to-coverage
preview modes. It does **not** modify serialized material/asset data,
tree growth, hexel occupancy, palette blending, wind or exported PNGs.

## Usage

In the **Foliage** or **Assets** 3D viewport, check
**Preview foliage mipmapping**. This is independent of the **GPU leaf
edges** selector. Uncheck it to return to the original leaf texture
sample at mip level zero. The default is off.

## Implementation

- A leaf texture upload now creates a mip chain (e.g.,
  **64 → 32 → 16 → 8 → 4 → 2 → 1**) at material-upload time.
- **Level zero is the original byte-identical card.** Each successive
  2×2 level averages four parent alpha values and averages RGB with
  alpha weights, preventing fully transparent black pixels from
  darkening leaf edges. For completely empty regions, colors use the
  parent average as fallback.
- The existing `Rgba8Unorm` GPU texture and single filtering sampler
  gain a linear mip filter. The **bark texture stays single-level**.
- When enabled, the shader selects `textureSampleGrad` using UV
  derivatives calculated outside conditional control flow. Hardware
  trilinear filtering chooses the appropriate leaf level for the
  projected card footprint. When disabled, it retains the original
  `textureSampleLevel(...,0)` on foliage and bark.
- The spare `globals.season.w` uniform carries the preview toggle.
  This **does not change the scene-upload cache**, and switching
  preview mode does not rebuild foliage textures, vertex buffers,
  seasonal palettes or the shader.

## Costs and limitations

Generating the mip chain has finite extra CPU work **on leaf texture
upload**, not per frame. It uses about one-third additional leaf texture
memory for ordinary power-of-two cards. All levels are resident on the
GPU even when the toggle is off, because this first slice favors instant
comparison without reuploading a card when switching modes. It does not
alter the author's own bounded `FoliageBakeCache` recipes.

Alpha is area-averaged, **not rescaled to preserve a fixed 0.5 binary
coverage threshold across levels**. As a result thin leaves may lose
coverage at distant mip levels under Legacy alpha discard; Ordered
and 4× MSAA coverage modes may retain more gradual disappearance.
This change does not implement geometry-based leaf LOD, stochastic
alpha remapping, alpha-test coverage preservation, alpha-to-coverage
calibration or GPU profiling. Real-device comparison across moving
Willow, Cottonwood, Oak, flower overlays and multiple zoom levels is
required before changing the default.

Regression tests check mip dimension counts, byte-exact level-zero
identity, alpha-weighted edge RGB, average alpha, constant colors,
preview-only uniform flags, shader branch presence, Naga WGSL
validation and unchanged scene resource-upload decisions. No new Rust
crates or external texture assets are introduced.
