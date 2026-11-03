# Phase 9e — opt-in thin-leaf transmission and backlighting (#10)

The Tree and editable Asset wgpu previews now offer a thin-leaf
backlighting approximation, building on the existing Phase 9d foliage
roughness/normal pipeline, rather than creating another renderer.

## Artist controls and compatibility

Enable **Preview PBR foliage (roughness + normal)** and then
**Thin-leaf backlighting (Beer-Lambert)**. Set **Leaf thickness (mm)**
from 0.10 to 1.00 mm (initial editor value 0.25 mm).

Both PBR and backlighting are **off by default**, are preview-only and
are never added to Tree, Voxel Asset, Flora or material-graph JSON.
Their settings do not change leaf geometry, occupancy, wind,
normal/roughness images, albedo, opacity, fruit cards or bark maps.

Thickness changes affect one vec4 in the existing small GPU uniform.
The scene-upload comparison does not mark vertex, albedo, normal,
roughness, metallic or seasonal palette resources as dirty. Existing
bounded graph render and foliage texture bake caches are preserved.

## Physical model, units and rendering stages

This is a physically motivated *single-sheet approximation*, **not**
measured per-species plant optics, volumetric subsurface scattering or
a calibrated bidirectional scattering distribution.

On cutout leaf-card fragments only, the shader:

1. Preserves existing leaf albedo, seasonal gradient, spring blossom
   and fruit color, wind-transformed mesh and every legacy/ordered/MSAA
   alpha and coverage decision.
2. Evaluates the existing double-sided dielectric GGX reflection as
   Phase 9d did. When transmission is disabled its output is unchanged.
3. Reconstructs the same view-oriented tangent-space normal to measure
   incoming light from the **opposite side** of the sheet.
4. Converts final seasonal leaf color from sRGB to linear and uses it
   as a wavelength-dependent pigment extinction proxy, in inverse mm:

       sigma_a_rgb = 0.35 + 4.0 * (1.0 - linear_leaf_rgb)

   With sheet thickness d_mm and reverse light cosine cos_back:

       path_mm = d_mm / max(cos_back, 0.2)
       transmittance_rgb = exp(-sigma_a_rgb * path_mm)

   This grazing-path floor avoids unstable values and heavy aliasing.
   The interface factor (1 - 0.04)^2 approximates two dielectric
   Fresnel losses. Reflected + transmitted terms add in linear light,
   then encode back to the preview's existing RGB output.
5. If **Studio environment reflections (IBL)** is enabled, it also
   samples the **existing** low-dynamic-range irradiance in the reverse
   normal direction. No additional environment resource is generated.

Without backlight and with studio IBL disabled, the reflected color
returns unchanged. The final alpha remains identical in all cases.
When transmission is off, the entire leaf path retains Phase 9d output.

The pigment approximation allows summer green and autumn red/yellow
leaves to have distinct transmitted spectra. Reflectance is not a
measured proxy for a species' actual microscopic absorption curve.

## Deliberate limits

- Textured cutout foliage cards only. Solid foliage voxel faces,
  bark and all legacy shader paths are unchanged.
- Petals painted onto cards share this optional dielectric thin-sheet
  approximation; no dedicated blossom petal transmission model.
- No measured leaf tissue thickness, leaf-layer internal reflection,
  physically complete thin-film BTDF, true volume/multiple scattering,
  fluorescence, per-species pigment spectra, world shadows or sky.
- This continues to use the bounded low-dynamic-range studio IBL and
  non-MikkTSpace-certified triangle tangent normals from prior phases.
  It does not establish pixel parity for static OBJ/MTL exports.
- Rust and Naga CI check source and resource contracts, but headless
  checks do not provide real-GPU screenshot-based visual parity.

Tests cover monotonic thickness and grazing attenuation, color-channel
selective transmission, invalid/legacy defaults, dependency on opt-in
leaf PBR, no GPU resource uploads for thickness-only changes and
unchanged alpha, wind and seasonal shading contracts.
