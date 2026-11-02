# Texture Studio — Rust Procedural Material Editor

An offline, standalone **Rust desktop application** for building procedural textures and PBR material maps in a node graph. Material Maker inspired, but independently implemented — no Material Maker code is copied and Blender/Godot are not required.

> **Status: early prototype.** This release emphasizes a working node graph and CPU rendering. The 3D tab shows an approximate lit sphere, not a full physically based GPU renderer.

## Screenshot-inspired workspace

- **Library (left):** filter nodes by name; add Noise, Checker, Bricks, Voronoi, Color, Invert, Colorize, and Blend.
- **Graph (center):** draggable nodes and thumbnails, colored cables, zoom (mouse wheel) and pan (middle-drag).
- **Preview (right):** lit 3D sphere simulation, selectable 2D output channels, and selected-node preview.
- **Inspector:** edit generator, color, and filter parameters in real time.
- **Material Output:** connect Albedo, Roughness, Metallic, and Height. An OpenGL-style tangent-space Normal map is derived from Height at export. Unconnected roughness/metallic/height use neutral defaults.
- **Export:** single PNG or five-map PBR set (albedo, roughness, metallic, height, normal). Export sizes: 256–2048 square pixels.
- **Native voxel modes:** use a true six-face cube or eight-face hexagonal prism (six walls, top and bottom), with an orbitable textured 3D preview.
- **Voxel UV mapping:** wrap the texture continuously around side walls or repeat it per face; caps use planar UVs and hex-cap pixels outside the shape are transparent.
- **Voxel export:** generate five per-face RGBA texture atlases, a UV-unwrapped `voxel.obj`, `voxel.mtl` and machine-readable `voxel_layout.json`. The OBJ material connects albedo; other PBR maps can be assigned in the target renderer.
- **Projects:** save/load JSON node graphs *including voxel shape and side UV mode*; older projects without voxel settings load as cubes.

## Native Procedural Foliage Designer

The app now includes a separate **Library → Foliage** workspace for modeling actual 3D vegetation, inspired by the **Unreal Engine 5.7 Procedural Vegetation Editor** tutorial: https://www.youtube.com/watch?v=o-kMXZX_oK8. This is an independently implemented Rust tree-mesh generator, not a copy of Epic's plugin, presets or proprietary Megaplants assets.

**Included plant starters:** Oak, Pine, Birch, Willow, Maple and Shrub. Every preset is a starting point with a reproducible random seed and editable controls.

Select one of ten **growth stages** in the Foliage panel, then edit its settings in the inspector:

| Stage | Native behavior |
|---|---|
| Preset Loader | Species, seed, tree height, branching density and length |
| Carve | Remove lower/upper branch-growing zones |
| Gravity / Phototropism | Bend branches downward or steer them upward toward light |
| Slope | Lean the trunk for hillsides / windswept forms |
| Scale | Scale the complete tree without changing the underlying growth seed |
| Remove Branches | Seeded random pruning |
| Mesh Builder | Tapered polygon trunk, limbs and twig geometry; adjustable 3–12 radial segments |
| Foliage Palette | Bark color and three leaf-tone material slots |
| Foliage Distributor | Leaf density, scale, variation and orientation; actual triangulated leaf geometry |
| Output | Save foliage settings or export a standalone polygon mesh |

The central viewport shows a rotatable, shaded **3D triangle-mesh preview**, rather than a 2D foliage noise texture. The foliage editor works alongside the previous material graph. **Save Tree** writes a standalone editable `tree.foliage.json`; **Open Tree** reloads it; **Export Tree OBJ** writes `foliage.obj`, `foliage.mtl` and `foliage.json`. The MTL colors are bark, leaf, darker leaf, and lighter leaf. These are solid polygon leaves and branches; the exporter does not depend on missing image assets.

**Important differences from Unreal:** The stages are a fixed procedural editing sequence, not a freely rewritable Unreal point-cloud graph. Output is a static triangulated OBJ with MTL; no skeletal bones, live physics wind, Nanite, PCG forest population, terrain-aware plant placement, foliage instancing, photoreal leaf textures, or Fab/Megaplant asset ingestion. Those require a bigger geometry/scene architecture.

## Expanded Material Maker-inspired Node Library

The native **Nodes** tab now includes **76 additional, working 2D nodes** in seven searchable groups:

| Group | Added nodes | Examples |
|---|---:|---|
| Shapes & Gradients | 6 | Linear/Radial Gradient, Circle, Rounded Rectangle, Polygon, Sine Wave |
| Noise | 7 | White Noise, Color Noise, Value Noise, Fractal Noise, Clouds, Crystal, Dirt |
| Patterns | 8 | Beehive, Weave, Diagonal Weave, Truchet, Fibers, Scratches, Spots, Triangles |
| Color & Tones | 20 | Brightness/Contrast, Gamma, Threshold, Posterize, Hue Shift, Saturation, Smoothstep, Quantize |
| Filters | 10 | Edge Detect, Blur, Sharpen, Emboss, Dilate, Erode, Pixelize, Normal Map, Directional Blur |
| Math & Mix | 14 | Add, Subtract, Multiply, Divide, Min/Max, Difference, Screen, Overlay, Soft Light, Hard Light, Mask, Lerp, Switch |
| Transforms | 11 | Translate, Rotate, Scale, Mirror, Kaleidoscope, Warp, Swirl, Shear, Tile, Wave Distort, Spherize |

