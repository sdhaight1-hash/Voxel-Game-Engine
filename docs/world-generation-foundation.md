# Synthetic world generation — current design and InfiniteDiffusion research

**Status:** Current **proposed** synthetic-world architecture, not implemented. Reconciled with [Procedural World Design Decisions](procedural-world-design-decisions.md) on 2026-11-01.
**Project:** Voxel Game Engine (Rust + wgpu); currently a procedural material and voxel asset editor, **not** a working planetary terrain engine.
**Earlier title:** “World-generation option 1: InfiniteDiffusion-inspired synthetic worlds.” The original unbounded, inference-on-demand plan is superseded for our default world.

> **Source of truth:** [Procedural World — Design Decisions](procedural-world-design-decisions.md) is the authoritative design record for world size, topology, vertical presets, finite water, hexel preference, persistence, reactive state and open decisions. This page goes deeper on the **synthetic generation pipeline** and retains research that might be useful in its world-creation backend. It does not specify an additional world mode.

## Current architectural direction

- **Finite two-axis periodic torus**, with a nominal **1,024 × 1,024 km** geographic footprint. A flat world grid that wraps in both horizontal directions is intentional; a sphere is not the base geometry.
- **Tectonics first:** generate periodic large-scale plates, uplift, subsidence, continents and ocean basins. Compute climate/hydrological constraints informed by geography.
- **Terrain Diffusion during world creation only:** refine global surface geography in bounded batches/tiles. Persist the **complete** canonical surface so gameplay does not depend on model inference or Python.
- **Precompute geography, not all 3D voxels:** save periodic elevation, drainage networks, geology/climate maps and initial reservoir information; generate detailed editable chunks on demand from saved geography and pinned deterministic local rules.
- **Playable geometry preference:** hexagonal prisms with axial Q/R and vertical Y; chunk dimensions and hexel physical scale are not finalized. A square/rectangular geographic raster is still suitable for diffusion output and map storage.
- **Persist edits and finite resources:** regenerated base chunks may be discarded; changes to blocks, mined deposits, constructed works, water reservoirs and ecosystem state must survive reload and cannot be restored merely by rerunning a generator.
- **Reactive world-state direction:** authoritative state, controlled edits and conservative water transfers, coalesced spatial invalidation and region/field interests. This remains a proposal to test, not an installed SpaceTimeDB runtime.

These are **design choices**, not claims about existing code. The current repository's working functionality is focused on procedural materials, foliage and editable voxel assets.

## Pipeline at world creation

```text
World configuration (seed, finite periodic domain, height preset, model revisions)
  -> periodic synthetic tectonic/continent model
  -> broad climate, ocean and rainfall constraints
  -> batched Terrain Diffusion surface refinement
  -> reconcile periodic seams and coastlines
  -> final drainage/watershed/lake/spillway pass and local channel carving
  -> initialize accounted finite water reservoirs
  -> canonical elevation + environmental fields -> tiled compressed world data
  -> mark world generation complete only after validated persistent output

During gameplay:
  saved geographic tiles + deterministic local geology/terrain detail
  -> streamed hexel chunks + persistent user/world edit overrides
  -> meshes, physics, AI and ecological field updates
  -> wgpu renderer
```

Terrain Diffusion is an **elevation/surface-field model**, not a volumetric hexel generator. Underground cavities, strata, ores, overhangs and one-hexel-scale surface detail require independent procedures. The current discussion has **not validated** a conditioning API that will reliably make Terrain Diffusion respect tectonic constraints or periodic seams; the integration must be proven at limited scale.

## Geographic data and storage contract

Each canonical tile should record:

- Origin and span in the **finite periodic geographic domain**, sample spacing, dimensions, datum, orientation and wrap convention.
- Quantized elevation in geographic units, not already mapped to a playable Y level; coverage/validity flags must be distinct from a valid zero elevation.
- Generator/model identifiers, seed, settings/revisions and format version, plus checksums/integrity information.
- Separate fields for geology, precipitation/seasonality, temperature/seasonality, ocean mask, watersheds, river routes and relevant ecological inputs, at resolutions appropriate to each field.

**Provisional storage target:** 32,768 × 32,768 elevation samples across 1,024 km, exactly **31.25 m** spacing; 16-bit storage is **2 GiB raw** for elevation alone. Consider signed 16-bit values quantized at 0.5 m vertical increments, then independent **512 × 512 sample tiles** compressed with Zstd. Those are engineering starting points; quantization, compression ratios, model output resolution, height ranges and total disk footprint still need benchmarks. The entire environmental state is **not** 2 GiB.

