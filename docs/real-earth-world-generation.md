# World-generation option 2: Tellus-inspired real-Earth reconstruction

**Status:** Reference architecture and candidate world mode, **not implemented**
**Project:** Voxel Game Engine (Rust + wgpu)
**Reference:** [Yucareux/Tellus](https://github.com/Yucareux/Tellus), especially [src/test](https://github.com/Yucareux/Tellus/tree/main/src/test).
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

## Design decision: two selectable world *modes*, one engine

| | Option 1: InfiniteDiffusion | Option 2: Tellus-inspired |
|---|---|---|
| Primary input | Seed, coordinates and pinned learned model | Georeferenced Earth datasets and map source versions |
| Geography | Fictional/generated and potentially unbounded | Real Earth; finite geographic domain, can wrap longitude |
| Core samples | Height and climate fields synthesized at requested positions | Height, bathymetry, biomes/climate, land cover, canopy, waterways, roads |
| Availability | Local compute and model weights required for learned inference | Remote geodata; offline only if datasets were cached or packaged |
| Reproducibility | Seed + frozen model + numerical/backend controls | Geospatial data versions + source coverage + transform settings |
| Voxel conversion | Deterministic geological detail and biome rules | Deterministic georeferenced terrain, hydrology and settlement conversion |
| Primary purpose | Large fictional worlds | Explorable real-Earth worlds |

**Do not implement two world engines.** Implement **one** chunk format, asset system, edit persistence layer, gameplay runtime and wgpu renderer. Implement pluggable **surface-data providers**, so synthetic terrain synthesis and real-Earth raster/vector sampling feed the *same* subsequent voxel-generation pipeline. World selection is an explicit project setting (e.g., `WorldMode::Synthetic` or `WorldMode::Earth`), not a guess based on coordinates.

## Shared engine contracts

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

## Recommended incremental implementation

1. **Shared world schema first:** signed chunk keys, `SurfaceTile`, source validity/provenance, `WorldMode`, saved generator configuration and persistent user edit deltas. Keep external providers behind a trait. No Earth datasets needed to test this.
2. **Local fixture-based Earth mode:** implement longitude/latitude projection, elevation transforms, raster resampling and a simple cached tile loader with small **self-generated synthetic fixtures**. Test negative coordinates, antimeridian, source resolution, missing-data semantics and shared tile seams.
3. **World-to-voxel stage:** classify rock, soil, water and surface materials from normalized terrain; place existing foliage/asset presets by vegetation/biome tags. Use the same rendering and edit path as InfiniteDiffusion mode.
4. **Network-capable geospatial adapters:** add one data source at a time behind explicit user/network settings, provenance, cache limits, cancellation and source license documentation. Validate real-world accuracy against controlled test coordinates. Only then expand to climate, land cover, ocean depth, canopy, water vectors, roads and buildings.
5. **Large-world performance:** maintain distinct bounded disk tile, CPU world-tile, chunk mesh and GPU caches, plus LOD terrain prefetch; test long-range teleports without sequential generation.

## Copyright and distribution boundaries

Tellus **source code is LGPL-3.0**, while some constituent data and assets are CC BY, ODbL, Apache 2.0 or other licenses. **Do not copy Java code or sample assets into this repository** as a convenience. Reimplement algorithms from documented interfaces and behavioral requirements, with independent Rust tests. If we later directly reuse Tellus code, review LGPL linking/source and notice obligations first.

This is an **architecture candidate recorded for future implementation**. No Tellus integration or Earth-data download is implemented by this document.
