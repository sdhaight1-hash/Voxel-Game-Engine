# Material authoring baseline — issue #10, Phase 1

This is a **measured CPU/debug-build reference**, not a claim about release-build
throughput, GPU frame time or the planned graph cache. Do not use these numbers
as hard performance targets across different runner hardware.

## Reproduction

Source: [Rust CI run 41508169877](https://github.com/sdhaight1-hash/Voxel-Game-Engine/actions/runs/41508169877)
at commit `3d5ad3f2f9a3ac04601c0273cf664e79b877a7f4`.

- GitHub-hosted runner, Linux x86-64, Rust stable, `cargo test` (debug profile).
- `available_parallelism()`: 4; the graph sampler is synchronous/CPU-based.
- `cargo test --all-targets` passed: 112 library, 10 desktop and 6 regression integration tests (the additional benchmark is ignored during this step).
- `cargo test --test material_legacy_regression -- --ignored --nocapture` then ran the measurement test successfully.
- Two successive samples, no explicit warmup; each figure is wall-clock elapsed time for a single graph RGBA render, excluding PNG encoding, egui, GPU resource creation and geometry.
- FNV-1a hashes identify the rendered RGBA bytes; they are **not** cryptographic or content-addressed cache IDs.

### Recorded reference output

| Graph output | px² | Run 0 (ms) | Run 1 (ms) | RGBA FNV-1a 64-bit |
| --- | ---: | ---: | ---: | --- |
| Legacy graph **albedo (constant color)** | 256 | 16.593 | 16.496 | `a632dee47d4a2325` |
| Legacy graph albedo | 512 | 65.297 | 64.785 | `d520d3e468c22325` |
| Legacy graph albedo | 1024 | 261.538 | 259.006 | `f37e68e416a22325` |
| Legacy graph albedo | 2048 | 1041.108 | 1036.755 | `9750cce2ce222325` |
| Warped seeded noise (albedo) | 64 | 5.673 | 5.654 | `5a194a1d0a8f2742` |
| Warped seeded noise | 128 | 22.011 | 22.107 | `f9946e26fd1e076e` |
| Warped seeded noise | 256 | 87.842 | 87.723 | `49b63f4a4bf09932` |
| Warped seeded noise | 512 | 350.990 | 360.585 | `147c1e5d52f40f19` |

The first run's `legacy_checker` log label was incorrect: it measured **albedo**
(channel 0), which is the constant-color input of the frozen legacy graph.
Commit `4d6f1fe` corrects the name to `legacy_color` and separately measures
**roughness checker** (channel 1). New measurements from that commit should not
be confused with the reference table above.

## Compatibility fixtures and verification

- `tests/fixtures/legacy_graph_v0.json`: frozen old-style node/links JSON
  without `voxel`, exercising legacy albedo, roughness, metallic and height.
  These are hand-authored representative fixtures, **not excavated historic user saves**.
- `tests/fixtures/legacy_asset_v1.json`: frozen old-style editable Asset with
  independent `material_graph` (which must not be stripped during migration).
- `tests/material_legacy_regression.rs`: fixed FNV-1a pixel snapshots
  (4×4 across four channels; plus verified 256/512/1024/2048 albedo), legacy
  Foliage/Flora loading, `hexel-game-tree-v1` occupied-cell ownership,
  serialization roundtrips and warp/coordinate sampling equivalence.
- Existing engine tests still validate the 1,039 material presets and all 43 Flora
  varieties. Existing desktop tests check geometry-only Asset history and GPU
  upload classification for palette, wind, geometry and texture changes.

## Manual editor-side invalidation profiling

To capture GPU-side **upload counts** and Foliage/Asset editor bake/generation
counts, run `VOXEL_PROFILE_MATERIALS=1 cargo run --release` and follow the
Oak/seed/season/bark/Asset-hexel sequence documented in the README. Record
counter deltas (rather than global totals) and compare on the same machine.
Resource-upload `cpu_ms` measures **CPU issue time**, not completion on GPU.
These measurements are **not** yet provided by headless GitHub CI.

For future graph-cache work, rerun both the fixed pixel hashes and the CPU
baseline on an identical runner and compare cold/warm results. Dynamic
season/wind uniforms must avoid remeshing/rebaking; static source graph
changes must invalidate their affected material resources.
