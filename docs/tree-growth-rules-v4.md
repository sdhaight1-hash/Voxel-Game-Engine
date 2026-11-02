# Tree growth rules V4 — Foliage generator integration

**Scope:** All 29 tree/bamboo catalog entries, plus the existing independent Shrub preset.

## Implemented
- The existing `FoliageProject::generate()` is still the entry point and produces the same 1m axial Q/R/Z hexel occupancy, tapered wood branch spans, species-specific leaf cutouts, source project JSON, and OBJ/MTL exports.
- `src/tree_growth_rules.rs` defines 30 (29 catalog + Shrub) per-preset bounded growth profiles: spiral/opposite/whorled/node branching arrangements; limb pitch and horizontal reach; extra lateral twig probability; trunk seed lean; foliage sites and cluster scaling.
- Seeded foliage sites distinguish broadleaf crown tips, compound-leaf twig distributions, Willow curtains, Pine/Redwood needle sprays, Larch spur shoots, Bald Cypress feather sprays and Bamboo upper node branching. Lower bamboo nodes remain comparatively clear of leaves.
- Species trunk taper and base flare are combined with seed-specific height and radius variation. Kapok and Redwood have broader base profiles; Bamboo is nearly cylindrical. These are still solid hexel approximations.
- Existing branch IDs remain stable for a given seed and project, with child branches assigned deterministic IDs.
- Red/Pink/Yellow Apple share identical shape parameters and different leaf/fruit colors. Green/Brown Bamboo share identical shape parameters and different culm/foliage colors.
- The Foliage inspector reports the species' branch and foliage architecture.
- Regression tests cover deterministic same-seed geometry, changing seed geometry, shape-variant identity, structural validation and bound ranges.

## Tree Designer preset editing

The `Foliage → Preset Loader` inspector exposes a collapsible **Species growth rules** editor. Selecting any of the 29 built-in tree/bamboo presets loads its own growth profile, including trunk taper and base flare, primary and secondary pitch ranges, lateral twig chance, and leaf-cluster size. The branching pattern and leaf arrangement are fixed to the species.

- **Seed / Next seed** create deterministic individuals of the selected type.
- Numeric growth controls can be tuned only within the current species' authored bounds. These controls are applied by `FoliageProject::generate()`, not just displayed.
- **Reset to species growth defaults** removes only rule overrides; it does not reset the current seed, palette, materials, or unrelated authoring sliders.
- **Save foliage project / Open foliage project** persist edited rule overrides alongside existing geometry and material settings. Older project JSON files with no overrides continue to load using species defaults.
- Selecting another tree type loads that species' canonical preset and colors as before.

The separate **Presets** workspace remains the procedural material library; tree growth profiles belong to **Foliage / Tree Designer**, not the material graph presets.

## Compatibility boundaries
- No change to serialized `FoliageProject` fields or authored material graph format.
- Existing saved editable `VoxelAsset` objects are independent and do not regenerate automatically.
- **Regenerated** older `FoliageProject` files will produce *new* geometry with V4 algorithms. This should be generator-versioned before long-lived gameplay worlds use the output.
- Seasonal GPU palette interpolation already works in the editor, but its preview slider does not remove fallen leaves or regrow geometry.
- Leaf geometry remains existing two-sided textured cutout cards inside harvestable 1m cells; not individual fully modeled 3D leaves.
- Bamboo still uses an upright hexel trunk and rule-bound branch nodes, not a biologically detailed multi-culm rhizome simulation.
- Tree morphology values are artist-designed visual bounds; Acacia, Ebony and several other labels represent broad botanical groups without a selected single species.

## Validation
`cargo test --all-targets` via GitHub Actions pull-request CI. Tests must also confirm existing 29-preset catalog, foliage exports and material/season JSON compatibility. 

## Follow-on work (not claimed as implemented)
- True segmented bamboo and multiple rhizome-connected culms; buttress roots for mature Kapok.
- Volumetric geometry for every leaf/needle/twig rather than card cutouts.
- Runtime persistence of tree growth generator version and stable per-leaf ID.
- Climate-driven seasonal foliage retention, blossom/fruit life cycles and visible falling leaves.
- GPU-based species/material batching and chunk-scale vegetation instancing.
