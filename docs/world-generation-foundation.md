# Architecture decision: InfiniteDiffusion-inspired world generation

**Status:** Foundational direction / design decision (not yet a running terrain implementation)
**Project:** Voxel Game Engine (Rust + wgpu)
**Research basis:** Alexander Goslin, *InfiniteDiffusion: Bridging Learned Fidelity and Procedural Utility for Open-World Terrain Generation*, SIGGRAPH 2026.

Reference sources:
- Project: https://xandergos.github.io/terrain-diffusion/
- Implementation: https://github.com/xandergos/terrain-diffusion
- API contract: https://github.com/xandergos/terrain-diffusion/blob/master/API_README.md
- Infinite Tensor: https://github.com/xandergos/infinite-tensor
- Minecraft integration example: https://github.com/xandergos/terrain-diffusion-mc
- 30m model: https://huggingface.co/xandergos/terrain-diffusion-30m

## Architectural commitment

Voxel Game Engine is a *game engine*, not only a procedural texture editor or a terrain renderer. Its world layer must support:

1. Unbounded coordinate space and bounded runtime memory.
2. Seed- and coordinate-addressable **generated base world**. Request order must not affect the result.
3. Random-access chunk generation and asynchronous streaming, including far-away teleports.
4. Deterministic regeneration from a pinned world configuration and model revision.
5. Editable, persistent player/world changes layered **over** regenerable content.
6. GPU rendering through wgpu, independent from the choice of terrain inference backend.
7. Terrain-informed climate, biomes, vegetation, water and procedural material selection.

InfiniteDiffusion's core idea (lazy, stateless, coordinate-addressable learned synthesis with a bounded cache) is the *design foundation*. The external Terrain Diffusion models are an important generator **backend**, not a required dependency of the entire game loop. All runtime/gameplay APIs consume normalized terrain data; they do not call Python or a neural network directly.

## Minimum data flow

```text
WorldConfig (seed, generator/model revision, resolution, world scale)
                |
                v
TerrainProvider: request geographic tiles by global coordinates
       |                              |
   deterministic fallback       learned Terrain Diffusion
       |                              |
       +------ normalized elevation + climate samples
                                      |
                                biome/erosion rules
                                      |
                             3D voxel density/materials
                                      |
                  +-------------------+-------------------+
                  |                                       |
             regenerated base                   persistent edit deltas
                  |                                       |
                  +-------------------+-------------------+
                                      |
                        chunk mesh / gameplay / physics
                                      |
                         wgpu renderer and world editor
```

The same base can be consumed by the editor and by the playable runtime. The procedural node graph and existing tree/leaf asset system stay intact, and become part of biome-aware terrain material and vegetation placement.

## Terrain data contract

Begin with 2D spatial tiles holding at least:
- Elevation in meters.
- Annual mean temperature and temperature seasonality.
- Precipitation and precipitation seasonality.
- Tile origin, sample spacing, dimensions and generator revision.

The upstream REST prototype supports `GET /terrain?i1=...&j1=...&i2=...&j2=...&scale=...` and returns int16 little-endian elevation followed by 4 interleaved little-endian float32 climate channels, with `X-Width` / `X-Height` response headers. This format is an **integration adapter**, not our permanent world-file format. Its API examples are explicitly in terms of a 90m base resolution; confirm coordinates/scale against the selected model before shipping the 30m variant.

**Important:** this upstream output is a 2D height/climate field, *not* a 3D voxel density volume. The engine must build playable-resolution topsoil, rock, water, caves, overhangs and custom blocks using additional deterministic rules or separate models. A 30m/px source cannot directly supply one-meter block-level detail. Treat it as large-scale geography and climate.

## Seed and tile invariants

- Global positions use signed 64-bit voxel/world coordinates. Convert to tile/chunk keys with Euclidean division so negative coordinates work.
- Define tile content using `(world_seed, generator_id, generator_version, coordinate, scale, settings_hash)`. Versions and settings are saved with the world.
- Overlapping or adjacent tile queries must agree on their shared samples. Tests must compare the generated seam.
- Parallel, reversed-order and distant teleport requests must not change a tile's generated content.
- A cache is a *performance optimization*, not an authority. Keep separate bounded CPU sample, mesh and GPU resource caches.
- GPU/AI inference can exhibit floating-point differences across providers; do not claim bit-for-bit cross-driver determinism without measuring it. Persist canonical sample tiles when strict multiplayer replay or save reproducibility needs it.
- Author modifications (digging, building, planting) must persist as sparse deltas/overrides; regenerated procedural base data must never overwrite those edits.

## Implementation sequencing

### M1 — World contract, no heavyweight ML runtime
Build a minimal `TerrainProvider` trait and async/request-queue terrain-tile cache; define signed coordinates, world config, sample schema and chunk identity. Supply a small deterministic procedural provider for tests and offline fallback. Test negatives, seams, seed identity, order-independent queries and bounded cache eviction.

### M2 — Exploreable chunked world
Convert terrain samples into chunk occupancy/material slots, greedy/visibility-aware mesh generation and wgpu rendering; implement camera/player position, chunk streaming and far-distance teleport. Attach the current procedural materials and voxel foliage/tree assets by biome. Persist sparse world-edit deltas.

### M3 — Terrain Diffusion reference integration
Integrate a separately managed local inference worker/adapter using the published REST interface as the first proof of concept. Do not run inference on the UI/render thread. Pin and record model revision, request resolution, seed and source geometry. Test that remote-worker failure falls back cleanly and never corrupts saved chunks.

### M4 — Production inference strategy
Evaluate native ONNX Runtime or a dedicated process and platform-specific providers (CUDA, DirectML, CoreML/CPU), model packaging, warm-up, cache budgets, streaming latency and VRAM. wgpu/Vulkan rendering does **not** automatically run PyTorch/diffusion models; inference acceleration is a separate design decision. Optionally replace the initial Python adapter only after measuring parity and performance.

## Not implied by this decision

- No claim of a fully 3D diffusion-generated voxel volume.
- No requirement that players run NVIDIA CUDA, Python, or download model weights just to use the editor.
- No assumption that the paper's "O(1) random access" means constant real-time latency independent of hardware, model, batch size, cache and requested tile count. It concerns independence from the *distance* to the requested region.
- No obligation to train new models before building a functioning world runtime.
- No copying of original source or model weights without separately checking terms and preserving copyright/license notices. The cited project source and 30m Hugging Face model currently identify as MIT; verify each asset's license before redistribution.

## Initial measurable acceptance criteria

- Same world seed, provider revision and coordinates produce matching terrain whether requested first, last, or concurrently.
- Adjacent tiles agree along their boundary without visible cracks; negative coordinate edges work.
- On-demand world generation does not block the render thread.
- A far teleport can request target chunks without generating every chunk along the path.
- Unloaded chunk caches stay within fixed memory budgets.
- Player-edited blocks survive unloading/reopening and take precedence over regenerated base terrain.
- World rendering, material presets and planted foliage all consume terrain samples without being coupled to the inference backend.
