# Bounded graph-output cache — Issue #10 / Phase 3 initial slice

`src/render_cache.rs` supplies an **explicit owner-managed**, CPU-only
`GraphRenderCache`. The Nodes UI owns one 64 MiB cache for rendered
output channels, normal previews, selected nodes and graph thumbnails.

## Key and invalidation

A cached RGBA image is identified by:

- Bake algorithm version, requested output channel / Normal / node ID
- Width and height
- Only node IDs, operations and parameters in the requested output's
  **transitive upstream dependency closure**
- Only links within that required closure, including input indices

Canvas node positions and unrelated voxel viewport settings are omitted
because they do not influence the normalized 2D graph sampling. Editing a
parameter or link **in the requested output's dependency closure** yields a
new key; editing an unrelated branch does not invalidate that output.
Switching back after undo can reuse an older entry until LRU eviction. A change of size, target node or
channel cannot accidentally hit a previous image. The graph is validated
**before** checking the cache, so invalid data is never hidden by an old hit.

This first slice caches only **complete rendered outputs**. It does not
rasterize internal analytic subgraphs or change `Graph::sample`, and it does
not assume an upstream image can substitute for coordinate-transformed lazy
sampling. That matters for `Warp`, `Rotate`, `Tile`, `Swirl` and similar
operations which sample their inputs at new UVs.

## Memory and profiling

The 64 MiB cap is measured in resident cached RGBA bytes, not in extra
egui/wgpu allocations or Arc clones held externally. Eviction is LRU,
oversized outputs return successfully but are not retained; a zero-byte cap
disables retention. The cache exposes cumulative hits, misses, evictions,
current entries and resident/peak byte counts.

With `VOXEL_PROFILE_MATERIALS=1`, the Nodes preview emits a
`[material-profile] event=node_render_cache` line alongside the existing
material-bake, scene-build and GPU-upload counters. It does not claim
whole-editor or whole-world texture de-duplication.

## Correctness tests and next gate

The unit tests compare cached and uncached bytes, including a warped graph;
verify dependency-aware invalidation for parameters/links/channels/resolutions; test LRU cap
and oversize bypass; and ensure invalid projects fail even after an earlier
cache hit. The pre-existing frozen legacy 256/512/1024/2048 output hashes and
1,039 preset graph tests remain unchanged.

Next Phase 3 work needs a dependency-order/required-output analysis,
coordinate-aware intermediate execution classes, and profiling of expensive
shared subgraphs before claiming faster cold renders. This initial cache
primarily avoids repeated *complete-output* CPU renders; it does not yet
deduplicate per-node graph sampling or Foliage/Asset material bakes.
