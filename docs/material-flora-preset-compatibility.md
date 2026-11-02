# Material ↔ Flora preset compatibility audit — Issue #10 / Phase 9a

## Findings and scope

The current material catalog had **1,039** original, seeded and editable
natural-material recipes. The tree generator has **43** built-in Flora
species (42 tree/bamboo/coffee plants plus Shrub). Material graphs,
seasonal palettes, foliage silhouettes, vector modes and GPU preview
filters have deliberately different owners.

Before this audit, 14 tree types used unrelated material-source names
as fallback defaults: Cherry Blossom → Cherry, Kentucky Coffeetree →
Walnut, the three Cottonwoods → Birch, five Coffee plants → Orange,
the three Palms → Mango and Shrub → Oak. These generated valid graphs,
but did **not** give each plant its own selectable authored bark/leaf
recipe.

## Changes

- Append **28 new material presets**: 14 species × exactly two roles
  (bark/stalk and leaf/frond). Categories remain the existing **14**.
  New profiles distinguish woody bark from foliar detail where appropriate.
  No old catalog record is deleted, renamed, reordered or reseeded.
  **Total: 1,067.**
- Remove the unrelated-species material-name aliases. Every entry in
  `PlantPreset::ALL` now maps to its own material catalog title.
  The bark/leaf variant chooser additionally understands palm
  **Stalk sides / Frond canopy** recipes.
- `FoliageMaterials::for_species` continues to embed editable `Graph`
  snapshots using the existing typed nodes and outputs. Seasonal GPU
  palettes continue to derive from the original species and leaf tint;
  bark/leaf graphs do not override them. No automatic conversion of all
  old graphs to signed-distance shapes is performed.
- Existing tree, Flora prefab and editable voxel asset JSON contain
  **material graph snapshots**, so previously saved recipes—including
  their older fallback-name provenance—remain as authored. New
  defaults are used only for new trees/prefabs or explicit preset
  assignments. Graph, function template and foliage file versions
  remain unchanged.

## Full-coverage regression gates

1. All **1,067** material recipes load, retain the first 1,039 ordered
   identities and stable seeds, validate their typed graphs, support all
   four PBR output channels, match exact-UV memoized rendering,
   and survive graph JSON roundtrips.
2. All **43** Flora built-ins have matching **species-owned bark and leaf**
   catalog entries and validated editable graphs, and survive tree
   project and versioned prefab JSON roundtrips. None silently falls
   back to another species.
3. Opt-in vector **Oak, Willow, Eastern/Fremont/Black Cottonwood**
   silhouettes and **2×2 coverage** survive prefabs and generated
   editable voxel assets without changing existing cutout ownership.
   The historical legacy binary shape remains the default.
4. Previously saved tree graph snapshots with old fallback titles
   remain authoritative. New catalog mappings do not silently rewrite
   existing authored material graphs.
5. Existing frozen legacy material pixel hashes, 43 species growth/
   branch/hexel tests, foliage bake-cache contracts, GPU scene-upload
   tests and Naga WGSL validation continue to run in CI.

## Important limits

This does **not** mean all 43 species have individually authored vector
geometry or physically scanned bark textures. Five species support the
current optional vector leaf-art sources; the other 38 still use their
existing species-specific 64px binary card cutouts. The new 28 material
presets are **original procedural approximations**, not botanical
photogrammetry. Nor do they make ordered coverage, 4× MSAA or mipmapping
the default; those remain preview options with independent device
quality/performance gates. These distinctions avoid destabilizing
previously saved assets and texture hashes.

Catalog changes are additive under the existing version-1 JSON schema.
New default bark/leaf material appearances can differ from the
previous fallback species for the 14 corrected types; this is an
intentional **new-project** authoring change, not a migration of user
data.