The prior core nodes (Noise, Checker, Color, Bricks, Voronoi, Strata, Invert, Colorize, Blend, Material Output) remain available. In the Nodes tab, search by name, scroll the node library, and click **+** to add a node. Drag its ports to connect it; change **Amount / scale** and **Seed** in the node inspector. Most generators have no inputs, filters/transforms take one image, and math/mix nodes take two.

Every operation is implemented directly in Rust, runs locally, can be saved in project JSON, and works with cube/hexagonal-prism atlas export. They are **similar node categories with independently written CPU implementations**, *not exact Material Maker shader replicas*. Parameters and visual behavior may differ.

**Not yet implemented:** the entire 200+ / ~250-node Material Maker library, 3D SDF and mesh/projection nodes, custom GLSL shaders, arbitrary multi-output nodes, group/subgraph editing, GPU buffer and iterative nodes, image/text/glyph loading, texture painting, GPU shader live preview, or material imports from Material Maker files. Full parity requires a GPU graph architecture and broader project schema. The added operations intentionally retain the current one-image-per-node graph model.

## Geometryless 2D material mode

Select **Material Geometry → Geometryless (2D)** in the right panel to work entirely in texture space. This is a true mesh-free workflow, not a flat cube preview.

- **2D tab:** view Albedo, Roughness, Metallic, Height, or derived Normal.
- **Node tab:** inspect any intermediate image, including procedural generators.
- **Export image:** save one square channel as PNG.
- **Export PBR maps:** save five independent square PNGs. No mesh, OBJ or face atlas is generated.
- **Switch back:** choose **Cube** or **Hexagonal prism** to enable the textured voxel preview and **Export voxel OBJ + atlases** again.
- The geometryless switch is stored in project JSON, along with the last selected voxel shape/UV mapping; old projects default to Cube.

## Updated natural-material preset graphs

All **1,039 built-in natural texture presets** now use the expanded node library. Each selection assembles an editable, validated network using material-family-specific primary patterns (Fractal Noise, Dirt, Crystal, Spots, Fibers, Clouds or Sine Wave, or Strata for layered wood/shell textures), a family-appropriate filter, secondary micro-detail, Colorize, an adjustable Lerp blend, an editable roughness network, and a metallic channel. Unlike the previous simple five-node templates, these graphs expose the new generators and filters for further editing. Presets work identically in geometryless, Cube, and Hexagonal Prism modes.

These remain **procedural approximations**, not exact scientific or source-game material scans, species-accurate foliage meshes, or copies of proprietary textures.

## Built-in Natural Preset Library

This build includes **1,039 named material/appearance presets across 14 categories**, derived from the provided *Complete Natural Texture Catalog*. The catalog is embedded in the executable at `src/presets/catalog.json` — no network connection or texture downloads needed.

- **Stone & Rock:** igneous, sedimentary and metamorphic materials, multiple rock surface appearances, marble colors.
- **Ores & Minerals:** metal-bearing ores, salts, crystals, gemstones, coal and additional reference minerals.
- **Soil & Ground; Sand, Gravel & Clay:** fertility/grass appearances, all 14 sand and gravel parents, clay deposits, natural loose terrain.
- **Trees & Wood; Fruit Trees:** bark, branches, leaf and seasonal appearances, including custom Willow and all its named components.
- **Berry Bushes; Flowers; Vegetation:** berry stages, colored flower varieties, ferns, grass sizes, reeds, cacti, vines, aquatic vegetation and croton color/size variants.
- **Mushrooms; Wild Crops:** 45 mushroom species and growth stages of wild plants.
- **Water, Lava & Ice; Natural Formations; Seashells:** surface states, organic/mineral formations and all 42 shell combinations.

**Using a preset:** On the left choose **Presets**, filter by category or search any material/variant name, select a result, then click **Apply selected preset**. This replaces the current node graph with an editable layered generator/filter/detail/color/roughness/metallic network. It **preserves your cube/hexagonal-prism shape and side UV settings**. Save your project before replacing the graph. Switch to **Nodes** to customize the generators.

**Accuracy limitation:** These are original, algorithmic **tileable surface material approximations**, not imported Vintage Story PNGs, photographs, plant models, or species-accurate foliage/mushroom/flower illustrations. Non-surface visual states such as blossoms, mushrooms, leaf clusters, snow accumulation and crop stages are color/pattern presets only; they do not generate geometry, transparency cutouts, animations, physics or seasonal changes. The catalog's wording is descriptive reference data, not a claim that every entry is an ordinary in-game natural deposit.

