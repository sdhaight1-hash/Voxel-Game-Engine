# Phase 9b — opt-in metallic-roughness bark PBR (#10)

This focused slice extends the Phase 9a **Tree and Asset bark-only GPU PBR preview** to evaluate the material graph's **Metallic (channel 2)** output in addition to Roughness (channel 1) and Height-derived Normal. All three data textures pass through the existing bounded `GraphRenderCache` at the established `MATERIAL_SIZE` and are bound separately in the existing wgpu pipeline.

The single new metallic texture uses `Rgba8Unorm` and the **red component as a linear metallic factor**, with no sRGB conversion. A missing bark graph yields a black metallic map (0), a flat normal map and 0.8 roughness. The existing opt-in preview toggle is still off by default and never serialized; zero metallic is the exact same dielectric shading path as Phase 9a.

## Direct-light metallic workflow

The WGSL microfacet shading keeps the existing GGX normal-distribution, Smith visibility, Schlick Fresnel, UV-derived object-local tangent frame, and explicit base-color sRGB→linear→sRGB handling. The Fresnel base reflectance is now:

    F0 = mix(vec3(0.04), linear_albedo, metallic)

The diffuse component and its ambient approximation are multiplied by `(1 - metallic)`; at metallic = 1 the conductor has no diffuse term. Roughness controls GGX lobe width for dielectrics and conductors. This is still a **fixed direct preview light**, not IBL/environment reflection, so fully metallic surfaces away from the light may be dark. It is not a claim of accurate game-world or studio HDRI lighting.

The three PBR map samplers share the **same two UV projections and 20% tapering-ring seam crossfade as bark albedo**, including opt-in physical meter scaling. Map edits only upload their respective GPU data textures; existing mesh, bark albedo, leaf cards, foliage palettes, authoring graphs and tree occupancy are unchanged.

## Compatibility and tests

- `with_bark_pbr_maps` accepts roughness, normal **and metallic** images, with equal-length validation.
- The scene tracks the metallic texture and upload counter independently.
- Legacy bark/foliage/fruit/blossom shader branches retain their previous result when the PBR preview toggle is off.
- Non-metallic authoring graphs retain dielectric shading (Metallic = 0). A Metallic scalar connected in the graph is honored in the CPU bake cache.
- Rust tests cover 0-metal fallback, all three map bakes and cache reuse, metallic-only GPU upload, roughness-only GPU upload, unchanged geometry and palette uploads, and meter-repeat uniform-only updates.
- The existing WGSL Naga parser/validator and desktop compilation remain mandatory CI gates.

## Still outside this change

Foliage PBR, true microfacet environment/IBL, metallic appearance in exported static OBJ material rendering, tangent-space parity against MikkTSpace, explicit normal texture graph nodes, and world light sources remain separate Phase 9 integration work. No external dependency or material/Flora preset rewrite is required.
