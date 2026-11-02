# Optional 2×2 vector leaf-card coverage — Issue #10 / Phase 8d

This builds on vector Oak/Willow/Cottonwood silhouettes and the **separate**
experimental GPU ordered-coverage mode from Phase 8c.

## Authoring

Under **Foliage** or **Assets** → **Procedural Material Slots**, choose one of
the five supported vector silhouette sources (Oak, Willow, Eastern/Fremont/
Black Cottonwood). Enable **2×2 vector silhouette coverage** inside that
species' silhouette panel. This is off by default. Enabling it changes the
**baked 64×64 card alpha** to an estimate of subpixel area coverage: four
fixed UV subpixel occupancy tests per output texel produce the deterministic
alpha values **0, 64, 128, 191 or 255**. The authored leaf and twig shape
and 64px raster output dimensions are otherwise unchanged.

To inspect the result, also enable **Experimental ordered leaf edge
coverage** in the GPU Foliage/Asset viewport. That independent option
compares filtered alpha against a 4×4 screen-space Bayer threshold. The
material option provides genuinely fractional edge alpha before GPU
texture filtering; the GPU option approximates spatial coverage without
blended transparency or true multi-sample depth buffers.

Disabling the vector silhouette automatically restores binary alpha
sampling. Selecting a legacy species shape with 2×2 sampling is
explicitly rejected during material validation: original 43 species
card algorithms and fixed regression hashes are left untouched.

## Ownership and caching

`FoliageMaterials.leaf_alpha_sampling` is a defaulted serialized enum:
`Binary` or `Coverage2x2`. Old tree/asset JSON defaults to `Binary`.
The material's original `leaf_silhouette` and species determine the
shape; the sampling flag determines how the 64px RGBA alpha is baked.
The palette-tinted leaf pattern graph still controls color detail.
Additional Leaf Cutout graphs can **only remove** already-existing
alpha pixels and cannot draw outside the selected vector source.

`FoliageBakeCache` includes the selected sampling mode in the Leaf
cache key. Changing mode rebakes the affected 64px leaf pixels and
triggers a **leaf-texture-only** GPU upload. Tree geometry/hexel growth,
bark, GPU wind and four-season palette uploads remain unchanged. A
warm cache entry is reused when returning to the previous mode.

## Performance and limits

The mode performs four distance evaluations per 64px texel; Willow
tapered Bézier SDF evaluation costs more than polygonal Cottonwood
shapes. The work is cached at material bake time, not per tree instance
or fragment. This is not continuous vector rendering on the GPU,
automatic mipmap generation, screen-space MSAA, alpha-to-coverage,
multiple layers of transparency, or an antialiased replacement for
the existing binary opacity-graph threshold.

Tests require repeatable fractional coverage for all five vector
variants, constant old-style binary output when sampling is disabled,
species and setting validation, correct JSON roundtrips, alpha-mask
containment, bounded cache invalidation/reuse and GPU leaf-only upload.
Full Rust/desktop and frozen legacy fixtures remain the CI merge gate.
