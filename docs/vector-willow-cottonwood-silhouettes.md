# Vector Willow and Cottonwood leaf-card silhouettes — Issue #10 / Phase 8b

This extends the optional base-silhouette system introduced for Oak in Phase 8a.
All **43 legacy species card silhouettes remain the default**. These sources
replace only the generated 64×64 leaf-card artwork, not branch geometry, hexel
occupancy, seasonal GPU palettes, wind or the material graph evaluator.

## Included species

- **Willow:** five pairs of long hanging, tapered cubic Bézier blades with
  connecting stems. `VectorWillow { blade_width }` scales the blade radius.
- **Eastern Cottonwood:** four broad deltoid (triangular/heart-like) polygons
  attached to a shared stem.
- **Fremont Cottonwood:** the deltoid family with an independently scaled
  broadness appropriate to this species variant.
- **Black Cottonwood:** ovate (elliptical/pointed) polygon outlines rather than
  copying the Eastern/Fremont triangular profile. All three Cottonwoods use
  `VectorCottonwood { blade_width }`, but their species identity determines
  the base polygon and dimensions.

Each silhouette uses normalized card UV coordinates and deterministic signed
distance/edge tests to generate binary-alpha artwork. The original species
shade variation is applied with the unchanged 64px card raster contract.
The blade-width range is 0.65–1.45; widening preserves prior coverage.
No external texture files, meshes or Rust dependencies are introduced.

## Authoring

In **Foliage** or **Assets** > **Procedural Material Slots**, expand
**Willow leaf-card silhouette** or **Cottonwood leaf-card silhouette** for
a matching species. Enable the vector checkbox and adjust **Blade width**.
Unchecking restores the exact legacy species image.

The new serialized `FoliageMaterials.leaf_silhouette` variants are additive;
old JSON without that field defaults to `LegacySpecies`. Invalid combinations
such as Willow on Oak or Cottonwood on Willow are rejected by validation
before baking. Oak retains its existing `VectorOak` and Lobe width controls.

A custom **Leaf Cutout** material graph still works *after* the chosen base
silhouette and can remove pixels but never add them outside the selected
source. Leaf Pattern controls luminance; tint, seasonal palettes and dynamic
wind remain independently owned by the existing systems.

## Performance and tests

The new vector generator runs when a material is baked, *not per tree, per
frame or per foliage hexel*. Its output uses existing `FoliageBakeCache`
keys, which already include source, blade width and species. Changing width
rebakes only affected leaf textures; identical recipes reuse cached bytes.
Existing CPU/GPU tree-render paths still consume a 64px RGBA card.

Regression coverage checks valid and invalid species combinations, finite
width bounds, deterministic vector/legacy differences, Eastern/Fremont/Black
shape differences, monotone width coverage, serialized material round trips,
the opacity masking boundary and cache hits/misses. The untouched 43 species
and 1,039 built-in material fixtures remain part of the normal Rust CI gate.

**Not implemented in this slice:** GPU-native vector rendering, high-res/
supersampled alpha cards, world-meter leaf dimensions, freeform multi-curve
outline authoring or leaf geometry tessellation. Those require separate
performance and integration validation.
