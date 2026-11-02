# Optional 4× MSAA alpha-to-coverage foliage preview — issue #10 / Phase 8e

This adds a **real 4-sample GPU depth/color render path** to the existing
wgpu editor viewport, distinct from the previous ordered (single-sample)
4×4 screen-door experiment. It is an optional preview setting. The
default remains **Legacy**, and original materials, the 43 original
species cutouts, PBR/PNG exports, voxel geometry and saved assets
retain their previous pixel/serialization behavior.

## GPU path

The Foliage and Asset preview panels now offer three mutually exclusive
**GPU leaf edges** settings:

- **Legacy** (default): the existing one-sample RGBA8/Depth24Plus
  attachments and hard 0.5 leaf-alpha discard;
- **Ordered 4×4**: the existing single-sample Bayer coverage threshold,
  which can show a visible screen-door pattern;
- **4× MSAA**: a separate pipeline with `wgpu::MultisampleState` configured
  for four samples and `alpha_to_coverage_enabled=true`. Four-sample
  RGBA8 and Depth24Plus render attachments resolve their color into
  the **same original one-sample viewport texture** that egui displays.

Only the multisample branch returns the linearly sampled **fractional
leaf alpha** to the fragment output. Hardware alpha-to-coverage converts
it into the per-sample coverage mask before depth/color writes. Legacy
and ordered paths still output fully opaque surviving leaf fragments and
retain the original discard behavior. Fully opaque bark, non-cutout
leaves, tree wind, GPU seasons and flowering overlays are unaffected.
The original Cherry/Coffea petal threshold still produces solid bloom
silhouettes beyond the leaf card.

The viewport checks the adapter's reported `MULTISAMPLE_X4` format
capability for **both** color (`Rgba8Unorm`) and depth (`Depth24Plus`).
When unsupported (or the GPU preview is unavailable), **4× MSAA** is
disabled in the UI and the single-sample path remains available.
The mode is a temporary editor value, not part of the saved tree,
asset or `FoliageMaterials` JSON.

The existing `globals.season.z` spare uniform channel carries the
mode (0 legacy, 1 ordered, 2 MSAA); the shader/bind-group layout and
seasonal palette data do not change. No geometry, bark, leaf or palette
reupload or material bake is required when switching preview mode.
The separate MSAA pipeline and attachments are initialized for
capable devices; this consumes extra GPU memory even while Legacy is
selected. That is a deliberate first-slice implementation tradeoff
to avoid rebuilding render resources on every toggle.

## Tests and scope

Rust desktop compilation validates the new wgpu pipeline and render-pass
descriptor types. Naga validates both conditional WGSL paths. CPU tests
check mode/uniform mapping, default legacy selection, shader outputs
and unchanged scene upload keys. The existing 2×2 vector silhouette
coverage may provide useful intermediate alpha levels for 4× MSAA, but
the two controls remain independent; even the binary 64px card benefits
slightly from linear-filtered alpha near its edge.

**Do not confuse** 4× MSAA alpha-to-coverage with fully alpha-blended
transparency, 4× texture supersampling, per-sample shading or mipmapping.
Samples still use bilinear leaf texture filtering and the existing
non-mipmapped 64px cards. Quality depends on adapter sample positions,
shape resolution and visible card size; visual/on-device and GPU
performance comparisons remain necessary before any default change.
This initial PR deliberately does not change opaque world rendering
outside the editor or add new leaf geometry/hexels.
