# Procedural World — Design Decisions (v0.3)

**Status:** Working design; documents agreed direction, provisional defaults, and open engineering decisions. **Not an implementation claim.**  
**Recorded:** 2026-11-01  
**Documentation synchronized:** 2026-11-01  
**Repository:** [sdhaight1-hash/Voxel-Game-Engine](https://github.com/sdhaight1-hash/Voxel-Game-Engine) — **this repository only**. The working executable is the Rust procedural material/voxel asset editor; the planetary game-world specification below is a future architecture, **not implemented**.

> This document records the current direction established in design discussion. It **supersedes the proposed unbounded, gameplay-time Terrain Diffusion workflow as the default synthetic-world direction** in [world-generation-foundation.md](world-generation-foundation.md). The older [Tellus/Earth-world research](real-earth-world-generation.md) remains reference material, but Tellus is **not** the chosen foundation for this synthetic toroidal world. Do not treat experimental techniques, sizes, compression ratios, or algorithms below as implemented or benchmarked.

## Documentation map and boundaries

This is the **authoritative current design record**. Companion documents in this same repository:

- [Synthetic world-generation foundation](world-generation-foundation.md) — current proposed tectonics → Terrain Diffusion → compressed geography → streamed hexel pipeline, including older InfiniteDiffusion research.
- [Tellus / real-Earth research](real-earth-world-generation.md) — **archived/superseded** dual-world-mode candidate, kept for geodata lessons; **not an active second terrain provider**.
- [Asset-creation subsystems and game-runtime integration](asset-creation-subsystems.md) — **chosen architecture:** Nodes, Presets, Foliage and Assets are modular subsystems for producing reusable game content; source-vs-game-ready formats and the integration steps are proposed.
- [Repository README](../README.md) — implemented asset-creation tools and future-world design overview.

**Project boundaries:** This repository already implements distinct **asset-creation subsystems** for procedural materials, presets, foliage and editable voxel models, currently presented in one `texture-studio` desktop authoring application. Those tools are **part of the intended greater engine architecture**, not throwaway prototypes or independent engines. They support **cube and hexagonal-prism assets**; that does not mean this repository already has a planetary hexel runtime. Another hexel prototype may be cited as a reference, but **no other repository is being changed or implicitly merged**. The 1,024 km torus, synthetic tectonics/diffusion, conserved hydrology and reactive pub/sub architecture are **design work** until tested and implemented.

## Decision status conventions

- **Decided:** Direction or preset explicitly chosen for the design.
- **Provisional:** Current recommended implementation value, pending testing or confirmation.
- **Open:** Significant choice not yet made.
- **Reference:** An inspiration, not a promise of identical implementation.

## 1. World topology and dimensions

| Property | Current design | Status |
|---|---|---|
| World topology | Finite, flat-coordinate **2D torus** | Decided |
| Wrap behaviour | Both horizontal axes wrap continuously; east ↔ west and north ↔ south | Decided |
| Default geographical footprint | **1,024 × 1,024 km** (~1,048,576 km²) | Decided |
| Rendering/physics coordinates | Locally Cartesian; no actual spherical voxel grid required | Decided |
| Planet-like visualisation | Optional globe/world-map presentation | Provisional |
| Physical metres represented by one gameplay voxel | **Undecided**; must not be inferred from km-scale map dimensions | Open |

The player and all world-space systems must cross the wrapping boundaries without visible terrain discontinuities. Periodicity applies to terrain, climate, hydrology, biome maps, chunk selection and relevant world interactions—not just the camera or player position. Use wrapped indexing for the finite domain; distinguish *canonical world coordinates* from local render coordinates.

A torus is **not a true sphere**: two physically distinct poles and exact spherical circulation are not available with simple north–south wrapping. The design aims for plausible planet-inspired climate rather than exact spherical atmospheric dynamics.

## 2. Vertical world presets and sea level

The user-selected vertical world height is fixed for each world at creation.

| World height | Role | Vanilla Vintage Story-style sea level* | Block indices above sea |
|---:|---|---:|---:|
| 256 | Compact option | Y=110 | 145 |
| **512** | **Default** | **Y=220** | **291** |
| 768 | Tall option | Y=331 | 436 |
| 1,024 | Extreme option | Y=441 | 582 |

`Y` is vertical; `X/Z` are informal Cartesian geographic directions. **Playable hexels use axial Q/R horizontally and Y vertically** if that geometry is adopted. `H` is the number of valid vertical block positions, `0 ... H-1`.

`Y_sea = floor(H × 0.431372549)`

*The proportional sea-level rule is the **current provisional default**, inspired by Vintage Story. The four selectable world-height presets and 512-block default are decided; final sea-level modes are not.*

A **lower sea-level mode** (for example 25% of height, yielding Y=128 at H=512) was explored as a way to reserve more space for mountains. It is **not decided** whether that should be exposed at world creation. A lower sea level trades away underground/ocean depth.

Sea level is **not** just cosmetic: terrain mapping, filled ocean volume, aquifers, rock strata, ore generation, caves, biome altitude, plants and snowfall must all use one declared reference. Persist the selected world height, sea-level rule/value and vertical-mapping revision in world metadata. Changing them after world creation requires a deliberate migration, not a runtime toggle.

## 3. Global generation pipeline

**Decided architectural direction:**

1. Run an abstract, synthetic **tectonic simulation** to create continent-scale structure (plates, uplift, basins, mountains and ocean basins).
2. Compute globally coherent coarse **climate and hydrological constraints** (temperatures, prevailing winds, rainfall, watersheds/ocean boundaries).
3. Use **Terrain Diffusion** to refine the surface elevation at regional scale, conditioned on or constrained by global structure where the selected model supports it.
4. Reconcile final coastlines, drainage and terrain continuity, especially across both wrap seams. Exact hydrology passes may need to be recomputed *after* the diffusion-refined elevations.
5. Quantize, validate and save the complete world geography in independently addressable compressed tiles.

The precise conditioning interface between tectonics/climate and Terrain Diffusion needs research and an integration test. Terrain Diffusion produces **2D elevation and associated surface fields**, not fully editable 3D voxel cave networks.

**Tellus is not our chosen underlay.** The earlier idea of real-Earth data with a diffusion makeover was superseded by a synthetic tectonic foundation. Existing Tellus notes remain useful research for geometry, hydrology and dataset-adapter lessons, not as a dependency of this design.

## 4. Whole-world pre-generation

**Decided:** Generate **the complete Terrain Diffusion base geography at world creation**, store it permanently, then **do not require Terrain Diffusion inference during gameplay or when reopening that world**.

This separates two very different jobs:

- **World creation (expensive, offline/batched):** tectonics, global fields, Terrain Diffusion, seam validation, compressed immutable geography.
- **Gameplay (bounded, streamed):** read saved geographical tiles, create local voxel detail and meshes as necessary, simulate active regions, apply persistent edits.

Benefits: no inference model resident in GPU memory during normal gameplay, shareable worlds without diffusion model weights, predetermined mountains/coastlines, and coherent rivers across the complete surface. This does **not** mean pre-generating every metre-scale voxel everywhere.

World creation must be resumable/transactional and able to fail without producing an apparently complete but partially missing world (proposed requirement).

## 5. Elevation resolution and storage

**Provisional elevation target:** approximately 30 m horizontal resolution, with an exact periodic raster of **32,768 × 32,768 samples** across 1,024 km, giving **31.25 m/sample**. This is a storage/indexing target, not a confirmed native Terrain Diffusion inference configuration.

- Total cells: **1,073,741,824**.
- Raw signed 16-bit elevation layer: **2 GiB**.
- At hypothetical 2:1–4:1 compression: **1 GiB to 512 MiB**; these are **illustrative ratios, not benchmarks**.
- Consider independent **512 × 512-sample tiles**, i.e. **64 × 64 = 4,096 tiles** globally. At 16-bit, each raw tile is 512 KiB.
- Store elevations in a documented, fixed world vertical datum, **not in playable voxel Y coordinates**.
- **Proposed quantization:** signed 16-bit at 0.5 m increments (~−16,384 m to +16,383.5 m); max rounding error 0.25 m before any later height-mapping transform. Range and datum require validation against generated extremes.
- **Proposed compressor:** Zstandard (Zstd), independently per tile, initially with a moderate compression level; compare raw Int16+Zstd with predictive encoding+Zstd on real outputs.
- Tile headers/index must include format revision, origin, dimensions, sample spacing, quantization/datum, checksum and generation metadata.

Global climate, biome, geology and hydrology fields should be stored at **appropriate, generally coarser** resolutions and compressed independently; they are *not* included in the 2 GiB figure. Whether a 90 m global layer plus denser selected areas is preferable remains open.

**Critical:** A 30 m elevation sample grid does **not** deliver 1 m voxel detail. Sub-cell rock, soil, small streams, cliffs and caves need separate deterministic synthesis.

## 6. Elevation-to-voxel height mapping

**Decided principle:** Keep original geographical elevation independently, then convert to playable vertical voxel coordinates using a declared, versioned world-height transform.

`Y_playable = Y_sea + height_transform(elevation_m, settings)`

- The height transform must honour sea level and the selected world height.
- Avoid unintentional clipping of peaks at the top of a 256/512/768/1,024-block world.
- Evaluate **uniform geographical scaling**, **nonlinear summit compression**, and possibly regional/local adjustments.
- **Do not claim Everest-height terrain fits at a 1 m/voxel physical scale** in a 512-high world: some scaling/distortion is unavoidable.
- The relationship between horizontal game metres/voxels and geographical kilometres is **open**. Uniform 10–15 geographical metres per game block is a *reference from a Vintage Story diffusion mod*, not our adopted setting.
- A transformed/quantized playable heightmap may be cached; the immutable original geographical map stays authoritative.

Whether gameplay mountains should preserve true physical height or use Vintage Story-style compressed geography is still unresolved. This decision affects slope, traversal distance, cave depth and geological interpretation.

## 7. Voxel architecture and persistence

**Agreed strategy:** Do **not** pre-create or save the complete three-dimensional voxel world. That would be impractical at the proposed horizontal area.

**Provisional playable geometry preference:** **regular hexagonal-prism voxels ("hexels")**, drawing from the existing [Hexel-Game-Engine](https://github.com/sdhaight1-hash/Hexel-Game-Engine) prototype. This is a preferred direction to evaluate, **not yet a completed engine migration**. The earlier 32³ cube-style chunk proposal is no longer the only assumed candidate.

**Provisional voxel plan:**

- Use axial **Q/R** horizontal hex coordinates and vertical **Y**; six lateral neighbours, plus above/below. Define true hexel radius/diameter, area, height, physical scale, and orientation before generating world assets or calculating water cell capacity.
- Retain the existing Hexel Engine **16 × 16 × 16 Q/R/Y chunk sections** as the initial prototype baseline; **benchmark** 32 × 32 × 32 hexel sections rather than committing to Vintage Story's cube chunk size.
- With 16-high sections, the 256/512/768/1,024 presets allow **16/32/48/64 vertical section indices** respectively; with 32-high sections, **8/16/24/32**.
- Define toroidal wrapping as two periodic **hex-lattice translations**. Simple independent `q % width` / `r % height` is only valid after the period vectors and physical seams are chosen; test neighbours, distances, geometry, players, rivers and edits across both edges.
- Keep the globally baked Terrain Diffusion elevation as **rectangular geographical raster data**. Sample its continuous/interpolated elevations at hexel centres; no need for a hexagonal diffusion file format.
- Create local editable 3D geometry deterministically from immutable elevation/geology/climate plus a pinned local generator version.
- Generate caves, overhangs, strata, ores, soil, vegetation and surface detail through dedicated 3D procedural systems, not by expecting Terrain Diffusion to output 3D voxels.
- Stream player-adjacent chunks and appropriate distant levels of detail; bound active CPU RAM, mesh buffers and GPU memory.
- Preserve *player/world edits* independently of the generated base, using sparse edits or versioned compressed chunk snapshots when a chunk becomes heavily modified.
- Cache regenerated, unmodified chunks only as an optional **disposable** performance optimization.

**Persistence invariant:** Regeneration or cache eviction must never restore mined ores, erase placed blocks or replace player construction. Version and/or retain generators needed to reproduce the base accurately across upgrades.

The raw fully allocated 1 m³ voxel storage for a kilometre-scale world is **petabyte-scale**, not a reasonable world-file design. Higher selected world height increases theoretical voxel address space, **not** the size of the fixed-resolution 2D global elevation layer.

The exact voxel representation (dense chunks, palettes, implicit solids, hybrid) remains an implementation decision. Chunk size is provisional, not benchmarked.

**Hexagonal implications to validate:** hex-cap and six-wall meshing/culling, shared-boundary chunk invalidation, collision, navigation, practical building/rectangular structures, and hex-face fluid transfer areas. Six horizontal neighbours do **not** guarantee better fluid accuracy or performance. Terrain geology, climates, immutable geographic storage and the global conservation rule remain independent of playable cell shape.

**Repository alignment note:** Hexel-Game-Engine's current README still describes an eventual **true sphere**, while this design selects a **flat torus**. Do not silently mix those topologies; reconcile the Hexel repository's design documentation when adopting this world model. The Voxel-Game-Engine repository still contains the procedural materials/asset editor and the present world-design document; no project merge is assumed.

## 7A. Asset-creation subsystems and game-content handoff

**Decided architectural principle:** The four working editor workspaces — **Nodes, Presets, Foliage and Assets** — are specialized, reusable **asset-creation subsystems of the greater game engine**. Retain their functionality and editable source data as the game runtime grows. The currently standalone authoring desktop app is a packaging/UI arrangement, **not a separate engine or a temporary prototype to replace**.

**Subsystem responsibilities:**

| Asset-creation subsystem | Creates and edits | Game-world consumer |
|---|---|---|
| **Nodes** | Deterministic tileable appearance graphs and albedo/roughness/metallic/height/derived-normal textures | Shared baked material/texture resources for renderer |
| **Presets** | Data-driven natural material/appearance recipes built using Nodes | Material library selected by geology, biome, plants or object definitions |
| **Foliage** | Seeded tree/shrub hexel or cube geometry, bark/leaves, cutout cards and visual wind parameters | Vegetation definitions/prefabs placed by ecology/biomes |
| **Assets** | Editable cube or hex-prism occupancy, authored objects, material bindings and JSON source models | Placeable/editable structures and objects |
| **Shared authoring preview** | Hex/cube exposed-face geometry, UV maps, existing wgpu preview and OBJ/MTL exports | Source of reusable geometric/rendering techniques; **not** automatically the streaming world renderer |

**Proposed integration:** Authoring **source asset** (graph/preset/vegetation recipe/voxel model) → explicit **validate/bake/publish** operation → **game-ready shared asset library** (stable asset IDs, versioned definitions, baked textures, reusable geometry or deterministic recipe, material metadata) → world-gen placement / game rendering. This pipeline, its exact compiled storage format, dependency handling and registry are **not yet implemented**. Preserve existing JSON authoring and OBJ compatibility exports; OBJ is not enough to represent editable in-game hexel cells.

**Runtime ownership boundary:** An asset definition says **what a tree, rock texture or structure is and looks like**; the world's tectonics, geology, ecology and gameplay decide **where it is placed, what it consumes, whether it grows/dies/is mined, and how it changes**. Physical water/ore/soil and player edits remain under conserved/persistent authoritative world state. Material texture height is not the game's geographic elevation field. Do not re-evaluate CPU texture graphs per block or simulation tick; bake/cache shared variants. Runtime water and ecology must not be implemented inside the asset editor.

**Compatibility and performance:** Continue to support the asset editor's cube mode even if playable terrain prefers hexels. Existing foliage/assets use **Q/R/Z** for local hex occupancy (Z vertical), while the proposed world uses **Q/R/Y**; an importer must map axes and physical scales explicitly. The existing `FoliageMesh` and one-asset GPU viewport are useful **asset-scale** implementations, not optimized full-world chunk meshing. A separately streamed chunk mesher and renderer will be needed.

**Persistence/revisions:** Game instances reference asset versions but own their **mutable gameplay state**. Changing or rebaking a source asset must not regenerate a chopped tree, recreate mined materials or overwrite player-edited structures. Asset-version pinning, runtime hot reload and migration strategy remain open.

**First proof:** Compile/bake two authored materials, one deterministic hexel tree and one editable hexel prefab; load them in a tiny chunked fixture through a game-side API without invoking the editor UI. Verify rendering, edit persistence, asset reuse and correct world-space coordinate conversion before adding a broad asset framework.

See [Asset-creation subsystems and game-runtime integration](asset-creation-subsystems.md) for the concrete current-code inventory, ownership matrix, validation plan and outstanding interface choices.

## 8. Geology, climate, hydrology and ecology

**Design direction, algorithms still open:**

- Tectonically informed geological regions, rock layers, deposits, caves and underground features.
- Large-scale plausible climate with west/east prevailing winds, repeating latitude-like belts, elevation effects, continental rain shadows, seas and seasonal variation.
- Periodic, connected basins and rivers across world seams; avoid independent chunk-level watershed calculations that contradict upstream geography.
- Biomes derived from climate, elevation, geology and hydrology rather than completely isolated biome noise.
- **Hierarchical ecological simulation** inspired by Eco: coarse world-wide aggregates, regional vegetation/wildlife/water state, and fine individual entities/physics around active players.
- Persist ecological effects and resource depletion where gameplay requires them.

### Finite water and hydrological simulation

**Decided design principle:** **Water is finite and globally conserved.** There are no unlimited source blocks, arbitrarily replenishing springs, infinite reservoirs, or rainfall that silently adds water to the world. The total planetary water inventory is accounted for across **oceans, rivers/lakes/wetlands, groundwater, snow/ice, atmospheric moisture, organisms, player containers, pipes and constructed reservoirs**. Transfers change location, phase, or owner—not total water mass. If water is chemically transformed or bound into another material, its water-equivalent mass must be explicitly accounted for rather than discarded. A closed world has no unexplained sources or sinks.

**Conservation invariant (conceptual):**

`total_water_before = total_water_after`

The invariant applies across region/chunk borders, toroidal wrap seams, changes in simulation detail, loading/unloading, persistence and multiplayer interactions. Use units and a precise accounting scheme that can represent partial-cell volumes; explore integer/fixed-point transfer amounts or a similarly auditable conservative method, including rounding remainders. The **physical/numerical solver is open**, but duplicating or deleting water during transfers is not acceptable.

**Decided behavioural goals:**

- Rivers follow real drainage structure, not arbitrary noise lines; channels descend according to water-surface potential and hydraulic conditions. Under pressure, local water may move uphill in elevation without violating physics.
- Terrain depressions collect water and may create lakes with spill elevations and overflow routes. Closed basins can instead lose water through evaporation or infiltration.
- Lakes and rivers have **finite stored volumes and variable water levels/discharge**. Droughts can dry riverbeds; storms and snowmelt can flood floodplains.
- Excavation, dams, canals, pumping, irrigation and mining can divert water, drain lakes, flood caves, and change groundwater availability without creating or destroying water.
- Rain/snow draw from atmospheric moisture; melt, evaporation, transpiration, infiltration and springs debit/credit their actual source/destination reservoirs.
- Oceans are **enormous finite reservoirs**, not infinite source blocks. Ordinary player-scale effects may be imperceptible, but ocean mass remains accounted for.
- Water may cross the X/Z periodic world seams with no loss, duplication or artificial boundary.

**Provisional multi-resolution architecture (not a chosen solver):**

1. **Global:** coarse ocean/atmosphere/ice/groundwater budgets and climate-driven transport, maintaining whole-world accounting without per-voxel ocean simulation.
2. **Regional:** connected catchments, river reaches, lakes with level–area–volume curves, springs and aquifers; store volume and transfers as persistent state.
3. **Local:** activate fine-grained surface/voxel fluid behaviour near player interaction, breaches, waterfalls, caves and floods; show currents that physically affect entities.

When transitioning between regional aggregates and locally active water cells, **transfer ownership of the volume**. Never leave the same water counted in both representations. On chunk unload, aggregate remaining local water back into persistent state, keeping river throughput and partial transfers consistent. Remote water can use animated shader waves/foam/spray without simulating each visible droplet; graphics are not a water source.

**World-creation integration:** After the tectonic and Terrain Diffusion surface is finalized, derive a **whole-world periodic drainage graph**, watersheds, river corridors, depressions, lakes and spillways; carve/refine appropriate channels and floodplains, then reconcile any affected flow paths. Initialize finite stores in oceans, lakes, soil, aquifers and snow/ice; atmospheric water is included in the budget. The ~31.25 m proposed global elevation raster cannot resolve every narrow stream or dam: finer riverbeds and local hydrology need deterministic sub-grid geometry. Compute actual playable lake/ocean storage capacities from the **final world-height-transformed terrain**, not directly from the original unscaled geographic elevations.

**Candidate local solver approaches to benchmark:** a conservative grid-based shallow-water scheme for surface channels/flooding, with a volume-transfer voxel solver for enclosed spaces; a cellular automaton can be a simpler local prototype. These are **not finalized choices**. Also unresolved: stable pressures, velocities, erosion, groundwater permeability, evaporation rates, numerical time steps, inactive-region updates and multiplayer authority.

**Reference concepts (not assumptions of full conservation in other implementations):**
- [Streams Reflowing (Modrinth)](https://modrinth.com/mod/streams-reflowing) — flowing river/terrain-water design reference; **do not assume it guarantees global water conservation**.
- [latticeGlade — *How I Made Advanced Procedural Voxel Water*](https://www.youtube.com/watch?v=ZxSRJJiw5Gk) — downhill terrain-guided river routing, lakes at depressions, waterfalls, water currents, visual foam, local cellular-automata water response when blocks are removed, rain visuals and distant animated water. The demonstration **does not establish whole-world mass conservation**.
- [Plate-local river generation](https://github.com/Davismk3/plate-local-river-generation) — river-generation technique credited by the video; investigate its region-based routing ideas for a tectonic world, while still validating global drainage.
- [Eco](https://play.eco/) — ecological consequence/resource-accounting inspiration; not a claim that its water simulation matches ours.

Not decided: tectonic solver, atmospheric and hydrological transport solvers, water accounting representation and numerical precision, ecology tick rates, region aggregation, cave/ore generation, or player-impact fidelity.

## 9. Performance, compression and generation/runtime separation

**Design objective:** Total world extent mainly affects **world creation duration and stored base data**, while gameplay memory and work scale primarily with *active* world regions.

- Generation is batchable; no whole-world diffusion tensors or voxel arrays resident simultaneously.
- Keep separately bounded geographic-tile, voxel, mesh and GPU resource caches.
- Use async requests and streaming; block generation and decompression should not stall render frames.
- Measure peak RAM/VRAM, world creation time, seam correctness, disk footprint, cold-load time, and worst-case exploration/teleport stutter.
- Test compression on representative smooth plains, oceans, extreme mountains, caves and heavily edited settlements before stating real-world ratios.
- Chunk read/decompress latency and editing costs matter at least as much as best-case compression ratio.

No end-to-end performance numbers or compression ratios have yet been demonstrated for this design.

## 9A. Reactive authoritative world state (SpaceTimeDB-inspired)

**Provisional architectural direction:** Build a **reactive, spatially scoped world-state system**, borrowing concepts from [SpaceTimeDB](https://spacetimedb.com/docs/intro/what-is-spacetimedb/) without adopting its database runtime, server or networking stack. Source design notes: *Reactive World Architecture: Pub/Sub, Dependencies, and World Fields* (Hexel Engine exploratory notes, 2026-11-01).

- **Authority:** Terrain edits, material occupancy, water volumes, entities and persistent environment state have a single authoritative owner. Meshes, local entity views, caches and render resources are **derived**, never competing sources of truth.
- **Controlled mutations:** Expose explicit edit/transfer APIs. Group related mutations where correctness requires atomicity. For finite water, a debit and its matching credit must commit together; no unbalanced intermediate state may become authoritative or persistent.
- **Structural dependencies (static):** A hex chunk's boundary faces depend on up to six horizontal and two vertical neighbours; hydrology depends on terrain/permeability, plants on moisture/climate, and navigation on occupancy. Use targeted direct invalidation where relationships are geometrically known, not a per-hexel listener allocation.
- **Dynamic interests:** Entities/players may subscribe to nearby chunks, threats, regional weather or relevant world fields; interest follows movement, provides initial state, delivers subsequent relevant updates and cleans up on exit/destruction.
- **State subscriptions vs transient events:** State interests require an initial snapshot plus future changes; events such as lightning or a discrete block edit need not replay for a newly interested consumer.
- **Change notifications:** Prefer **typed field/region identifiers, revision counters and dirty/invalidation flags**. Consumers query the current authoritative state at their scheduled update rather than trusting stale event copies.
- **Batching:** Deduplicate changes per chunk/field/phase; skip unrelated regions. Immediate per-cell callbacks and recursive event storms are not the default.
- **Simulation phases:** Climate → precipitation → hydrology → vegetation and similar feedback loops may be cyclic across time. Run systems in explicit phases or iterative solves, rather than treating the entire environmental dependency graph as a synchronous DAG.
- **High-frequency paths:** Per-frame transforms, individual water-cell exchanges and tight physics loops should use direct data access and efficient scheduling, not a generic pub/sub hop for each operation.
- **Local first, multiplayer ready:** Same conceptual ownership and spatial interests can inform network replication later. No distributed database or multiplayer requirement is implied by the architecture.

**Illustrative workflow: dam breach:** commit edited hexels → invalidate nearby fluid topology/meshes → schedule conservative lake-to-downstream water transfers → update lake and receiver volumes atomically → issue coalesced water-region revisions → recompute local render, physics, navigation and ecology as needed → persist authoritative water changes. When unloading a region, move water ownership to its coarse representation rather than retaining two counted copies.

**Incremental validation, not a universal framework rewrite:** (1) authoritative edit API, (2) exact neighbour/dirty-chunk invalidation, (3) bounded typed update queue, (4) region-interest changes as an entity moves, (5) one region-scoped derived field depending on terrain, (6) transactional lake transfer and unload/reload conservation. Profile queue depth, rebuild counts, water balance, CPU time and memory before generalizing.

**Open:** update ordering, event durability, subscription identity/lifecycles, thread partitioning, atomic region transfer strategy, persistence consistency and index layouts. Strict conservation is a previously decided invariant; the notification implementation is **not** decided.

## 9B. Rust dependency inventory for **this repository**

**Verified in this repository's [Cargo.toml](../Cargo.toml) (2026-11-01):** This is the **actual installed/declaration baseline** for the existing `texture-studio` asset editor, **not** the dependency list from another engine.

| Dependency declared here | Version / features | Current role |
|---|---|---|
| `eframe` | `0.37`, `wgpu` feature | Native egui app and its GPU backend |
| `bytemuck` | `1.25`, `derive` | GPU-compatible data casting |
| `serde` | `1`, `derive` | Serialization structures |
| `serde_json` | `1` | Project and asset JSON |
| `png` | `0.18` | PNG assets/export |
| `rfd` | `0.17` | Native file dialogs |
| `naga` (dev) | `30`, `wgsl-in` | WGSL validation in tests |

**Important correction to earlier discussions:** The list `bytemuck 1.21`, `image 0.25`, `glam 0.29`, `hexx 0.20`, `noise 0.9`, `pollster 0.4`, `tracing 0.1`, `tracing-subscriber 0.3`, `wgpu 24`, `winit 0.30` is from the **separate Hexel Engine prototype**, not from this repository's Cargo manifest. It is a source of **possible implementation choices**, not existing dependencies here. No external repo is part of this doc maintenance.

**Candidates for future world development — introduce only for measured needs:**

| Candidate | Why consider it | Current status |
|---|---|---|
| `hexx` | Axial hex coordinates, neighbours, layouts; periodic torus wrapper still custom | Preferred for hexel prototype, not declared here |
| `glam` | Camera, vectors and transforms if not met by existing framework | Evaluate against eframe's existing math/types |
| `noise` | Deterministic fine terrain/biome noise | Optional, not declared |
| `zstd` | Compressed immutable elevation tiles and chunk snapshots | Strong candidate when persistent tiles exist |
| `rayon` | CPU parallelism for measured heavy generation/meshing | Add after benchmarking |
| `redb` | Transactional embedded state for regions, player edits and finite resources | Evaluate; not committed |
| `crossbeam-channel` | Bounded worker/change queues | Evaluate against standard library and existing queues |
| `tracing`, `tracing-subscriber` | Structured tracing as simulation grows | Optional, not declared here |
| `image`, `pollster`, direct `wgpu` / `winit` | Possible low-level graphics/load/init needs | **Do not add reflexively**; the app already uses `eframe` with `wgpu` support |
| `serde` | Versioned persistent world schemas | **Already present**; extend its use rather than add it again |

**Upgrade discipline:** Do not replace the existing eframe/wgpu integration, introduce a second independently versioned wgpu dependency, or change the asset renderer simply because the hexel prototype used different versions. Test `hexx`/`glam` type compatibility if adopting them, and change Cargo only when actually implementing world features. A real database is **not** required merely to emulate SpaceTimeDB-style subscriptions.

**Scope:** No dependencies or source code are changed by this design document. Asset-authoring facilities already exist in this repository; a game-ready runtime asset registry is still proposed.

## 10. Technical references (inspiration, not implementation claims)

- **Vintage Story:** large configurable voxel worlds, 32³ chunking, geology, climate, vertical world-height presets, proportional sea level.
- **VS Terrain Diffusion mod (Techmo):** Terrain Diffusion-to-Vintage-Story conversion, scale and vertical-mapping strategies; a useful reference implementation, not an engine dependency.
- **Eco:** finite wraparound voxel worlds, globe-like presentation, ecology and persistent human/environment interactions.
- **Terrain Diffusion / InfiniteDiffusion:** surface-height and climate synthesis, hierarchical regional generation.
- **Tellus:** retained as research for geographic data handling; no longer the planned synthetic-world base.
- **Streams Reflowing and latticeGlade's voxel-water showcase:** references for river geometry, lakes, currents, waterfalls, responsive local fluids and shader-driven visuals; not proof of strict water conservation.
- **Hexel-Game-Engine:** existing Rust hexagonal-prism prototype and source for actual baseline dependency versions; its spherical-world vision needs reconciliation with the toroidal design.
- **SpaceTimeDB:** authoritative mutations, snapshot-plus-change subscriptions, spatial interests and transactional design patterns; not a required infrastructure dependency.

External references:
- https://github.com/xandergos/terrain-diffusion
- https://xandergos.github.io/terrain-diffusion/
- https://mods.vintagestory.at/show/mod/63560
- https://github.com/Techmo2/VS-Terrain-Diffusion
- https://modrinth.com/mod/streams-reflowing
- https://www.youtube.com/watch?v=ZxSRJJiw5Gk
- https://github.com/Davismk3/plate-local-river-generation
- https://github.com/sdhaight1-hash/Hexel-Game-Engine
- https://spacetimedb.com/docs/intro/what-is-spacetimedb/

## 11. Outstanding decisions (must not be silently treated as settled)

1. **Game voxel/metre scale:** does one voxel represent 1 m in-game, or a compressed geographic distance?
2. **Default height transform:** uniform/linear scaling versus nonlinear summit compression; how to preserve believable slopes.
3. **Final sea-level policy:** Vintage Story proportional rule by default (provisional), with or without lower-sea option.
4. **Terrain Diffusion resolution:** full 31.25 m global target versus multi-resolution global/land strategy.
5. **Tectonic + diffusion conditioning:** data contract, compatible scales, and testable integration.
6. **Periodic seams:** ensure both adjacent samples and conditioned diffusion output match across X/Z wraps.
7. **3D subsurface generation:** caves, ore, rock structure, material rules, and surface-to-voxel interpolation.
8. **Voxel chunk representation and persistence format:** including edits, snapshots, caches and generator version upgrades.
9. **Generation time/storage/performance budgets:** quantify on target hardware.
10. **Climate and ecological simulation detail:** periodicity and physical plausibility versus cost.
11. **Conserved-water implementation:** choose quantization/accounting, reservoir/basin and groundwater models, local fluid solver, active/inactive reconciliation, and validation limits; the **finite-water rule itself is decided**.
12. **Hexel architecture:** exact cell radius/height/scale, wrapping lattice periods, 16³ versus 32³ chunk performance, meshing, collision, water-face geometry and practical building rules.
13. **Reactive state design:** choose the smallest adequate typed invalidation, region interests, transaction boundaries, ordering and storage mechanism; do not assume SpaceTimeDB is a dependency.
14. **Rust dependency additions/upgrades:** benchmark and add only when a feature requires them; preserve **this repo's** eframe/serde/bytemuck baseline until compatibility tests justify upgrading. Hexel-prototype crate versions are not installed here.
15. **Asset publishing format:** source/compiled content schemas, stable IDs, cached material variants, runtime prefab conversion, local Q/R/Z → world Q/R/Y scale, version pinning and safe asset updates for existing world instances.

## 12. Initial validation plan

1. Implement a **small toroidal fixture**: opposite edges match for height, climate, biome and drainage, including cross-boundary neighbour lookups.
2. Create one test terrain region with frozen elevation data, and render it at **256/512/768/1,024** heights with their proportional sea levels; evaluate slopes, summits and cave room.
3. Prototype **16-bit tiled Zstd** storage; measure real ratios and random-tile read latency.
4. Generate local **16³ hexel** chunks using existing Hexel Engine as the starting baseline (benchmark 32³ hexel sections); persist edits over immutable base data and verify reload and regeneration preserve modifications.
5. Run an end-to-end **limited-area** tectonics → diffusion → voxel test, then test a wrapped seam. Do not commit to generating an entire 1,024 km world until memory, runtime and seam tests pass.
6. Add fixed-budget terrain streaming tests and document measured hardware and results.
7. Build a **small finite lake → player-dug channel → downstream reservoir** fixture. Dig a new outlet, allow water levels to change, then assert the total before and after is identical without rain/evaporation.
8. Test **lake/river boundary transfers across chunk unloads and both toroidal seams**, including local ↔ regional representation handoff, save/reload, parallel updates, and large player-built dams; never count the same water twice.
9. Test a closed **evaporation → atmospheric moisture → rain/snow → infiltration/runoff** cycle with explicit budgets and mass-balance diagnostics. Measure solver stability and worst-case active-region cost.
10. Validate **periodic axial hex-coordinate wrapping** across both lattice periods; confirm unique canonical cells, neighbour reciprocity, seam collision and hex-face fluid transfers.
11. Add one authoritative edit command, region-scoped typed dirty notification and an entity changing subscribed regions; verify only relevant meshes and fields rebuild.
12. Test an atomic finite-water transfer across active and inactive region state, including failure/rollback, save/reload and no duplicate water ownership.
13. Profile **this repository's actual eframe/wgpu + serde dependency stack** before introducing `hexx`, `glam`, `rayon`, `redb`, or dedicated message-queue crates; test any direct wgpu API change separately.
14. Verify an **authoring → game-ready asset** path for two baked materials, one deterministic tree and one editable hexel object, with a persistent world instance and shared material resources; do not run authoring texture graphs per visible hexel.

---

### Summary

**Chosen future-world direction (not implemented):** A **1,024 × 1,024 km**, finite **horizontally toroidal** synthetic world; **512-block default** vertical height with **256/768/1,024** options; **complete pre-generation and permanent compression of base Terrain Diffusion geography**; **no diffusion inference in gameplay**; **streamed, regenerable detailed voxel terrain**; **persistent independent edit data**; tectonics-first geography, **finite conserved water** and coherent global climate/ecology. **Preferred playable cell geometry is now hexagonal prisms (Q/R/Y)**, with the existing Hexel Engine as the implementation reference, pending lattice/chunk tests. A SpaceTimeDB-inspired reactive world-state design and selective Rust dependency additions are **proposals**, not implemented subsystems. **Nodes, Presets, Foliage and Assets are chosen as retained asset-creation subsystems of the greater engine**, currently exposed through the existing `texture-studio` desktop authoring app, with cube and hexagonal-prism asset support. The game-ready library, world runtime and integration pipeline have **not yet been implemented**.

**Important open problem:** How to map geographic kilometres and realistic mountain elevation into a fixed-height voxel game **without unacceptable slope distortion**. Do not hard-code a metre-per-voxel equivalence until this is resolved.
