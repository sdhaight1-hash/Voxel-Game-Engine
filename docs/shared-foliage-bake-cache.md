# Shared bark/leaf authoring bake cache — issue #10, Phase 3 follow-up

`FoliageBakeCache` reuses *final baked RGBA* leaf and bark textures between
Tree Designer and editable Asset preview paths. It is a separate,
owner-managed **32 MiB LRU cache** within the editor, not a global static
resource and not a replacement for the existing `FoliageMaterials` graphs.

## Recipe identity and invalidation

The leaf key includes its assigned Leaf graph, Leaf Cutout graph, species
silhouette, fruit color, cutout threshold, base leaf tint and output size.
The bark key includes its assigned Bark graph, optional tint strength, base
bark tint and output size. The keys use normal serialized graph authoring
data; this version conservatively includes canvas positions/voxel metadata
when present within a graph. Such unrelated edits may cause *extra misses*,
never incorrect pixel reuse. Refining canonical keys is a separate gate.

Species season gradients are **not** in either key. They remain dynamic GPU
palette data, so changing seasons does not require a new cutout or bark
texture. Geometry seeds, mesh cells, wind and asset provenance are absent;
they do not alter either material bake. Bark edits cannot invalidate an
unchanged leaf bake, and leaf edits cannot invalidate an unchanged bark bake.

Every request still validates the authoring inputs before a hit. The same
legacy `bake_leaf` and `bake_bark` implementations execute on a miss. A
successful hit returns an `Arc<[u8]>` without recalculating the graph. The
egui texture upload still receives its own RGBA buffer and the wgpu upload
diff logic remains unchanged.

## Memory and profiling

The cap counts only cache-resident RGBA bytes. Caller-held Arc clones and
egui/wgpu upload buffers are not included. Oversized textures are generated
normally but not stored; LRU eviction removes the least recently used
entry. Nothing persists between editor launches.

With `VOXEL_PROFILE_MATERIALS=1`, the editor logs
`event=foliage_bake_cache` with hits/misses/evictions and current/peak
resident bytes. The existing `leaf_bakes` and `bark_bakes` counters now
increment for **actual cache misses** (bake attempts), not a successful
reuse request. A miss that returns an evaluation error is also counted as
an attempted bake.

## Regression and boundaries

Tests compare the cached bytes to the existing uncached functions, confirm
generated trees and converted Assets share entries, verify per-role and
seasonal invalidation, check tint/size/opacity sensitivity, guard invalid
inputs and enforce memory bounds.

This is authoring-time, **per-process** recipe reuse. It does not yet add
runtime world-chunk texture residency, persistent disk caching, node-level
execution plans or data sharing between independent processes. Do not
claim end-to-end FPS improvements from these cache-hit metrics.

Run `cargo test --all-targets` in CI and compare against the frozen legacy
pixel hashes before merging.
