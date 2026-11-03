# Phase 8b — meter-scaled static OBJ bark UVs (Issue #10)

This slice connects the already-authored **Bark meters per repeat** option
to two existing static exporters:

- **Export Voxel Tree** writes \`foliage.obj\` with physical bark tube UVs.
- **Export Asset** writes \`asset.obj\` with the same mapping after converting
  a tree into editable standard hexels.

The project's graph-owned RGBA albedo/PBR maps remain the same; the
OBJ \`vt\` coordinates now choose a physical repetition density for
organic branch tube sides. No extra PNG resources or dependencies are
needed. The original mesh geometry, cell ownership, leaf and cutout UVs,
and flat bark/twig/cap UVs remain untouched.

## Metric interpretation

On supported six-sided wood tubes, the existing renderer exposes
\`bark_circumference_m\` per vertex (6 × ring radius) and cumulative
branch-arc V in meters. For a configured \`meters_per_repeat\`:

    repeats = bark_circumference_m / meters_per_repeat
    U = ring_fraction * repeats
    V = branch_arc_m / meters_per_repeat

The static path matches that mapping for ring fraction U from 0 to
0.80. A fractional number of repeats cannot join perfectly around
a closed cylinder with a plain OBJ UV. In the final 20% of the
circumference, the static export warps the phase toward the **nearest
integer repeat** with a smoothstep. It therefore matches at U=0 and U=1
under ordinary repeating texture samplers, but **is not pixel-identical
to the interactive GPU preview**, which blends two *sampled colors*
instead of warping one sample's coordinates.

The closest-integer phase can also change along an aggressively
tapered tube. This is a limitation of static UV-only exports, not an
issue solved by increasing mesh density. A truly identical static
export would require a bounded bake of the shader's seam-aware
appearance or a shader-aware format.

## Compatibility guarantees

- **Option disabled (the default):** \`FoliageMesh::obj()\` is unchanged
  byte-for-byte, and \`obj_with_bark_meters_per_repeat(None)\` produces
  exactly the same text. All pre-existing Flora presets use this path.
- **Option enabled:** only \`vt\` entries for measured organic wood tube
  sides change; \`v\`, \`f\`, materials, leaves and JSON-backed hexels remain
  unchanged.
- Invalid nonfinite or out-of-range settings are rejected at export,
  consistent with \`FoliageMaterials::validate\`.
- All authored material and PBR texture pixels are reused, so the
  existing material bake cache is unaffected.

## Remaining work

The full GPU PBR integration still needs roughness/metallic/normal
sampling and physically correct tangent-space shading. A future
exact static bark appearance bake is separate from the standard
OBJ UV exporter.
