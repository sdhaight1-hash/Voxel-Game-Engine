# Voxel Game Engine — Procedural Voxel Asset Editor

**World-design source of truth:** [Procedural World — Design Decisions v0.2](docs/procedural-world-design-decisions.md). Companion docs: [current synthetic world-generation direction](docs/world-generation-foundation.md) and [archived Tellus/real-Earth research](docs/real-earth-world-generation.md). **The world/gameplay architecture is proposed, not implemented in this repository.**

An offline, standalone **Rust desktop application** for voxel asset creation, procedural tree modeling, and editable PBR material maps. Material Maker inspired, but independently implemented — no Material Maker code is copied and Blender/Godot are not required.

> **Status: working procedural asset/material editor, planned world engine.** Node-graph materials are computed on CPU; the **Assets** and **Foliage** mesh viewports use the native wgpu renderer described below. The material-preview sphere remains an approximate lit preview, not a full physically based renderer. **No tectonic planet, Terrain Diffusion world, finite hydrology, reactive world runtime or multiplayer game is implemented yet.**

## Planned playable world (design only)

Our current future-world architecture is a **finite, flat-coordinate torus with both horizontal axes wrapping**, and an intended **1,024 × 1,024 km** geographic footprint. The terrain foundation is **synthetic periodic tectonics → Terrain Diffusion at world creation → permanently stored/compressed geography**, not real-Earth Tellus terrain or gameplay-time neural inference. It pre-generates **global 2D surface and environmental fields**, *not* the entire 3D voxel volume.

- **World vertical presets:** 256, **512 default**, 768 and 1,024 block levels; a Vintage Story-style proportional sea level is currently provisional (Y220 at 512).
- **Playable cell preference:** **hexagonal-prism voxels**, with axial Q/R and vertical Y coordinates; exact cell size, chunk dimensions, building ergonomics and periodic lattice mapping remain to be validated. The existing editor still supports **both cube and hexagonal-prism assets**.
- **Terrain runtime proposal:** stream and deterministically regenerate local hexel detail from immutable geography; persist player edits, geology/resource depletion and environmental state independently.
- **Water invariant:** all world water is **finite and conserved** across rivers, lakes, oceans, groundwater, ice, atmospheric moisture, and player storage. Use global/regional reservoirs plus local water physics, with accounted transfers at loading/unloading boundaries.
- **Reactive state proposal:** SpaceTimeDB-inspired authoritative state, controlled/transaction-like mutations, selective region/field invalidations and dynamic spatial interests. **SpaceTimeDB itself is not required**.
- **Provisional performance/storage plan:** ~31.25 m geographic elevation samples, 16-bit tiled heightmap, Zstd candidate compression and bounded runtime caches. Benchmarks, model conditioning, horizontal metre-per-hexel scale, and final fluid solver are open.

**Important:** These are design decisions and candidates, **not currently shipping features**. See [the full decision register](docs/procedural-world-design-decisions.md) for agreed versus provisional choices, tests and unresolved issues. Existing material and foliage export workflows below remain unchanged.

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

## wgpu / Vulkan GPU Rendering

**The 3D editor is now powered by wgpu** (the existing Rust/egui desktop UI also runs on the same wgpu device). The app's native adapter selector **prefers Vulkan** when a compatible Vulkan adapter is present, then chooses Metal (macOS), Direct3D 12 (Windows), or an available GL backend as a fallback. This is an adapter preference, **not** a requirement to install Vulkan on macOS or a guarantee that every GPU supports it. The active backend and GPU adapter are shown in the editor status.

The **Assets** and **Foliage** 3D viewports use a shared native WGSL rendering pipeline in `src/gpu_renderer.rs` + `src/voxel_gpu.wgsl`:

- Voxel/cutout mesh triangles are uploaded to GPU vertex buffers. They are re-uploaded only when geometry or baked material content changes, not when orbiting the model.
- GPU vertex shaders project the 3D camera and animate foliage tips. The underlying editable voxel coordinates and static OBJ exports stay unchanged.
- A **depth-tested offscreen render target** provides actual 3D occlusion, replacing painter's-order triangle rasterization. The egui interface displays the target texture without a second graphics context.
- Fragment shaders sample the **existing procedural bark and foliage textures**, discard transparent leaf-cutout pixels and shade visible faces.
- Asset selection still uses CPU ray/screen-face intersection on clicks, but the GPU handles the per-frame rasterization. The material node graph, preset construction, voxel growth, file formats and OBJ exports remain deterministic CPU code.

For now, the render target is 1024 × 1024 pixels and uses a simple orthographic camera and directional face lighting. The node graph is **not yet a GPU compute graph**, and there is no PBR-lighting model, shadow map, compute meshing or skeletal wind engine. The underlying wgpu renderer is the foundation for those later stages.

## Procedural Bark and Foliage Materials

