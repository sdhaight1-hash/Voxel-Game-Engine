# Flora prefab v1 — shared plant definition

**Implemented in:** `src/flora_prefab.rs` (Rust), `src/foliage.rs` (existing source), and `src/plant_climate.rs` (habitat). **43 existing plant types** use the same `FloraPrefab` data schema; no species-specific Rust subclasses or duplicate geometry generators.

## Composition instead of inheritance

```text
FloraPrefab (reusable authoring definition)
├── format_version = 1
├── asset_id (e.g. "flora/ArabicaCoffee")
├── species = PlantPreset
├── source = FoliageProject
│   ├── seed + per-species growth rules and artist overrides
│   ├── trunk, branches/fronds, foliage geometry controls
│   ├── bark and foliage materials, textures and cutout shapes
│   ├── seasonal palettes + wind
│   └── local 1m axial Q/R/Z hexel generator
└── habitat = ClimateProfile
    ├── annual temperature and rainfall
    ├── brief cold-event floor and dry-month preference
    ├── root-zone moisture and light preference
    └── dependable groundwater requirement

FloraPlacement (future world instance reference)
├── prefab_id → refers to one immutable library definition
├── seed → creates an individual, not a new species
└── q, r, y → world coordinates (Y vertical)
```

**Single sources of truth:** geometry and material generation remain in `FoliageProject::generate()` / `FoliageMaterials`; climate/site scoring remains in `ClimateProfile::suitability()`. A `FloraPrefab` connects them and adds a stable asset ID and schema version. The species field is validated against the source project's preset. The habitat may be customized independently of the built-in default and is validated before saving/generation.

**Definitions vs instances:** `FloraPrefab::from_preset(species)` builds an existing built-in definition. `FloraPrefab::from_project(asset_id, project)` preserves authored Tree Designer edits. `FloraPrefab::generate_with_seed(seed)` produces local geometry for an individual without modifying the prefab. `FloraCatalog::built_in()` enumerates and validates **all 43** definitions. `FloraPlacement` is only a compact serializable world reference; it does not yet perform placement or change world state.

## Saved prefab format

Select a species, adjust growth/palettes as desired, then choose **Foliage → Tree Designer → Output → Save Flora prefab JSON**. This exports one `FloraPrefab`, using the canonical built-in ID `flora/<serialized-variant>` unless explicitly edited in JSON. **Authored variants must receive unique IDs before publishing**. Files are self-contained authoring data, including the FoliageProject, optional material graphs and the selected habitat, and can be validated/read through `FloraPrefab::from_json`. Regular **Save foliage project** continues to write the unchanged legacy project schema; old projects are not migrated or broken.

Example (abbreviated; an exported file contains the full source):

```json
{
  "format_version": 1,
  "asset_id": "flora/ArabicaCoffee",
  "species": "ArabicaCoffee",
  "source": {"preset": "ArabicaCoffee", "...": "full editable FoliageProject"},
  "habitat": {
    "mean_annual_temp_c": [18, 23],
    "annual_precip_mm": [1200, 2200],
    "extreme_min_c": 2,
    "dry_months_per_year": [1, 3],
    "soil_moisture": "Moist",
    "light": "PartialShade",
    "requires_groundwater": false
  }
}
```

`FloraPrefab::to_pretty_json()` exports the full validated format; `FloraPrefab::from_json()` parses and validates it. The example above is illustrative, not parseable source JSON because the nested tree is abbreviated.

## Scope boundaries

**Implemented now:** reusable shared prefab definition, built-in catalog of 43, deterministic per-instance mesh generation, editable project wrapping, habitat scoring delegation, JSON roundtrip, editor export, identity/consistency validation, unit tests.

**Not yet implemented:** world-runtime vegetation spawning/scattering, a compiled/packed asset-library index, actual terrain chunk persistence or harvesting, growing/sapling lifecycle, age or reproduction, inter-species competition, seed dispersal, seasonal leaf occupancy, migration, resource inventory, world Y/Z coordinate import, or instance persistence under player modifications.

The existing world-generation docs say the proposed 1,024 km toroidal world and source→published asset library are not live. Defining `FloraPlacement` does **not** imply that runtime now exists. Once it does, pin prefab IDs and revisions in world saves so edited/harvested individuals do not regenerate underneath players.

## Tests

- All 43 species have unique built-in IDs and valid one-schema prefab definitions.
- Existing source seed, artist morphology overrides and custom seasonal palette survive prefab JSON roundtrip.
- Individuals generated with the same seed are deterministic, changing seeds varies geometry and never edits shared source.
- Reject version mismatch, invalid asset IDs, invalid climate settings or source/species mismatch.
- World placement reference serializes without embedding the reusable mesh.

Run `cargo test --all-targets`.
