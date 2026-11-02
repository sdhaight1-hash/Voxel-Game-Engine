# Organic, hexel-owned trees — implementation and limits

**Status:** Implemented in the Rust authoring engine; this is not yet a game-world ecology, felling or chunk persistence system. Shared by **all 43** Flora presets through the unchanged `FoliageProject` / `FloraPrefab` pipeline.

## What is authoritative?

- **Physical tree** — one canonical map of occupied 1 m regular **hexagonal-prism** cells with axial Q/R horizontal coordinates and Z vertical in the authoring editor. Tree cells remain individually selectable, paintable, removable and serialized in `VoxelAsset` and exported `hexel-game-tree-v1` data.
- **Biological/authoring structure** — parent-before-child `TreeBranch` IDs and `BranchSpan` paths. New generated trees also include **branch ID 0** spans for the continuous main trunk, including seed-specific lean, taper and root flare. These are not extra world entities or additional occupied blocks. Older loaded assets with no trunk spans retain their original full hexel trunk.
- **Appearance** — a derived, disposable triangle surface built by `tree_surface_triangles()`. Smooth, gently twisted tapered tubes are sampled along spine and branch chains, with transported frames and continuous bark UVs. Very thin twigs use intersecting bark ribbons; foliage still uses the existing species-specific cutout cards.

## Exact render ownership in eight convex planes

`src/hexel_clip.rs` describes each hexagonal prism by its six side halfspaces and top/bottom planes. It clips each bark triangle using convex polygon clipping and interpolates UVs for newly created vertices. Each surviving triangle is assigned to the real hexel that contains its entire visible surface; the operation emits **no independent gameplay branch pieces**.

`tree_surface_triangles()` is the *single* shared renderer/export surface builder, used by:

1. **Tree Designer** — direct `FoliageProject::generate()` preview and export;
2. **Flora prefab** — `FloraPrefab::generate_with_seed()` delegates to the same generator;
3. **Editable Assets** — `VoxelAsset::from_tree()`, `mesh()`, painting, cell removal, save/reload;
4. **wgpu viewport and OBJ** — both consume `FoliageMesh.triangles`, so neither can render branch geometry from a deleted cell.

The clipping operation is exact to a small floating-point tolerance **for the generated render triangles**. It is not yet an exact collision-volume/rasterizer for curved and tapered tubes: generated wood occupancy continues to use the conservative sampled brush plus face-connected bridge-cell routing. Some occupied cells (particularly thick wood interiors) may have no visible organic surface. Conversely, clipping may trim geometry that would otherwise extend beyond its allocated cells. This is deliberately safer than rendering unowned wood.

Clip ownership is independent of scene/world coordinates. A future world renderer still needs the documented local Z-up to world Y-up coordinate conversion, canonical periodic Q/R mapping and chunk visibility handling.

## Pipe-model / thickness

For woody plants, an optional postprocess counts **occupied foliage hexels**, not the number of decorative GPU cards, and aggregates support load from descendants toward the root. An area-like exponent (default **2.3**, UI 1.5–3) determines target relative child radii. A user-facing blend (default **0.35**, UI 0–1) mixes the result with each species' authored branch radius. A bounded radius change of 0.65–1.35× preserves distinctive forms.

The authored trunk radius and taper stay unchanged; palms and bamboo skip this model to preserve their single-stem/culm anatomy. Any new wood swept by a thicker branch is claimed in the occupied-cell map before rendering. The calculation is a **game-art approximation**, not a calibrated wood hydraulics, carbon allocation or fatigue simulation.

## Boundaries and next improvements

- Fork branches still overlap/intersect their parent tubes. This is a deliberate first implementation: an MTree-style welded ring junction is a separate meshing algorithm, and must maintain hexel ownership. We do not claim every fork is mathematically manifold.
- Removing a cell removes all fragments assigned to it; the renderer does **not yet add realistic end-grain cut scars/caps** at the newly exposed cell boundary. No runtime detached-limb physics, resource drops, regrowth or persistent world-chunk effects are implied.
- A branch passing between two cells is face-connected in the logical wood grid; its visual tube is clipped at physical boundaries. The existing bridge-cell fallback remains clipped so a diagonal does not create phantom wood.
- The editor still uses bounded 40,000-cell / 96-layer foliage limits. For long-distance worlds, build/cache the derived surface per tree/dirty region, never run the clipping pass per frame or per camera movement. Dynamic LOD is not implemented.
- All procedural graph materials, seasonal palettes, wind, fruit-color accents, Flora habitat profiles and existing project JSON/asset exports remain separate from wood clipping.

## Implementation references (technique, not copied code)

- MTree: `PipeRadiusFunction.cpp` and `ManifoldMesher.cpp` ([source](https://github.com/MaximeHerpin/modular_tree/tree/master/m_tree/source)). Open-source C++ reference; our Rust code is independent.
- TreeForge: `analysis/pipe_model.py`, `geometry/mesher.py`, `generators/palm.py` ([source](https://github.com/Kapilthakur1520/treeforge)). Demonstrates bottom-up radius, transported tube rings and palm-specific anatomy.
- [Red Blob Games axial hex grids](https://www.redblobgames.com/grids/hexagons/) for hex geometry; regular six-plane + vertical-prism clipping is implemented directly for our chosen metric.

## Validation

`cargo test --all-targets` tests strict inside-hexel wood triangles for Oak, Pine, Cottonwood, Coffee, Coconut Palm and Bamboo; authored branch deletions and painted wood-to-leaf conversions; loaded Asset JSON roundtrips; old trunk-block rendering; valid 43-species structures; and bounded pipe-model on/off behavior. Existing material/season UI and GPU/WGSL tests remain in the same CI pipeline.

