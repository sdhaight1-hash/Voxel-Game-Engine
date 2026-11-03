# Phase 9a — opt-in dielectric bark PBR in the wgpu preview (#10)

**Purpose:** Connect the already-authored material graph's bark Roughness
and height-derived Normal outputs to the *existing* Rust wgpu tree and
editable Asset viewport, without changing any preset, voxel ownership,
legacy leaf rendering or default bark appearance.

### User workflow

In the Tree or Asset 3D viewport, enable **Preview PBR bark
(roughness + normal)**. The toggle is preview-only and is **off by
default**. It does not change exported textures or saved tree material
JSON. Normal and Roughness images come from the assigned bark node
graph; no bark graph uses a neutral flat tangent normal and 0.8
roughness.

The same bounded GraphRenderCache already used for node previews is
used for 64px bark roughness and height-to-normal bakes. A camera orbit,
seasonal gradient change or meter-repeat edit does **not** alter source
graph pixels and should not cause another authored map evaluation.

### Renderer contract

- Bark geometry includes one face-local normal plus a UV tangent with
  bitangent sign. The basis is constructed from actual triangle edges
  and original unwrapped UVs, with a safe deterministic fallback when
  UVs are degenerate. This is *not* a MikkTSpace-smoothed basis; that
  requires separately validated mesh-vertex sharing and testing.
- The WGSL material bindings add two independently cached/data-typed
  **Rgba8Unorm** images for roughness and tangent-space normals. No
  gamma transform is applied to either. The albedo texture is stored
  in the same legacy Rgba8Unorm format, but the PBR branch explicitly
  decodes RGB sRGB → linear before shading and encodes linear → sRGB
  before writing into the unchanged UNORM preview target.
- The shader uses a **GGX distribution, Smith masking and Schlick
  dielectric Fresnel** with fixed F0=0.04. The lighting is one fixed
  preview directional light plus a small diffuse ambient approximation.
  This is a useful direct-lit dielectric material preview, not an
  HDR environment/IBL or full game-world renderer.
- Physical meter-scaled U/V and noninteger-ring seam crossfades use
  identical UV selection for albedo, roughness and normal data, keeping
  authored surface detail registered through tapering.
- The old vertex tint, foliage/fruit/blossom coverage, seasonal
  palette, ordered alpha and MSAA paths are untouched when PBR is off.
  Only bark activates the new BRDF. The renderer does not modify the
  CPU material graph or separately export PBR images.
- Camera and material context still use the existing small uniform
  buffer. Enabling the toggle creates/updates only two data textures:
  it does not re-upload the tree mesh, bark albedo, leaf texture or
  seasonal palette unless those assets independently changed.

### Scope limits

This slice is deliberately **bark-only dielectric PBR**. GPU metallic
mask, foliage roughness/normal shading, environment reflections,
tangent-space equivalence across external DCC formats, height
displacement, physically calibrated luminance and full Vulkan/world
lighting remain later work. The existing normal map is derived from
the graph's Height channel, which is not a replacement for an
explicit authored tangent normal input node or a validated
MikkTSpace mesh tangent contract.

Unit and GitHub CI gates exercise graph cache reuse, opt-in defaults,
TBN handedness/degenerate fallback, targeted texture uploads,
meter-repeat uniform independence and WGSL naga validation.
