# Phase 9d — opt-in dielectric PBR for cutout foliage cards (#10)

This slice extends the existing Rust/wgpu Tree and editable Asset preview,
not the voxel ownership system. It uses the leaf graph's Roughness socket
and Height-derived tangent-space Normal output, and does not rewrite any
existing material or Flora species preset.

## Artist workflow and exact opt-in scope

Preview PBR foliage (roughness + normal) is a separate checkbox from bark
PBR and defaults off. It affects textured crossed cutout foliage cards
(kind 0.5..1.5) only; solid foliage voxel faces keep their legacy
appearance. Studio environment reflections (IBL) become available when
bark OR leaf PBR is enabled. The already-authored studio environment is
loaded once lazily in the shared wgpu viewport callback resources.

These preview options never serialize in Tree, Asset or Flora JSON.
Their toggles never rebuild occupied hexels, tree growth or branches.

## Material, tint and alpha correctness

Existing WGSL first samples foliage albedo in legacy level-0 or selected
mip mode, performs the existing seasonal gradient and spring blossom
computations, and runs the original legacy cutout, ordered coverage, or
4x MSAA alpha-to-coverage decision. Only after that logic does the new
opt-in foliage branch read graph Roughness and tangent Normal maps and
use the shared GGX/Smith/Schlick PBR shader with metallic fixed to zero.

The shader lights final seasonal/blossom RGB with the original per-card
tint, converts sRGB to linear to shade, and encodes to the unchanged
UNORM target. It returns alpha unchanged. All leaf wind sway and
clip geometry stay in the existing vertex stage; backfaces use the
preexisting two-sided normal frame. A missing leaf graph uses neutral
0.8 roughness and a flat tangent normal.

Data maps are linear RGBA8Unorm with no sRGB conversion, generated at
64px using the existing bounded GraphRenderCache. The GPU uses the
existing alpha-opaque leaf mip builder for PBR data mip chains and samples
with the same UV gradients/control as albedo; normal vectors are
renormalized after sampling.

## Uploads and compatibility

The GPU scene compares the leaf roughness/normal images independently,
binding them at WGSL texture slots 10 and 11. Changes to one graph
material output do not need uploads of geometry, bark, leaf albedo,
seasonal palettes or the other leaf data map. Preview flags change only
small uniforms once the data maps have been uploaded. The optional IBL
environment stays cached between scene and season updates.

All existing 43 Flora presets, leaf silhouettes, opacity masks, seasonal
color transitions, blossoms and fruit textures, wind, alpha-cutout, MSAA,
and old bark shader branch remain unchanged when foliage PBR is off.

## Limits

- Cutout leaf cards only, no material change for solid foliage voxel faces.
  Blossoms on cards inherit the same optional dielectric BRDF.
- No true thin-leaf transmission, subsurface scattering, anisotropy or
  calibrated plant optics. The legacy per-face vertex tint includes a
  small baked shading factor; it remains part of optional PBR base color.
- The normal map is still derived from the graph Height output, not a
  separately authored tangent-normal texture.
- Tangents are the existing per-triangle UV basis, not certified
  MikkTSpace-compatible geometry at external export boundaries.
- Studio IBL remains the small LDR environment from Phase 9c, not HDRGI.
- Static OBJ/MTL exports retain their existing texture conventions:
  foliage PBR is a viewport feature, not OBJ renderer equivalence.

Regression gates cover graph cache reuse, roughness-only edits, leaf
silhouette and tint parity, missing-graph neutral fallback, independent
GPU texture upload flags, unchanged IBL-only upload behavior, opaque
data-map mip invariants, preserved alpha/shader contracts and full Naga
shader validation and Rust engine/desktop tests.
