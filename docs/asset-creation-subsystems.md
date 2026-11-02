# Asset-creation subsystems and game-runtime integration

**Status:** Architecture direction recorded 2026-11-01. **Subsystem separation and game consumption are chosen design principles;** the runtime asset-pack contract, build/import pipeline and world integration are **proposals, not implemented features**.
**Repository:** [sdhaight1-hash/Voxel-Game-Engine](https://github.com/sdhaight1-hash/Voxel-Game-Engine) only.
**Related:** [Procedural World — Design Decisions](procedural-world-design-decisions.md) (authoritative overall design), [synthetic world generation](world-generation-foundation.md), [README](../README.md) (implemented editor features).

## Core architectural decision

**Nodes, Presets, Foliage and Assets are separate, specialized subsystems of one greater game engine, geared toward creating reusable assets.** They already work together inside the current `texture-studio` desktop authoring application. Keep these authoring responsibilities modular and available as tools as the playable world engine develops; **do not discard or replace them with procedural world-generation code**.

They are **not four separate game engines** and do **not** make the current repository a running planetary simulation. The current product packages the authoring tools as a desktop app; the planned game runtime will consume their published content through explicit interfaces. The authoring UI, asset preparation and gameplay simulation should not be tightly coupled.

```text
Authoring subsystems (existing)                World systems (future)
  Nodes  ── procedural material graphs ─┐
  Presets ─ material/appearance recipes ─┤
  Foliage ─ tree/plant definitions ──────┼──> asset build + library ──> world runtime
  Assets ─ editable cube/hexel objects ──┘      (proposed)               |
                                                                        +-- world-gen placement
                                                                        +-- hexel geometry / GPU rendering
                                                                        +-- authoritative simulation / persistence
```

## Existing working subsystems and their responsibilities

| Subsystem | Current source | What it already produces | Responsibility in the larger engine |
|---|---|---|---|
| **Nodes** | `src/lib.rs`, `src/node_library.rs` | Editable, validated procedural graph; tileable albedo, roughness, metallic, height and derived normals | **Author** surface/material appearances; **bake** textures for runtime reuse |
| **Presets** | `src/presets.rs`, `src/presets/catalog.json` | Data-driven natural material/appearance variants that generate editable node graphs | **Catalog** artistic recipes and candidate appearances; **not** determine geologic placement or physical properties alone |
| **Foliage** | `src/foliage.rs`, `src/foliage_materials.rs` | Seeded 1 m game-hexel wood/leaf occupancy, UV-cutout leaf cards, wind settings | **Author/generate** vegetation geometry and visual variations, which the world can instantiate or build from a pinned recipe |
| **Assets** | `src/asset.rs`, parts of `src/main.rs` | Editable cube/hex-prism occupancy, material slots, JSON source model, OBJ/MTL compatibility export, optional attached material graph | **Author** placeable objects/structures and prefabs; retain editability independently of their origins |
| **Shared authoring preview** | `src/gpu_renderer.rs`, `src/voxel_gpu.wgsl`, `src/voxel.rs` | Exposed hex/cube faces, per-face UVs, GPU textured mesh preview, cutout leaves and wind | **Preview/test** authored assets; reuse rendering techniques, **not** assume one-asset viewport is a world chunk renderer |

**Preserve existing behaviour:** The asset editor currently supports both cube and hex-prism assets, and its existing asset/project JSON files remain valid. The **playable world** has a preferred hexagonal-prism geometry; that does not require removing cube authoring/export support.

## Implemented game-specific 1 m hexel foliage authoring

**Foliage is no longer a generic cube/hex voxel-tree generator.** All trees created by `FoliageProject::generate()` use the intended **1 m across-flats × 1 m high hexagonal-prism world cells**. The authoring UI no longer offers cube foliage, arbitrary voxel sizes, or solid-leaf tree generation. The separate **Assets** subsystem retains its broader generic modeling tools.

- **Wood:** real, individually addressed and removable axial Q/R/Z hexels. Trunks and manually authored wood use full solid hexel surfaces; generated limbs display tapered six-sided wood tubes and the finest twigs crossed bark strips. Branch rasterization still bridges diagonal gaps with face-adjacent harvestable cells.
- **Foliage:** real occupied leaf hexels now display six independently staggered and rotated double-sided UV cutout sprays per cell, with existing visual wind. These sprays stay owned by one removable block coordinate, not separate floating entities. The increased triangle count is an intentional near-asset fidelity trade-off; world-scale LOD/batching is still unimplemented.
- **Generation:** species-specific **1 m hexel-scale** trunk sizes, branch lengths, canopy clearances and foliage density. **Oak, Birch and Maple** default to `CanopyGuided`; **Pine, Willow and Shrub** default to `Classic` to preserve tapered, drooping and low-growing forms. Both branching styles remain editable in the authoring UI; neither changes the world cell geometry. The guided mode is not yet a full iterative space-colonization skeleton.
- **Denser canopy generation:** interior ellipsoid occupancy is biased toward filled cells while edge noise retains the irregular silhouette. Broadleaf presets receive additional overlapping crown lobes and a top cap to hide exposed leaders; Willow's hanging leaf groups are placed along descending fork paths and conifer needles along intermediate limb spans. Density is still a generation probability, not a physical leaf count. All added cells retain ownership IDs and survive the same connectivity filter.
- **Species-specific structural presets:** Oak and Maple produce different broad crown envelopes; Pine forms staggered six-way tiers with shorter upper branches; Birch grows narrower rising limbs; Willow's limbs arch outward before dropping into hanging forks and elongated leaf clusters; Shrub produces low spreading branches. These profiles change real hexel occupancy while preserving the 1 m world grid, branch ownership, and editable generation parameters. Automated tests check pine whorls/taper and birch/willow branch direction.
- **Asset handoff:** the existing **Edit Tree as Asset** command now always produces an ordinary 1 m hex-prism `VoxelAsset`, without any separate game conversion step. Existing Foliage preview and voxel export use the same hexel geometry.
- **Per-cell authoring removal:** `VoxelAsset::harvest_cell(position)` removes one occupied cell and returns its `Material` (wood vs foliage classification). This is an **asset-editor primitive**, not a working axe/felling, item-drop, save-world or tree-instance system.
- **Species-specific leaf-card art (implemented):** Family-appropriate deterministic 64×64 alpha silhouettes include **Oak (lobed), Pine (needles), Birch (pointed/serrated), Willow (long narrow blades), Maple (palmate five-lobed), Acacia (feathery), Bamboo (narrow blades), fruit trees (oval/compound), and Shrub (small rounded clusters)**. They are still crossed, UV-textured card geometry inside harvestable 1 m leaf hexels, not free-floating or solid leaves. `FoliageMaterials::leaf_species` records the chosen shape with the material definition so texture baking, preview, tree-to-asset conversion, JSON save/reload and PNG/OBJ exports use the same species. An optional Node opacity graph may remove pixels but cannot fill transparent parts of the silhouette. The species leaf tint is authoritative; the material graph modulates luminance rather than replacing foliage hue. Tests validate cutout masks, material differences and asset persistence.
- **Fixed dimensions:** 1 m across opposite hexagon faces × 1 m tall = approximately **0.866 m³** of occupied prism volume. A 1 m hexel does not have the physical volume of a 1 m cube.
- **No foliage legacy formats:** earlier alternate grid/voxel-size foliage project settings and old standalone tree-JSON import conversion are no longer supported. Do not retain dual paths or hidden UI compatibility switches for foliage.
- **Automated tests:** verify the 29-name tree catalog and category counts, fixed geometry/export, all available presets' deterministic generation with wood and foliage, structural distinctions, leaf cards, connected wood in tested configurations and editing/removing saved tree hexels. Real-world play, physics and world-block persistence are not implemented yet.

**Ownership stays explicit:** the Foliage creator produces actual world-block blueprints; world placement, tool requirements, felling traversal, item yields, ecology, multiplayer and authoritative persistence belong to the future runtime.

## Complete tree and bamboo species catalog (implemented authoring presets)

**29 entries, matching the requested natural-world catalog**; `Shrub` remains an additional non-tree foliage preset. The Foliage UI categorizes and selects the full range. These are authored 1 m-hexel structural growth recipes with reusable procedural bark and UV cutout-card materials; they are **not** live ecological growth models or automatically forest-scattered objects.

| Group | Presets |
|---|---|
| Hardwood & broadleaf (10) | Acacia, Birch, Ebony, Kapok, Maple, Crimson King Maple, Oak, Purpleheart, Walnut, Willow |
| Conifer & evergreen (5) | Bald Cypress, Mediterranean Cypress, Larch, Pine, Redwood |
| Fruit trees (12) | Red Apple, Pink Apple, Yellow Apple, Pear, Olive, Peach, Cherry, Mango, Orange, Lychee, Breadfruit, Pomegranate |
| Bamboo (2) | Green Bamboo, Brown Bamboo |

`PlantPreset::TREE_CATALOG` is exactly these 29 in catalog order; `PlantPreset::ALL` adds `Shrub` for the authoring UI. Serialized enum variants are stable internal IDs, and `PlantPreset::material_name()` maps display titles onto existing sentence-case material catalog entries. Fruit trees store `fruit_color` in `FoliageMaterials`, baked as modest **visual fruit-colored accents on leaf-card textures only**. In particular, Red/Pink/Yellow Apple share an appropriate leaf shape but retain different fruit colors. No separate harvestable fruit objects, ripening seasons, picking actions, or inventories are implied.

Species are designed to produce distinguishable architectural habits on the same world hexel grid: high umbrella Acacia crowns, tall Kapok, spreading broadleaves, tiered conifers, columnar Mediterranean Cypress, drooping Willow, orchard forms and narrow bamboo stalks. Each recipe provides its own editable height, trunk size, branch count, branch reach, leaf density, pigments and growth settings. Multiple biological species may legitimately share leaf **morphology**; the goal is differentiated geometry and identity, not an artificial one-to-one set of 29 alpha silhouettes.

## Structural trees: trunk, attachments, supported foliage (implemented)

The Foliage generator now records a **single cohesive tree structure** alongside its real 1 m harvestable hexels, inspired by PlanetSmith's editable structure workflow and the Vintage Story forum discussion of believable branches and crowns. This is **our own implementation**; no external model packs are imported.

- **Trunk first:** rasterize a solid hexel trunk with the existing species-specific thickness and taper. The trunk is recorded as structural branch `0` and its bottom-layer wood hexes form the **root footprint**.
- **Attached major and small branches:** each generated limb and fork has an ID, a parent branch, a snapped Q/R/Z attachment cell and a tip. `TreeStructure.spans` stores the actual unsnapped branch endpoints and taper radii in integer millimetres to retain reproducibility and JSON round-tripping. Each wood cell is attributed to one owner, while overlaps retain already-authoritative trunk/limb ownership. Wood remains 1 m solid game hexels, not independent mesh-only branch entities.
- **Branch-owned leaf clusters:** each foliage cell records which limb/fork spawned it. Leaf cards remain species-specific alpha-cutout textures occupying real, removable leaf hexels.
- **No floating canopy islands:** a face-neighbour flood-fill removes any probabilistic leaf island that does not ultimately reach wood. This does **not** guarantee botanical realism, but prevents entirely unsupported leaf clusters in the generated structure.
- **Editable, persistent provenance:** `TreeStructure` (`origin`, `root_footprint`, `branches`, `spans`, per-cell `TreePart`/`branch_id`) is embedded in Foliage game-tree JSON and in `VoxelAsset.tree_structure`. Individual add/erase/paint operations update cell-role metadata and recompute the ground footprint; a fully removed base allows the asset to lose its tree-specific metadata rather than keeping an invalid root record. Untouched tree components keep their original branch identity.
- **Pure planting-site check:** `TreeStructure::fits_site` takes terrain-support and clearance callbacks over local cells. It performs no biome selection, terrain mutation, chunk streaming or wrapping; the eventual world generator must map local Q/R/Z onto world coordinates and check period-boundary and chunk overlap correctly.
- **Testable bounds:** the existing 40,000-cell asset budget, deterministic seeds and fixed 1 m across-flats / 1 m vertical geometry remain. Tests check branch relationships and root/site constraints, supported foliage, manual edit consistency and serialized provenance.

**Visual direction:** The [MountainLabs Voxel Tree Pack](https://mountainlabz.itch.io/voxel-tree-pack) is a **style reference** for distinctive silhouettes (dense broadleaf crowns, tapered conifers, hanging willows, irregular exposed branches and seasonal variants). It is **not** bundled, copied or converted to the game's hexel assets. The generator does not yet match those models exactly; especially, small branches still appear as full-sized wooden hexel prisms, and future refinement could use narrower *render geometry* inside harvestable branch cells.

**Not yet implemented:** Gameplay tree-felling logic, tool/drop rules, segment collapse, published variant catalog, runtime chunk-aware asset stamping, toroidal planting/scattering, climate-based species selection, ecological growth and persistence of world instances. Structural IDs are groundwork for those systems, not proof those systems are already running.

## Proposed asset handoff: editable source vs game-ready content

1. **Source asset (editable and authoritative for creation):** a `Graph`, `FoliageProject`, `VoxelAsset`, or preset/recipe with its source settings, parameters, seed, material bindings and format version.
2. **Validation/build step:** verify geometry, shapes, source dependencies and references; bake reusable texture channels; produce geometry/occupancy and any optional foliage cutout. Fail explicitly on missing/invalid inputs.
3. **Game-ready asset (proposed format):** stable asset ID plus version/content fingerprint, geometry or pinned deterministic generation recipe, compact material IDs, baked textures, collision/placement metadata when needed, and optional biome/category tags. The **exact binary format and packing strategy are open**.
4. **Shared runtime asset library (proposed):** index assets by stable IDs, load and cache them once, and supply them to world generation, rendering and local gameplay. Duplicate instances should reference shared definitions rather than carry copies of authoring graphs.
5. **World instance/state:** placement and identity, current damage/edits, growth, inventory, water usage and other simulation data live in **authoritative world state**, not inside reusable immutable asset definitions.

**Source projects and game-ready data are different formats by responsibility**, not necessarily different repositories or applications. The existing editable JSON and OBJ/MTL exports are useful; **OBJ alone is not an authoritative in-game hexel asset format**, because it lacks editable occupancy and semantic bindings.

**Determinism and upgrades:** Generated vegetation can use a stable asset ID, pinned generator revision, seed and parameters so a world reproduces an asset without storing its full expanded occupancy in every untouched instance. A modified tree, harvested resource or player-edited structure must persist its overrides so regeneration never restores removed material. A source-asset edit should not silently rewrite or reset already-existing placed objects in saved worlds; asset revision/migration and eventual hot-reload policy remain open.

## Ownership boundary: assets describe, world systems decide

| Content or property | Owner of truth |
|---|---|
| Bark, leaves, granite, clay **appearance** | Nodes/Presets source + compiled material definition |
| Tree shape, leaf geometry, artistic variation | Foliage asset definition / pinned procedural seed |
| Placed object/structure blueprint and local hexel occupancy | Assets subsystem source / published prefab |
| Where granite, rivers, soils and biomes occur | Tectonic/geology/climate/hydrology world generation |
| Where a tree species can appear | Biome/ecology placement rules, optionally drawing on asset tags |
| A tree's age, health, harvested state or consumed water | Authoritative ecology/world state |
| Mine excavation, structure damage, placed hexels | Persistent world edit state |
| Lake volume, rainfall budget, groundwater and water transfers | Authoritative **conserved** water simulation |
| Mesh GPU cache, texture resources, distant visual animation | Derived renderer state; never the source of physical truth |

**Do not execute per-hexel material graphs every frame.** Bake textures once per material/variant and share GPU resources, using instancing or appropriate chunk meshes. A material height-map channel is not a physical 1,024 km terrain elevation field. **Do not make foliage asset generation itself the plant-growth or ecological solver**; ecology triggers appearance or geometry updates when required.

## Hexel conversion and scale boundaries

- The existing Foliage/Asset code describes hex occupancy using **axial Q/R plus a vertical Z** within local `[i32; 3]` arrays; the future playable world design uses **Q/R/Y**, with Y vertical. Introduce one explicit import/coordinate mapping, not ad hoc axis swapping scattered through renderers and serializers.
- Existing `FoliageMesh::from_voxels_with_render` validates **asset-scale** coordinates and a 40,000-voxel limit. It is **not** the main renderer/mesher for an entire 1,024 km world or chunk streaming.
- **All Foliage-generated trees** now use 1 metre across opposing hex flats and 1 metre vertically. This does not finalize the geographic raster resolution or physical dimensions of every other asset category; non-foliage Assets models may still have independently editable scale.
- A finished world chunk needs neighbour-aware face culling across chunk boundaries, levels of detail and persistent sparse edits. Reuse geometry math/UV conventions where correct; make a separately optimized world mesher.
- Preserve the current editor's cube/hex material workflows for existing projects and external use even if the game defaults to hexels.

## Relationship to reactive world architecture

The SpaceTimeDB-inspired world-state proposal applies to **runtime world data**, not every operation inside an asset authoring graph.

- Editing an asset changes its **authoring document** and marks only its preview/bake as dirty, as the editor already does through local dirty flags.
- **Publishing** an updated asset creates or revises game-ready library data; consumers use asset IDs/revisions to invalidate cached resource instances where appropriate.
- A gameplay block edit or dam breach instead goes through **authoritative world mutation**, updates persistent occupancy/water transfers, then propagates field/chunk invalidations.
- Hot reloading a material for live play may eventually update render resources without rebuilding unrelated geology or simulations. Changing an asset's **physical** geometry must be handled deliberately to preserve player modifications and saved world consistency.
- A real pub/sub or embedded database dependency is not required just for baking a texture or changing a preview.

## Initial integration slice (proposed, not implemented)

1. **Keep the four authoring subsystems and existing saves operational.** Identify reusable library APIs; do not rewrite the UI, sources or formats merely to create an engine boundary.
2. **Define the minimum `AssetId` + asset type/version contract.** Validate one hexel object, one material graph and one seeded tree; choose a source-to-game build step without designing a universal asset framework.
3. **Bake two terrain materials and one tree** from existing tools; load them from a small fixture-based game/runtime test that does not invoke the editor UI.
4. **Place a hexel prefab in a tiny chunked world** with explicit local Z → world Y coordinate conversion. Ensure matching occupancy, face culling and appearance.
5. **Apply authoritative world edits**, unload/reload and verify chopped trees or altered structures do not reset, while the original source assets remain unchanged.
6. **Connect field-driven placement**, using biome/geology tags to select content but keeping geological/climate calculations independent of texture-node graphs.
7. Add finite-water and reactive spatial update tests once chunk authority/persistence is stable; benchmark materials/mesh caching and asset-instance memory use.

**Non-goals for this phase:** gameplay-time graph baking per block; making a universal ECS or pub/sub bus; merging other repositories; full live asset hot reload; replacing the editor's export formats; implementing a global ecosystem before an import/render/persist test passes.

## Open integration decisions

- Stable asset ID namespace, package structure, source/compiled format, dependency and content-hash policy.
- Compiled mesh instance versus expanded editable-world-hexels representation for individual assets; how to handle mixed cases.
- Material registry identity versus physically simulated block/substance identity.
- Which authored asset properties affect collision, harvesting, growth, fluids and simulation.
- How to pin or migrate asset and generator versions for already-created worlds.
- Whether future asset tools run as editor panels, command-line builds, or shared Rust library calls; **all remain subsystems of the same engine design**.

**Bottom line:** **Asset-creation subsystems create reusable content; world generation places it; rendering displays it; authoritative simulation gives it persistent meaning and behaviour.** None of those responsibilities should be conflated.
