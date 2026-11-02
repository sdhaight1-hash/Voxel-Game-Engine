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
| **Foliage** | `src/foliage.rs`, `src/foliage_materials.rs` | Seeded tree/shrub hexel or cube occupancy, bark/leaf assignments, UV-cutout leaves, wind settings | **Author/generate** vegetation geometry and visual variations, which the world can instantiate or build from a pinned recipe |
| **Assets** | `src/asset.rs`, parts of `src/main.rs` | Editable cube/hex-prism occupancy, material slots, JSON source model, OBJ/MTL compatibility export, optional attached material graph | **Author** placeable objects/structures and prefabs; retain editability independently of their origins |
| **Shared authoring preview** | `src/gpu_renderer.rs`, `src/voxel_gpu.wgsl`, `src/voxel.rs` | Exposed hex/cube faces, per-face UVs, GPU textured mesh preview, cutout leaves and wind | **Preview/test** authored assets; reuse rendering techniques, **not** assume one-asset viewport is a world chunk renderer |

**Preserve existing behaviour:** The asset editor currently supports both cube and hex-prism assets, and its existing asset/project JSON files remain valid. The **playable world** has a preferred hexagonal-prism geometry; that does not require removing cube authoring/export support.

## Implemented 1 m harvestable-hexel tree authoring (2026-11-01)

The Foliage subsystem now exposes a **world-ready conversion** via `FoliageProject::generate_world_hexels()` and the Foliage **Output → Create editable 1 m hexel tree** button. The resulting tree opens in **Assets** as a regular editable `VoxelAsset`. This is an **authoring and per-cell edit/harvest foundation**, **not a working game-side tree-felling system or published runtime asset library**.

- Wood/trunk and substantial branch geometry consist of **real occupied regular hexagonal-prism cells** on axial Q/R and integer Z. A standard cell is **1 metre across flats and 1 metre high** (approximately **0.866 m³** in volume), not a 1 m³ cube. This size is fixed for the **world-ready conversion**, while old project-specific artistic cell sizes and cube authoring remain available.
- Leaf cells are real, individually addressable occupancy entries, but their visible geometry is **two crossed UV-textured alpha-cutout cards** with existing visual wind. No solid leaf-cube requirement is introduced; leaves can be removed as logical cells.
- The world conversion leaves the source `FoliageProject` unchanged; generated wood and leaves are returned as ordinary material-labelled `VoxelCell` entries. `Material::is_wood()` and `Material::is_foliage()` distinguish the two broad harvest categories.
- `VoxelAsset::harvest_cell(position)` removes exactly one cell and returns its `Material` for later game-side item/drop rules. This presently edits an **asset**, not an actual loaded world chunk; felling, tools, drop quantities, block-state replication, ecology and persistence of instances await the world runtime.
- Hex-prism branch rasterization now inserts intermediate **face-adjacent wood cells** to bridge diagonal/vertical jumps, supporting connected harvestable branch geometry. No assumption of mesh-only wood is needed.
- The optional `BranchGrowth::CanopyGuided` mode uses seeded canopy attraction targets to steer branch tips without changing older saved projects' default **Classic** generation. This is **not a full iterative space-colonization implementation**; deeper Shrubbery-like skeletons and growth recipes remain future work.
- Tests cover the 1m cell scale, retained leaf cards, deterministic guided variation, connected wood, legacy project defaults and per-cell material-returning removal.

**Source-of-truth separation is unchanged:** The foliage/asset editor prepares blocks and visual data; the eventual world runtime owns placements, block edits, harvesting rewards, persistence and ecology. A fabricated game harvesting feature must not be inferred from the new source APIs.

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
- **World-ready tree hexels now use 1 metre across opposing hex flats and 1 metre vertically**; this does not finalize the geography-to-world sampling scale, asset transforms or all other world-block categories. Normal asset voxel-size values remain independently editable artistic scale parameters.
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
