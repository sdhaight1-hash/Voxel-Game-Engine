# Procedural World — Design Decisions (v0.1)

**Status:** Working design; documents agreed direction, provisional defaults, and open engineering decisions. **Not an implementation claim.**  
**Recorded:** 2026-11-01  
**Project:** Voxel Game Engine (Rust + wgpu)

> This document records the current direction established in design discussion. It **supersedes the proposed unbounded, gameplay-time Terrain Diffusion workflow as the default synthetic-world direction** in [world-generation-foundation.md](world-generation-foundation.md). The older [Tellus/Earth-world research](real-earth-world-generation.md) remains reference material, but Tellus is **not** the chosen foundation for this synthetic toroidal world. Do not treat experimental techniques, sizes, compression ratios, or algorithms below as implemented or benchmarked.

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

`Y` is vertical; `X` and `Z` are horizontal. `H` is the number of valid vertical block positions, `0 ... H-1`.

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

**Provisional voxel plan:**

- 32 × 32 × 32 voxel chunks, inspired by Vintage Story.
- Each vertical preset implies 8, 16, 24 or 32 possible chunk levels, respectively.
- Create local editable 3D geometry deterministically from immutable elevation/geology/climate plus a pinned local generator version.
- Generate caves, overhangs, strata, ores, soil, vegetation and surface detail through dedicated 3D procedural systems, not by expecting Terrain Diffusion to output 3D voxels.
- Stream player-adjacent chunks and appropriate distant levels of detail; bound active CPU RAM, mesh buffers and GPU memory.
- Preserve *player/world edits* independently of the generated base, using sparse edits or versioned compressed chunk snapshots when a chunk becomes heavily modified.
- Cache regenerated, unmodified chunks only as an optional **disposable** performance optimization.

**Persistence invariant:** Regeneration or cache eviction must never restore mined ores, erase placed blocks or replace player construction. Version and/or retain generators needed to reproduce the base accurately across upgrades.

The raw fully allocated 1 m³ voxel storage for a kilometre-scale world is **petabyte-scale**, not a reasonable world-file design. Higher selected world height increases theoretical voxel address space, **not** the size of the fixed-resolution 2D global elevation layer.

The exact voxel representation (dense chunks, palettes, implicit solids, hybrid) remains an implementation decision. Chunk size is provisional, not benchmarked.

## 8. Geology, climate, hydrology and ecology

**Design direction, algorithms still open:**

- Tectonically informed geological regions, rock layers, deposits, caves and underground features.
- Large-scale plausible climate with west/east prevailing winds, repeating latitude-like belts, elevation effects, continental rain shadows, seas and seasonal variation.
- Periodic, connected basins and rivers across world seams; avoid independent chunk-level watershed calculations that contradict upstream geography.
- Biomes derived from climate, elevation, geology and hydrology rather than completely isolated biome noise.
- **Hierarchical ecological simulation** inspired by Eco: coarse world-wide aggregates, regional vegetation/wildlife/water state, and fine individual entities/physics around active players.
- Persist ecological effects and resource depletion where gameplay requires them.

Not decided: tectonic solver, atmospheric solver, water mass accounting, ecology tick rates, region aggregation, cave/ore generation, or player-impact fidelity.

## 9. Performance, compression and generation/runtime separation

**Design objective:** Total world extent mainly affects **world creation duration and stored base data**, while gameplay memory and work scale primarily with *active* world regions.

- Generation is batchable; no whole-world diffusion tensors or voxel arrays resident simultaneously.
- Keep separately bounded geographic-tile, voxel, mesh and GPU resource caches.
- Use async requests and streaming; block generation and decompression should not stall render frames.
- Measure peak RAM/VRAM, world creation time, seam correctness, disk footprint, cold-load time, and worst-case exploration/teleport stutter.
- Test compression on representative smooth plains, oceans, extreme mountains, caves and heavily edited settlements before stating real-world ratios.
- Chunk read/decompress latency and editing costs matter at least as much as best-case compression ratio.

No end-to-end performance numbers or compression ratios have yet been demonstrated for this design.

## 10. Technical references (inspiration, not implementation claims)

- **Vintage Story:** large configurable voxel worlds, 32³ chunking, geology, climate, vertical world-height presets, proportional sea level.
- **VS Terrain Diffusion mod (Techmo):** Terrain Diffusion-to-Vintage-Story conversion, scale and vertical-mapping strategies; a useful reference implementation, not an engine dependency.
- **Eco:** finite wraparound voxel worlds, globe-like presentation, ecology and persistent human/environment interactions.
- **Terrain Diffusion / InfiniteDiffusion:** surface-height and climate synthesis, hierarchical regional generation.
- **Tellus:** retained as research for geographic data handling; no longer the planned synthetic-world base.

External references:
- https://github.com/xandergos/terrain-diffusion
- https://xandergos.github.io/terrain-diffusion/
- https://mods.vintagestory.at/show/mod/63560
- https://github.com/Techmo2/VS-Terrain-Diffusion

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
10. **Climate, hydrology and ecological simulation detail:** periodicity and physical plausibility versus cost.

## 12. Initial validation plan

1. Implement a **small toroidal fixture**: opposite edges match for height, climate, biome and drainage, including cross-boundary neighbour lookups.
2. Create one test terrain region with frozen elevation data, and render it at **256/512/768/1,024** heights with their proportional sea levels; evaluate slopes, summits and cave room.
3. Prototype **16-bit tiled Zstd** storage; measure real ratios and random-tile read latency.
4. Generate local **32³** chunks and persist edits over immutable base data; verify reload and regeneration preserve modifications.
5. Run an end-to-end **limited-area** tectonics → diffusion → voxel test, then test a wrapped seam. Do not commit to generating an entire 1,024 km world until memory, runtime and seam tests pass.
6. Add fixed-budget terrain streaming tests and document measured hardware and results.

---

### Summary

**Chosen direction:** A **1,024 × 1,024 km**, finite **X/Z toroidal** synthetic world; **512-block default** vertical height with **256/768/1,024** options; **complete pre-generation and permanent compression of base Terrain Diffusion geography**; **no diffusion inference in gameplay**; **streamed, regenerable detailed voxel terrain**; **persistent independent edit data**; tectonics-first geography and coherent global climate/ecology.

**Important open problem:** How to map geographic kilometres and realistic mountain elevation into a fixed-height voxel game **without unacceptable slope distortion**. Do not hard-code a metre-per-voxel equivalence until this is resolved.
