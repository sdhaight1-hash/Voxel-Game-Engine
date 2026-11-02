# Experimental GPU leaf-edge coverage — Issue #10 / Phase 8c

## What this changes

The 3D **Foliage** and **Assets** previews now have an optional
**Experimental ordered leaf edge coverage** checkbox. The toggle is
**off by default**. Both previews share this transient studio setting;
it does not become part of the saved tree, asset, material JSON or exported
texture.

The existing wgpu render target has one color/depth sample and the shader
currently samples 64px RGBA leaf cards using a **linear-filtered** sampler.
With the historical mode the shader discards fragments when sampled alpha
is below 0.5. Binary card texels can produce fractional sampled alpha
near silhouette edges, but the cutoff makes that a hard decision.

When enabled, leaf-card fragments compare their *sampled alpha* to a fixed
**4×4 Bayer screen-space threshold** in the current single-sample WGSL
fragment shader. This distributes fractional edge coverage spatially
without alpha blending, render order sorting, expensive multi-sample
textures or shader recompilation. Zero alpha never passes; full alpha
always passes. Opaque bark and non-cutout leaf triangles are unaffected.

For ornamentally blooming Cherry and Coffea, the old flower threshold
(0.55) still provides fully opaque petals outside the green leaf base.
Seasonal gradients and GPU wind remain identical. Every frame writes the
same small camera uniform as before: a single existing spare
`globals.season.z` value switches the coverage method. No vertex,
leaf texture, bark texture or seasonal palette buffer reupload is required
when toggling this setting.

## Limits

This is **ordered screen-door coverage**, *not* true 4× MSAA,
alpha-to-coverage, analytical GPU edge filtering, alpha blending, mipmapped
foliage, alpha-weighted normal maps or a fully supersampled leaf card*.
A 4×4 pattern may be visible at some viewing distances, particularly
when the tree moves or the editor preview is zoomed. Because the Bayer
pattern is tied to screen pixels rather than time or leaf UV, it does
not add frame-random noise; pixel-level changes can still occur when
a moving leaf crosses the coverage pattern.

The GPU's existing one-sample depth test and opaque output remain intact.
This minimizes hidden sorting/performance risks but cannot achieve the
fidelity of 4× MSAA alpha-to-coverage. The checkbox is explicit so artists
can compare it against the legacy cutout on the same scene.

**This change does not create fractional coverage in the underlying
64px silhouette files.** Vector Willow/Cottonwood/Oak and legacy species
still bake binary alpha cards. Coverage is derived only from the
existing bilinear texture filtering near those edges. Existing PNG,
OBJ/MTL and serialized material outputs are byte-identical.

## Tests and verification

The existing Naga WGSL parse/validation test exercises both shader
branches. CPU regression tests check a complete deterministic 4×4
threshold distribution, endpoint/50% coverage, 4-pixel periodicity,
default shader branch presence and preview uniform mapping. An existing
GPU resource-diff test confirms scene uploads are independent of the
preview-only toggle. The suite retains full legacy texture and foliage
regressions.

A real-device visual comparison across thin Willow blades, Cottonwood
deltoids, wind movement and zoom is still recommended before changing
the default. More sophisticated coverage work should be benchmarked
separately rather than silently upgrading this mode to MSAA.
