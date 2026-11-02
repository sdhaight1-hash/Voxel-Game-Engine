# Optional 2×2 antialiased PNG export — Issue #10 / Phase 5e

This is a narrowly scoped **authoring/export** option for signed-distance
masks and other CPU procedural textures. It does not change the legacy
normal renderer, the per-channel exact-UV memoizer or materials on trees.

In **Nodes**, select **Albedo, Roughness, Metallic or Height** as the
preview channel, choose an export size between 256 and 1024, and click
**Export AA (2×2)** in the toolbar. Save the requested PNG. The original
**Export image** and **Export PBR maps** buttons continue using their
original rendering paths, including the existing opt-in multi-channel
memo flag.

### Sampling contract

For each output pixel, the new `Graph::render_output_channel_rgba_ssaa2x`
method samples its graph at four UV offsets:
`(0.25,0.25)`, `(0.75,0.25)`, `(0.25,0.75)` and `(0.75,0.75)`
within the pixel. It averages the **floating-point RGB values**, then
clamps/rounds **once** when writing the destination RGBA8 pixel. This
resolves partial coverage along an authored SDF outline even if
`Distance to Mask` is configured for an entirely hard 0-width cutoff.

RGBA remains opaque; an exported SDF mask's *RGB channels* carry coverage.
This feature does not make the current species leaf-card silhouette
automatically supersampled or replace its binary alpha/cutout threshold.
It also does not implement a GPU MSAA pipeline, resolution-aware
analytical derivatives, mipmaps or a physically correct filtered
normal-map derivation. Those are separate future work.

### Performance and compatibility

Four independent evaluations per output pixel can be approximately
four times the CPU sampling work of a regular export, especially with
multiple Bézier segments or contour warps. The method rejects zero
sizes, dimensions over 1024 and invalid output channels. The toolbar
disables the AA button for normal-map preview or exports above 1024.

No old graphs, 1,039 material presets, JSON, tree hexels, seasonal
color palettes or PBR export bytes are mutated. Tests check exact
constant-color parity, default-channel values, bounds rejection,
partial SDF coverage and byte-identical regular render before and
after an opt-in supersampled export.