The **Foliage** and **Assets** workspaces now use real material roles instead of treating every block or leaf card as a flat RGB color. Each project retains independent editable **Bark**, **Leaf Albedo** and optional **Leaf Cutout** node graphs. Existing asset/project JSON remains readable; missing material assignments use palette colors and the original generated leaf cutout.

Every new procedural Oak, Pine, Birch, Willow, Maple or Shrub tree automatically assigns the appropriate **existing natural-material presets** for its bark and leaf appearance. Use **Procedural Material Slots** in either inspector to search and assign additional entries from the 1,039-preset library. Select a slot, click **Edit slot in Nodes**, edit its graph, then click **Apply ... to Tree/Asset** in the top toolbar. The optional cutout graph removes extra pixels from the existing silhouette; it cannot fill air gaps. **Cutout threshold** controls this additional mask.

The same baked images drive the **3D editor preview** and the **static OBJ/MTL export**:

- `bark-albedo.png` from the assigned bark graph (when present)
- `foliage-leaves.png` with procedurally colored RGB and true cutout alpha
- `foliage-leaves-alpha.png` grayscale opacity mask for MTL `map_d`
- `bark-roughness.png`, `bark-normal.png`, `foliage-leaves-roughness.png`, `foliage-leaves-normal.png` when their material graphs exist

Bark meshes carry UV coordinates; foliage retains crossed, rotated, two-sided UV cards with wind swaying in the preview. The exporter always uses the static, unswayed geometry. Normal and roughness PNGs are exported for engines supporting PBR workflows, but simple OBJ/MTL importers generally need manual assignment for those extra channels.

These are **original procedural approximations** built from the app's material presets, not proprietary game art or source-game textures. Per-leaf biological geometry, real-time GPU wind, translucency/SSS and advanced texture painting remain future work.

## Voxel Asset Workspace (Blockbench-inspired foundation)

Voxel Game Engine is organized around **four linked workspaces**. **Assets** is the new primary editing workspace; **Foliage** supplies generated voxel-tree geometry; **Presets** and **Nodes** remain responsible for procedural surface materials. The design takes inspiration from Blockbench's viewport/outliner/modeling/texturing workflow while keeping our own native Rust code and cube/hex voxel representation.

**Foliage mesh setting:** In the **Assets** inspector you can choose cutout crossed cards or classic solid leaf blocks, and adjust preview wind sway. Legacy asset JSON defaults to cutout cards when its new setting is absent.

**Asset editing:** Open **Library → Assets** to create, save, load or export a standalone voxel model. The left panel is a scrollable voxel outliner with per-material counts and selectable occupied cells. The center panel shows a rotatable 3D viewport. The right inspector includes:

- **Select**: click a visible voxel face or choose a cell in the outliner to edit its integer X/Y/Z position
- **Add**: click a face to place an adjacent cube or hexagonal-prism voxel, or enter an exact coordinate
- **Erase**: click a visible occupied voxel face to remove it
- **Paint**: choose Bark, Dark Leaf, Leaf or Light Leaf and click a voxel to change its material slot
- **Undo/Redo**: 20-step asset-edit history for add, erase, move, recolor and voxel-size changes
- **Palette**: customize bark and foliage colors; the viewport shows these directly
- **Voxel size**: uniformly rescale the grid geometry while retaining integer cell coordinates

**Convert procedural growth to editable modeling:** Build a tree under **Foliage**, then click **Edit Tree as Asset**, or use **Import Procedural Tree** from the asset outliner. The tree's occupied cells become a separate editable model, preserving its cube/hex grid and material colors. Manual changes are not overwritten when its originating tree parameters change. You can also **Open Asset** on a previously exported `foliage.voxels.json` from the older tree workflow.

**Asset files:** **Save Asset** stores `asset.voxelasset.json`, including name, cube/hex grid, cell size, each occupied voxel and material slot, palette, leaf-card/wind settings and an optional embedded procedural node graph. **Export Asset** also writes `asset.obj` (UV-mapped, exposed bark faces and two-sided cutout foliage cards), `asset.mtl` (bark/foliage colors and cutout maps), and—in cutout mode—`foliage-leaves.png` and `foliage-leaves-alpha.png`. In the Assets inspector, **Capture current material nodes** attaches the active procedural graph; **Edit attached material in Nodes** loads it back to the node editor. Changes in Nodes must be captured again to update the saved asset.

**Current boundaries:** This is a first usable voxel asset editor, **not full Blockbench feature parity**. It does not yet have per-pixel UV painting, automatic multi-part UV atlas baking, arbitrary hierarchies or named groups, rigging/animation timelines, skeletal binding, mesh sculpting, plug-ins, Blockbench project-file compatibility, or direct 3D surface painting from the procedural PBR graph. The viewport now samples baked bark and foliage albedo/alpha using their UVs, and the asset stores per-role material graphs. Per-pixel painting, PBR-lit shading and bone animations remain future work.

## Native Voxel Tree Designer