The catalog is easy to extend: edit `src/presets/catalog.json` (groups with category, family, names, variants and profile). The Rust loader validates duplicate names/variants per family and builds the variants deterministically. `Strata` is a new tileable generator for wood/water/shell-style layered patterns.

## Build and run

1. Install the current stable [Rust toolchain](https://rustup.rs/).
2. Clone this repository.
3. From its root directory:

```bash
cargo run --release
```

Or compile without starting:

```bash
cargo build --release
cargo test --all-targets
```

The executable lives at `target/release/texture-studio` (Windows: `texture-studio.exe`).

GitHub **Actions → Desktop builds → Run workflow** produces downloadable **unsigned executables** for Windows, macOS and Linux. These are not installers or signed releases.

## First material in 60 seconds

1. Start the app; it opens an editable **bricks** example.
2. Select Bricks and adjust row/column counts and mortar.
3. Select Colorize and change the two colors.
4. Use the 3D / 2D / Node preview tabs to examine the material.
5. In the **Voxel Geometry** section, select **Cube** or **Hexagonal prism**, then select either **Wrap around perimeter** or **Repeat per face**.
6. Use **Voxel 3D** to inspect the shape, **Atlas** to inspect the face layout and **Orbit** to rotate the preview.
7. Choose an export size and click **Export voxel OBJ + atlases**. Choose an empty destination folder.
8. Save your editable project with **Save**.

**Connecting and moving cables**

- **New connection:** left-click and drag a blue output socket onto a yellow input socket, then release.
- **Rewire a cable:** drag the *connected input socket* or grab the cable itself; drop on a different input to move its destination, or drop on a different output to change its source.
- **Click mode:** click an output socket, then click the target input socket, if you prefer not to drag.
- **Remove:** right-click an input socket or the wire between nodes.
- **Cancel:** press Escape, or release on empty canvas during a drag; existing cables remain intact after invalid drops.
- **Move nodes:** drag their top title bar. Middle mouse pans; mouse wheel zooms.

A destination input supports only one incoming cable. Reconnecting replaces the prior cable at that input. Cyclic connections are rejected without deleting the original link.

## Native voxel atlas convention

A *voxel* is a block occupying a spatial cell. Hexagonal cells are modeled as **extruded hexagonal prisms**, not flat 2D hexagons.

| Geometry | Side walls | Caps | Atlas tiles | Atlas resolution |
|---|---:|---:|---|---|
| Cube | 4 | 2 | 3 × 2 | `3T × 2T` |
| Hexagonal prism | 6 | 2 | 4 × 2 | `4T × 2T` |

`T` is the chosen **pixels per face** (256, 512, 1024, or 2048). Face tiles are ordered in rows: `side_0...` followed by `top` and `bottom`. `voxel_layout.json` includes each face's normalized UV rectangle, row, column, shape, and repeat/wrap mode. Normals are tangent-space and based on height.

The `voxel.obj` file already contains UVs corresponding to `voxel_albedo.png`. Keep the exported files together. This is an **individual voxel mesh/material**, not a voxel terrain generator, chunk mesher, or hex-grid placement system. You can instance the mesh in your target application.

Side mapping affects all exported channels and the preview:
- **Wrap:** texture coordinate U advances continuously around the full perimeter (one repeat per voxel circumference).
- **Repeat:** the entire tile is repeated once on each face (best for repeated seamless patterns).

The OBJ/MTL format directly links the albedo map. Roughness, metallic, height, and normal maps are exported as extra atlas images, but usually require manual assignment in a game engine or material renderer.

## Current limitations

- Procedural render is CPU-bound and can take time at 2048×2048, particularly with many nodes.
- Sphere view is a fast approximate shading preview, not full ray tracing, custom mesh rendering, or accurate tangent-space lighting.
- Nodes are single-image outputs. Multi-output nodes, subgraphs, undo/redo, external image textures, and painting are not implemented.
- Generated normal maps use fixed-height finite differences rather than a configurable strength node.
- Only a small initial set of nodes is provided; Material Maker offers a substantially broader library.

## Source

- `src/lib.rs`: deterministic tileable procedural graph renderer, input validation, PBR channels, and unit tests.
- `src/presets.rs` and `src/presets/catalog.json`: embedded 1,039-preset natural material catalog and procedural node-graph builder.
- `src/node_library.rs`: 76 executable original Rust generators, filters, math/mix operations and transforms with tests.
- `src/foliage.rs`: deterministic 3D plant skeleton, branch and leaf generation, OBJ/MTL export, and tests.
- `src/main.rs`: native egui interface, graph controls, sphere and node previews, project dialogs, and PNG output.
- `.github/workflows/ci.yml`: build/test validation on push.
- `.github/workflows/desktop-builds.yml`: manual downloadable desktop executables.

## License

No license has been selected. Choose one before accepting contributions or distributing derivative builds.