**Playable height** is separately selected as 256, **512 default**, 768 or 1,024 Y levels. A Vintage Story-like proportional sea level is provisional (at 512: Y220). How many geographic metres each hexel represents horizontally and the vertical height mapping are **open**; do not assume square grid pixels, metres or elevation samples equal hexel cells.

**Hex-grid caveat:** map geographic sample coordinates to axial Q/R via an explicitly chosen layout and two valid lattice-period translations. Prove reciprocal neighbours, unique canonical cells, equal geometric seams and height continuity when crossing either periodic boundary. Do not simply apply independent modulo to geometric world-space coordinates without defining these periods.

## Runtime contract and deterministic regeneration

- Read and decompress **already-created** surface tiles asynchronously around active regions. Far teleport should not require generating every area along the path.
- Use deterministic local 3D generation keyed by world seed, canonical cell/chunk identity, local generator revision and immutable input-map versions. Preserve or migrate this versioning so updates cannot change terrain underneath player edits.
- Keep bounded independent geographic tile, voxel occupancy, mesh and GPU caches. Eviction must never delete authoritative edits or finite water quantities.
- Provide persistence that atomically accounts for water moving between reservoirs/chunks; do not publish or save only the debit half of a transfer.
- Generate off-thread where appropriate; high-frequency fluid-cell transfers and physics should not be routed through one generic pub/sub callback for every cell.
- Climate, water, ecology and landscape fields should be periodically updated at global/regional/local scales, while **water mass remains finite and conserved** across these representations.
- Derived outputs such as chunk meshes, local AI views, navigation and lighting should invalidate only when relevant world revisions change.

## Terrain Diffusion / InfiniteDiffusion research retained

The earlier InfiniteDiffusion proposal described **lazy, stateless, coordinate-addressable learned synthesis** at arbitrary positions and a bounded inference cache. It also proposed a second, Tellus-derived Earth-data world mode. That is **historical research**, not the current default world topology or gameplay inference strategy.

Reusable lessons:

- Batch and cache expensive inference at **world creation**; separate a model adapter from engine runtime code.
- Preserve canonical generated samples: floating-point inference and GPU backends are not guaranteed to reproduce bit-for-bit identical outputs without testing.
- Independent overlapping tile requests should agree at seams; query order, parallel completion and tile generation order must not alter canonical results.
- Use a local procedural fixture provider to test the world storage schema and seam logic without requiring inference hardware.
- Read external model/source licenses and notices before bundling code, weights or data. A published project's licensing may change; confirm at integration time.
- The original upstream REST prototype described elevation encoded as int16 and interleaved climate float32 channels. Treat that protocol as an **adapter reference**, not our persistent world-file schema. Its examples targeted a 90 m base, so actual coordinates and 30 m model behaviour need validation.

Research references:
- [Terrain Diffusion project](https://xandergos.github.io/terrain-diffusion/)
- [Terrain Diffusion implementation](https://github.com/xandergos/terrain-diffusion)
- [API reference](https://github.com/xandergos/terrain-diffusion/blob/master/API_README.md)
- [Infinite Tensor](https://github.com/xandergos/infinite-tensor)
- [Terrain Diffusion Minecraft integration](https://github.com/xandergos/terrain-diffusion-mc)
- [30 m model](https://huggingface.co/xandergos/terrain-diffusion-30m)

## Incremental implementation and acceptance tests (not yet implemented)

1. **Finite domain fixture:** define immutable world config and test periodic geographic tiles, wrapped axial neighbours and stable canonical identities without inference.
2. **Compressed world store:** write/read multiple 16-bit elevation tiles with checksums, revisions, bounded random access and measured Zstd compression/decompression.
3. **Limited-area geology → diffusion test:** establish what conditioning is actually possible; compare adjacent tiles and periodically wrapped seams.
4. **Hydrology from finalized geography:** calculate reservoirs, watersheds, spill elevations and drainage connections, including across the torus; initialize and audit a **finite** water inventory.
5. **Local hexel runtime:** stream and mesh hexel chunks from canonical geography plus deterministic microterrain, caves and geology, preserving sparse player modifications.
6. **Reactive validity and water transaction fixture:** editing a boundary cell rebuilds only necessary neighbours; dam breach moves water with equal debit/credit, including unload/reload and seam crossings.
7. **Benchmark before scaling to 1,024 km:** measure inference time, disk, load latency, peak RAM/VRAM, frame impact and persistent data size; don't claim the full world is practical until measured.

**Out of scope for this document:** implementation of a second Earth-data world mode, realtime terrain inference during gameplay, a true spherical planet, server deployment, or universal object-level event buses.
