# Phase 9c — prefiltered studio image-based lighting (#10)

This opt-in slice builds on Phase 9b metallic-roughness/tangent-normal
bark preview. An authored metallic surface now reflects a bounded
procedural studio environment, rather than relying only on direct light.

## Artist controls / compatibility

The Tree and editable Asset viewports share the new **Studio environment
reflections (IBL)** checkbox beneath Preview PBR bark. Both are **off
by default**. IBL is disabled while the PBR checkbox is off and does
not change Tree, Asset, Flora, material graph or preset serialization.
When IBL is off the previous direct-plus-ambient formula is retained;
with PBR off, legacy bark and every leaf/season/alpha shader path
remain unchanged.

The environment is generated and uploaded once on first request,
stored in existing wgpu callback resources. Turning it on or off
alters only the small material uniform and bind group on first upload.
Camera edits, seasonal gradients, bark material changes and meter
repeat changes do not rebuild the environment or tree geometry.

## Environment data

- A deterministic linear-RGB studio sky/ground radiance source with
  two luminous soft panels. No downloaded HDR assets or dependencies.
- Diffuse irradiance-over-pi is cosine-weighted hemisphere integration
  at 48x24 with 48 deterministic samples per normal direction.
- GGX-prefiltered specular radiance uses 48 deterministic half-vector
  importance samples per direction at roughness 0, .25, .5, .75 and 1.
  V=N=R is used for the prefilter assumption. Five 48x24 maps are
  arranged as 48x130 linear Rgba8Unorm atlas, with guard rows.
- WGSL samples and interpolates two roughness bands, samples the
  cosine irradiance, and applies the Karis/UE4-style analytic BRDF
  scale/bias approximation using the existing graph Metallic and
  Roughness outputs.
- Source environment data is LINEAR; only base color uses sRGB
  decode/encode. Texture U wraps, V stays inside the guarded poles.

## Known boundaries

This is a deterministic **studio preview IBL**, not an externally
supplied HDRI, game-world probe or full physically calibrated HDR
renderer. The tiny environment maps use low-dynamic-range linear
values capped at 1 and limited sample counts. Very smooth, highly
reflective surfaces can show environment prefilter aliasing; the
implementation does not claim true HDR probe quality. Leaf/fruit
rendering, OBJ export, shadows, world lights, AO, multibounce GI,
MikkTSpace external tangents and exact static export parity are
out of scope.

Tests cover deterministic maps, finite alpha/guard rows, roughness
blurring, sky vs ground irradiance, opt-in default off, zero resource
re-uploads for IBL-only toggles and existing Naga WGSL validation.
