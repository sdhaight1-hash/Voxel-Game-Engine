# Optional vector Oak leaf silhouette — Issue #10 / Phase 8a

This is the first **opt-in vector-defined foliage silhouette**. It
replaces, rather than only subtracts holes from, the base 64×64 Oak
leaf-card shape. Every existing species and saved project's original
cutout remains unchanged by default.

## Authoring

In **Foliage** or **Assets**, under Procedural Material Slots, expand
**Oak leaf-card silhouette**. Enable **Use vector Oak leaf silhouette**,
then adjust **Lobe width** between 0.65 and 1.45.

The 64×64 cutout is generated from four groups of signed-distance
ellipse lobes, smooth unions and connected twig segments in normalized
0..1 card UV coordinates. Changing lobe width controls the four leaf
profiles, not the branch or hexel geometry. The output remains binary
alpha, matching the existing GPU's cutout/shading contract; shape
sampling and CPU SDF are deterministic.

The leaf's authored graph still controls luminance/detail, and the
species tint and dynamic four-season GPU palettes continue to control
color. Any assigned **Leaf Cutout** mask graph is applied *after* this
source and can only remove existing pixels. The new vector silhouette
can introduce leaf coverage outside the original Oak sprite precisely
because it owns the **base silhouette**, rather than acting as another
subtractive opacity mask.

## Compatibility boundary

`FoliageMaterials.leaf_silhouette` is an optional/defaulted
`LeafSilhouetteSource`. `LegacySpecies` is the default for every old
project. `VectorOak { lobe_width }` is valid for **Oak only** in this
first slice, rejecting invalid widths and non-Oak species.

No change is made to `leaf_cutout_for_species()`, the existing 43
species shapes, TreeGrowthRules, hexel occupancy, leaf-card mesh
geometry, wind shader, seasonal-palette tables, or texture cache
capacity. The existing `FoliageBakeCache` recipe includes the
new silhouette selection and width so a vector change cannot reuse
stale leaf pixels. Old bark bake/cache keys are unaffected.

The original Oak base is still available immediately by unchecking
the option. Both the generated tree and its editable voxel Asset
inherit the selected source through their existing self-contained
material state. The UI marks only material textures dirty when it
changes; no tree growth or geometry regeneration is requested.

## Tests and next work

Tests cover vector vs legacy silhouette difference, actual pixels
added beyond the legacy shape, binary alpha, bounded width changes,
JSON roundtrip, rejected non-Oak usage, opacity masks unable to add
coverage beyond the new base and cache key changes after a width edit.

This is an initial Oak shape, not an automatically inferred leaf
outline from a scanned mesh. It does not add GPU-native Bézier path
compilation, geometric leaves instead of crossed cards, real harvestable
leaf entities, continuous alpha antialiasing, or support for all 43
species. Willow and Cottonwood vector silhouettes will require
species-specific authoring and separate validation.
