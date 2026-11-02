# Tellus / real-Earth reconstruction — archived research reference

**Status:** **Archived/superseded world-mode proposal**; research reference only, not implemented or on the current roadmap. See [the authoritative procedural-world decisions](procedural-world-design-decisions.md) and [current synthetic generation architecture](world-generation-foundation.md).
**Project:** Voxel Game Engine (Rust + wgpu)
**Reference:** [Yucareux/Tellus](https://github.com/Yucareux/Tellus), especially [src/test](https://github.com/Yucareux/Tellus/tree/main/src/test).

> **Current decision (2026-11-01):** Our chosen default is **one finite 1,024 × 1,024 km toroidal synthetic world** built from **periodic tectonics + world-creation Terrain Diffusion**, with 512-block default vertical height, **hexagonal-prism playable voxels as the current preference**, persistent edits, hierarchical ecology, **globally finite/conserved water**, and a proposed reactive authoritative state system. We are **not** using Tellus as an Earth terrain foundation and have **not** selected two playable world modes. This document remains because its geodata handling, hydrology and test patterns may prove useful in the future. Do not interpret the historical mode-comparison table or work plan below as current commitments.

**Repository scope:** This repository currently implements a procedural materials and voxel asset editor; the synthetic world and the Tellus integration are **not implemented**. No external repository is part of this documentation update.
**Source license:** LGPL-3.0 ([Tellus LICENSE](https://github.com/Yucareux/Tellus/blob/main/LICENSE)); geospatial datasets and third-party assets have separate terms ([THIRD_PARTY_NOTICES.md](https://github.com/Yucareux/Tellus/blob/main/THIRD_PARTY_NOTICES.md)).

## What Tellus actually does

Tellus is a **Java mod for Minecraft** that builds Earth-scale voxel landscapes from real geographic datasets. This is fundamentally distinct from InfiniteDiffusion's generation of *synthetic* terrain and climate. Tellus includes terrain elevation and ocean bathymetry, categorical land cover, canopy height, Köppen climate classifications, hydrology, structures/roads, biomes and vegetation. Its gameplay-specific Minecraft integrations are not engine components we should transplant.

The original Tellus README explicitly states that its current workflow **requires an internet connection** to fetch external geographic and weather datasets on demand. An offline edition of our engine would need **prepackaged/cached geographic tiles**, a separately licensed offline data distribution strategy, or a synthetic fallback. Do not imply that Tellus is currently an offline procedural generator.

Key published source layers (each with distinct provenance/licensing and resolution):
- Mapterhorn global terrain elevation tiles, with resolution-aware sampling.
- OpenWaters Seascape bathymetry for ocean depth, with explicit ocean masks and coastal transitions.
- ESA WorldCover 2021 v200 10 m land-cover categories, selected overview levels and bounded COG HTTP byte-range cache.
- ETH Global Canopy Height 2020, 10 m, LERC/ArcGIS tiles; canopy height informs tree scale.
- Köppen–Geiger climate classification and other sea/weather datasets.
- OSM / Overture vector features, fetched via range access and used for water, roads, buildings and related features.

Consult [Tellus README](https://github.com/Yucareux/Tellus/blob/main/README.md) for specific data sources, availability, cache budgets and disclaimers. Do not redistribute datasets, tiles or models without checking each source's terms.

## Historical idea (superseded): two selectable world modes

| | Option 1: InfiniteDiffusion | Option 2: Tellus-inspired |
|---|---|---|
| Primary input | Seed, coordinates and pinned learned model | Georeferenced Earth datasets and map source versions |
| Geography | Fictional/generated and potentially unbounded | Real Earth; finite geographic domain, can wrap longitude |
| Core samples | Height and climate fields synthesized at requested positions | Height, bathymetry, biomes/climate, land cover, canopy, waterways, roads |
| Availability | Local compute and model weights required for learned inference | Remote geodata; offline only if datasets were cached or packaged |
| Reproducibility | Seed + frozen model + numerical/backend controls | Geospatial data versions + source coverage + transform settings |
| Voxel conversion | Deterministic geological detail and biome rules | Deterministic georeferenced terrain, hydrology and settlement conversion |
| Primary purpose | Large fictional worlds | Explorable real-Earth worlds |

**Historical proposal only:** the earlier plan was a single game engine with pluggable synthetic or Earth-derived surface-data providers, one chunk format, one renderer and one edit-persistence layer. It proposed a `WorldMode::Synthetic` / `WorldMode::Earth` setting. **Neither the dual-mode selection nor Tellus-backed Earth gameplay is an approved/current design requirement.** The current world uses synthetic periodic geography and does not require real-Earth datasets or a Tellus adapter. If Earth mode is reconsidered someday, it should be evaluated as a separate, explicit feature with its own georeferencing and licensing constraints.

## Historical two-provider contract (not active architecture)

The diagram below preserves the older integration proposal for reference. The **current** runtime contract is instead: immutable pre-generated periodic synthetic geography → deterministic on-demand **hexel** chunks → persistent authoritative edits and finite resource state → derived render/simulation caches. Terrain Diffusion is performed at world creation, **not via gameplay tile inference**. Global climate/hydrology are generated and persisted separately; changing player-built dams or waterways changes authoritative water state and invalidates interested systems.

```text
                    WorldConfig (world mode, source revisions, scale, seed)
                                      |
                   +------------------+------------------+
                   |                                     |
          Synthetic surface provider               Earth geo providers
          (InfiniteDiffusion / noise)               (DEM, ocean, cover,
                                                    climate, OSM, canopy)
                   |                                     |
                   +------------ Normalizer ------------+
                                        |
            typed SurfaceTile (world origin, cell size, elevation,
            valid/missing mask, climate, land cover, water, source versions)
                                        |
                       world-space hydrology and biome rules
                                        |
                      deterministic voxel chunk construction
                   (strata, caves, materials, roads, foliage, buildings)
                                        |
                    +-------------------+-------------------+
                    |                                       |
                 base chunk                       saved user edit deltas
                    |                                       |
                    +-------------------+-------------------+
                                        |
                   chunk streaming, LOD, gameplay and wgpu renderer
```

- World-space positions must use signed 64-bit coordinates with correct negative div/mod, so independent chunks have stable addresses.
- **Earth mode needs a georeferencing transform:** WGS84 longitude/latitude, a *declared* map projection (e.g. Web Mercator) and meters-per-block scale. Handle antimeridian wrapping and pole/latitude limits. Vertical-scale correction must be explicit rather than silently distorting slopes. Reference [EarthProjectionTest](https://github.com/Yucareux/Tellus/blob/main/src/test/java/com/yucareux/tellus/worldgen/EarthProjectionTest.java) and [TerrainHeightTransformTest](https://github.com/Yucareux/Tellus/blob/main/src/test/java/com/yucareux/tellus/worldgen/TerrainHeightTransformTest.java).
- **A valid zero-meter elevation is not a missing-data sentinel.** Store coverage and sample provenance independently. Ocean classification must not be inferred from height alone; refer to [TellusElevationSourceDecisionTest](https://github.com/Yucareux/Tellus/blob/main/src/test/java/com/yucareux/tellus/world/data/elevation/TellusElevationSourceDecisionTest.java).
- All provider reads are async/cancellable with explicit **pending / valid / unavailable** results; failed downloads must not poison disk caches by masquerading as permanent terrain, dry land or ocean.
- Preserve source resolution and confidence/provenance so distant LOD queries can use coarser source levels without corrupting near-field terrain. The same point queried at different zoom levels may differ; pin detail requirements per chunk.
- Persistent sparse player edits always override procedural terrain, for both world modes.

## Why the linked src/test directory matters

These are regression-test **examples for behavior**, not a library we can import directly:

| Test from Tellus | Engine invariant to reproduce in Rust |
|---|---|
| [EarthProjectionTest](https://github.com/Yucareux/Tellus/blob/main/src/test/java/com/yucareux/tellus/worldgen/EarthProjectionTest.java) | Coordinate/projection round-trip and latitude-dependent vertical scale |
| [TerrainHeightTransformTest](https://github.com/Yucareux/Tellus/blob/main/src/test/java/com/yucareux/tellus/worldgen/TerrainHeightTransformTest.java) | Vertical height bounds, scale settings, ocean compression |
| [HydrologyIndexExactnessTest](https://github.com/Yucareux/Tellus/blob/main/src/test/java/com/yucareux/tellus/worldgen/HydrologyIndexExactnessTest.java) | Indexed polygon/rivers queries must match direct geometric scans exactly |
| [TellusElevationSourceDecisionTest](https://github.com/Yucareux/Tellus/blob/main/src/test/java/com/yucareux/tellus/world/data/elevation/TellusElevationSourceDecisionTest.java) | Ocean mask, source fallback and missing-data handling |
| [TellusCacheManagerTest](https://github.com/Yucareux/Tellus/blob/main/src/test/java/com/yucareux/tellus/cache/TellusCacheManagerTest.java) | Cache accounting, controlled deletion and cancellation |
| [TellusBiomeLocatePolicyTest](https://github.com/Yucareux/Tellus/blob/main/src/test/java/com/yucareux/tellus/worldgen/TellusBiomeLocatePolicyTest.java) | Correct vertical biome sampling, including negative Y |

The test tree also contains tests for water flow, river corridors, geology, coastal transitions, underground generation, buildings, vegetation and LOD. Selectively adapt their *invariants* into independent Rust tests; do not copy their Java implementation.

**Current-world relevance:** The toroidal world must validate two-axis seams (not merely the Earth's longitude seam), and the hexel grid needs explicit Q/R lattice-period wrapping. Water must be accounted for across oceans, lakes, rivers, aquifers, weather, ice and player storage; published Earth datasets may inform **geometry**, but must never be treated as an unlimited water source. Any future terrain conversion also needs a tested geographic-to-hexel scale and vertical height transform.

## Historical Earth-mode implementation checklist (not the current roadmap)

1. **Historical:** common world schema with signed chunk keys, `SurfaceTile`, provenance and potential `WorldMode`, saved config and player edits. Current implementation priority is instead a **finite periodic** synthetic schema with unique canonical hexel locations and no required Earth mode.
2. **Future Earth-mode possibility only:** prototype WGS84 projection, elevation transforms, raster resampling and a cached tile loader with synthetic fixtures; verify antimeridian and latitude limits if ever adopted. These do not substitute for the current **two-axis torus** tests.
3. **Reusable idea:** map normalized geography to rock, soil, water and surface materials, then place existing foliage/asset presets by biome. The chosen playable representation is **hexagonal-prism** cells (provisional), irrespective of the geography source.
4. **Future Earth mode only:** add real-world data providers individually behind explicit network settings, source provenance, license documentation and bounded caches, after validating coverage and projection behaviour. No such adapter is scheduled by the current plan.
5. **Still useful for our current world:** keep bounded disk-tile, CPU geographic-tile, hexel chunk, mesh and GPU caches; long-distance teleport should read canonical saved geography without sequentially generating every intervening chunk.

## Copyright and distribution boundaries

Tellus **source code is LGPL-3.0**, while some constituent data and assets are CC BY, ODbL, Apache 2.0 or other licenses. **Do not copy Java code or sample assets into this repository** as a convenience. Reimplement algorithms from documented interfaces and behavioral requirements, with independent Rust tests. If we later directly reuse Tellus code, review LGPL linking/source and notice obligations first.

This is an **archived research candidate, not an active implementation plan**. No Tellus integration, Earth-mode selection or Earth-data download is implemented by this document. The authoritative chosen architecture is linked at the top.