Use **Library → Foliage** to generate **grid-based trees with distinct geometry for bark and leaves**—a saved set of occupied bark and leaf cells; bark is a block mesh, while a leaf cell can render custom crossed polygon cards. This implements the block-world interpretation of the workflow shown in the supplied Unreal Engine 5.7 tutorial, while remaining native Rust.

**Plant starters:** Oak, Pine, Birch, Willow, Maple, Shrub. They use seeded procedural trunks, branching and volumetric foliage clusters. Settings are editable and reproducible.

**Cube or hexagonal-prism voxels:** In the **Voxel Builder** stage, select the block shape and **Voxel cell size**. Cube trees use integer Cartesian x/y/z coordinates. Hex-prism trees use an integer axial q/r hex grid plus vertical z levels. Bark occupies the trunk and voxelized branch paths; leaf voxels fill jittered ellipsoid clusters around branch tips. Wood always takes priority over leaves.

The ten editable stages are Tree Preset (species/seed/branches), Carve (top/bottom), Gravity / Phototropism (branch growth direction), Slope (trunk lean), Scale, Remove Branches, Voxel Builder (cube/hex and cell size), Block Palette (bark/leaf color), Leaf Voxel Distributor (volumetric foliage, density and variation), and Voxel Export.

**Preview:** The central viewport orbits the tree. Bark displays as cube or hex-prism exterior surfaces, with internal bark faces culled. Leaf cells default to **two intersecting, double-sided, angled foliage planes**, with a generated binary-alpha leafy sprig texture, true gaps, and deterministic 90° cell rotation. Opaque bark fully surrounding a leaf cell causes that card mesh to be omitted; neighboring transparent leaves do not count as opaque. **Block Palette → Leaf cell geometry** can switch back to solid leaf voxels. Very dense trees use reduced preview detail while **exports always include every occupied cell**. **Wind sway** animates leaf-card vertices in the CPU-based preview while keeping their bases anchored; it never moves the occupied voxel grid. This is a preview approximation, *not GPU vertex-shader displacement*.

**Exports:** **Export Voxel Tree** writes four files into an empty folder:
- `foliage.voxels.json`: authoritative voxel asset: grid type, cell size, plant settings and an explicit list of occupied (x, y, z, bark/leaf-material) cells. Hex (x,y) represent axial (q,r).
- `foliage.obj`: exposed bark surfaces **plus double-sided, UV-mapped cutout leaf-card meshes** (or solid leaf faces when selected).
- `foliage.mtl`: bark/foliage material slots. In cutout mode its leaf materials reference the two PNGs listed below.
- `foliage.json`: editable generation parameters including leaf mesh style and wind controls. **Save Tree** and **Open Tree** use this format.
- `foliage-leaves.png`, `foliage-leaves-alpha.png` (cutout mode): an **original generated RGBA sprig texture** and a grayscale alpha mask. Both are referenced in MTL; texture importers vary in how they apply color tint and opacity.

**Limitations:** This is a voxel-tree asset generator with mesh-based leaf cards, not a full voxel terrain, forest scatterer, plant growth simulator, or GPU wind/physics solver. It does not import Unreal Megaplants, support Nanite/PCG/skeletal animation, or yet paint individual tree cells. Smaller voxel sizes cost more CPU/memory and settings have bounds to protect the native editor. The foliage editor remains separate from the 2D surface-material node graph.

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

- The complete procedural planetary world described in `docs/` is **not implemented**; these are architecture plans, not evidence of a terrain, hydrology, or gameplay runtime.

- Procedural render is CPU-bound and can take time at 2048×2048, particularly with many nodes.
- Sphere view is a fast approximate shading preview, not full ray tracing, custom mesh rendering, or accurate tangent-space lighting.
- Nodes are single-image outputs. Multi-output nodes, subgraphs, undo/redo, external image textures, and painting are not implemented.
- Generated normal maps use fixed-height finite differences rather than a configurable strength node.
- The implemented node library remains a subset of Material Maker's capabilities; it is not full feature parity. The expanded original node set is described above.

## Source

- `src/lib.rs`: deterministic tileable procedural graph renderer, input validation, PBR channels, and unit tests.
- `src/presets.rs` and `src/presets/catalog.json`: embedded 1,039-preset natural material catalog and procedural node-graph builder.
- `src/node_library.rs`: 76 executable original Rust generators, filters, math/mix operations and transforms with tests.
- `src/foliage.rs`: deterministic cube/hex-prism voxel tree generation and shared exposed-face geometry for editing/export.
- `src/asset.rs`: standalone editable voxel asset model, tree imports, block operations and asset serialization.
- `src/gpu_renderer.rs`, `src/voxel_gpu.wgsl`: wgpu GPU 3D rendering, depth-buffered voxel mesh projection, procedural texture sampling and leaf wind.
- `src/main.rs`: native egui interface, graph controls, sphere and node previews, project dialogs, and PNG output.
- `.github/workflows/ci.yml`: build/test validation on push.
- `.github/workflows/desktop-builds.yml`: manual downloadable desktop executables.

## License

No license has been selected. Choose one before accepting contributions or distributing derivative builds.
