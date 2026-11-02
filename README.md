# Voxel Game Engine — Procedural Voxel Asset Editor

**Engine design:** [Procedural World — Design Decisions v0.3](docs/procedural-world-design-decisions.md). See also [Asset-creation subsystems and game integration](docs/asset-creation-subsystems.md), [current synthetic world-generation direction](docs/world-generation-foundation.md) and [archived Tellus research](docs/real-earth-world-generation.md). **The planetary world/gameplay runtime is proposed, not implemented in this repository.**

An offline **Rust desktop asset-authoring application** comprising dedicated subsystems for **Nodes, Presets, Foliage and Assets**. These are **permanent, reusable asset-creation subsystems of the planned greater game engine**, currently presented in one desktop tool—not independent game engines or throwaway prototypes. They create procedural voxel objects, trees and editable material maps. Material Maker inspired, but independently implemented; Blender/Godot are not required.

> **Status: working procedural asset/material editor, planned world engine.** Node-graph materials are computed on CPU; the **Assets** and **Foliage** mesh viewports use the native wgpu renderer described below. The material-preview sphere remains an approximate lit preview, not a full physically based renderer. **No tectonic planet, Terrain Diffusion world, finite hydrology, reactive world runtime or multiplayer game is implemented yet.**

## Asset-creation subsystems in the greater engine

**Architectural direction:** Keep the existing four workspaces as **separate, specialized engine subsystems responsible for creating reusable content**. They remain useful and editable even after the playable world exists.

| Existing subsystem | Authors | Intended game-runtime use |
|---|---|---|
| **Nodes** | Procedural materials, tileable surface detail and texture channels | Bake and share visual materials across terrain and objects |
| **Presets** | Indexed natural-material/appearance recipes | Provide selectable materials; world geology and biomes decide their placement |
| **Foliage** | Deterministic cube/hexel trees and plants with bark, leaves and wind settings | Produce reusable vegetation definitions; ecology/biome systems choose placement and manage life state |
| **Assets** | Editable cube/hex-prism objects, models and structures | Publish prefabs/placeable geometry whose player modifications belong to world state |

The **shared authoring preview** (current wgpu-backed Assets/Foliage viewports) is an asset-inspection/rendering facility. It is *not* yet a chunk-streaming game renderer. The underlying geometry, texture baking, UV rules and rendering techniques can be reused without making the authoring UI responsible for gameplay.

**Proposed content handoff (not yet implemented):** editable **source** `Graph` / preset / `FoliageProject` / `VoxelAsset` → **validate, bake and publish** → **versioned, game-ready shared asset library** → world-generation placement, game renderer and authoritative runtime state.

- Shared materials should be baked/cached **once per recipe/variant**, rather than running CPU procedural graphs for each terrain hexel.
- Gameplay instances reference asset identifiers/versions; a harvested tree or edited structure keeps its **persistent world changes** even if its source is re-exported.
- The published game data will need stable asset IDs, material bindings, geometry or pinned procedural recipes, and a defined import/versioning contract; these are **design proposals**, not current export features.
- Existing source JSON, OBJ/MTL and PNG exports stay supported. **OBJ is an exchange mesh, not the authoritative editable hexel asset representation**.
- The present editor supports **both cubes and hexagonal prisms**; choosing hex-prism geometry for the playable world must not break cube authoring.

See [Asset-creation subsystems and game-runtime integration](docs/asset-creation-subsystems.md) for the current-code inventory, ownership boundaries and minimal integration tests.

## Planned playable world (design only)

Our current future-world architecture is a **finite, flat-coordinate torus with both horizontal axes wrapping**, and an intended **1,024 × 1,024 km** geographic footprint. The terrain foundation is **synthetic periodic tectonics → Terrain Diffusion at world creation → permanently stored/compressed geography**, not real-Earth Tellus terrain or gameplay-time neural inference. It pre-generates **global 2D surface and environmental fields**, *not* the entire 3D voxel volume.

- **World vertical presets:** 256, **512 default**, 768 and 1,024 block levels; a Vintage Story-style proportional sea level is currently provisional (Y220 at 512).
- **Playable cell preference:** **hexagonal-prism voxels**, with axial Q/R and vertical Y coordinates; exact cell size, chunk dimensions, building ergonomics and periodic lattice mapping remain to be validated. The existing editor still supports **both cube and hexagonal-prism assets**.
- **Terrain runtime proposal:** stream and deterministically regenerate local hexel detail from immutable geography; persist player edits, geology/resource depletion and environmental state independently.
- **Water invariant:** all world water is **finite and conserved** across rivers, lakes, oceans, groundwater, ice, atmospheric moisture, and player storage. Use global/regional reservoirs plus local water physics, with accounted transfers at loading/unloading boundaries.
- **Reactive state proposal:** SpaceTimeDB-inspired authoritative state, controlled/transaction-like mutations, selective region/field invalidations and dynamic spatial interests. **SpaceTimeDB itself is not required**.
- **Provisional performance/storage plan:** ~31.25 m geographic elevation samples, 16-bit tiled heightmap, Zstd candidate compression and bounded runtime caches. Benchmarks, model conditioning, horizontal metre-per-hexel scale, and final fluid solver are open.

**Important:** The playable world features are design decisions and candidates, **not currently shipping features**. The **asset-creation subsystems above are already implemented** in the desktop editor, while their game-ready publishing pipeline is still proposed. See [the full decision register](docs/procedural-world-design-decisions.md) for agreed versus provisional choices, tests and unresolved issues. Existing material and foliage export workflows below remain unchanged.

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

The **Foliage** and **Assets** workspaces now use real material roles instead of treating every block or leaf card as a flat RGB color. Each project retains independent editable **Bark**, **Leaf Albedo** and optional **Leaf Cutout** node graphs. Current asset/project JSON supports editable material assignments; missing material assignments use palette colors and the generated leaf cutout.

Every Foliage tree and bamboo preset selects its associated **existing natural-material recipes** for bark/wood and leaves, where the catalog provides them. Use **Procedural Material Slots** in either inspector to search and assign additional entries from the 1,039-preset library. Select a slot, click **Edit slot in Nodes**, edit its graph, then click **Apply ... to Tree/Asset** in the top toolbar. The optional cutout graph removes extra pixels from the existing silhouette; it cannot fill air gaps. **Cutout threshold** controls this additional mask.

The same baked images drive the **3D editor preview** and the **static OBJ/MTL export**:

- `bark-albedo.png` from the assigned bark graph (when present)
- `foliage-leaves.png` with procedurally colored RGB and true cutout alpha
- `foliage-leaves-alpha.png` grayscale opacity mask for MTL `map_d`
- `bark-roughness.png`, `bark-normal.png`, `foliage-leaves-roughness.png`, `foliage-leaves-normal.png` when their material graphs exist

Bark meshes carry UV coordinates; foliage retains crossed, rotated, two-sided UV cards with wind swaying in the preview. The exporter always uses the static, unswayed geometry. Normal and roughness PNGs are exported for engines supporting PBR workflows, but simple OBJ/MTL importers generally need manual assignment for those extra channels.

These are **original procedural approximations** built from the app's material presets, not proprietary game art or source-game textures. Per-leaf biological geometry, real-time GPU wind, translucency/SSS and advanced texture painting remain future work.

## Voxel Asset Workspace (Blockbench-inspired foundation)

Voxel Game Engine is organized around **four linked workspaces**. **Assets** is the new primary editing workspace; **Foliage** supplies generated voxel-tree geometry; **Presets** and **Nodes** remain responsible for procedural surface materials. The design takes inspiration from Blockbench's viewport/outliner/modeling/texturing workflow while keeping our own native Rust code and cube/hex voxel representation.

**Asset leaf mesh setting:** The general **Assets** inspector can choose cutout crossed cards or solid leaf blocks for unrelated authored objects. Generated trees from **Foliage** always use card-based leaves.

**Asset editing:** Open **Library → Assets** to create, save, load or export a standalone voxel model. The left panel is a scrollable voxel outliner with per-material counts and selectable occupied cells. The center panel shows a rotatable 3D viewport. The right inspector includes:

- **Select**: click a visible voxel face or choose a cell in the outliner to edit its integer X/Y/Z position
- **Add**: click a face to place an adjacent cube or hexagonal-prism voxel, or enter an exact coordinate
- **Erase**: click a visible occupied voxel face to remove it
- **Paint**: choose Bark, Dark Leaf, Leaf or Light Leaf and click a voxel to change its material slot
- **Undo/Redo**: 20-step asset-edit history for add, erase, move, recolor and voxel-size changes
- **Palette**: customize bark and foliage colors; the viewport shows these directly
- **Voxel size**: uniformly rescale the grid geometry while retaining integer cell coordinates

**Convert game foliage into editable objects:** Generate a tree under **Foliage**, then click **Edit Tree as Asset** (or **Import Procedural Tree** from the Asset outliner). The generated tree **already consists entirely of the game's standard 1 m across-flats / 1 m high hexagonal-prism cells**, with solid harvestable wood and logical leaf hexels displaying textured cutout cards. It opens as an independent editable `VoxelAsset` for individual cell removal, painting, saving, and export. Editing an imported tree never changes its source generation settings. There is **no separate cube/size foliage mode or secondary game conversion step**. The editor does not yet implement in-world felling or a durable shared asset library.

**Asset files:** **Save Asset** stores `asset.voxelasset.json`, including name, cube/hex grid, cell size, each occupied voxel and material slot, palette, leaf-card/wind settings and an optional embedded procedural node graph. **Export Asset** also writes `asset.obj` (UV-mapped, exposed bark faces and two-sided cutout foliage cards), `asset.mtl` (bark/foliage colors and cutout maps), and—in cutout mode—`foliage-leaves.png` and `foliage-leaves-alpha.png`. In the Assets inspector, **Capture current material nodes** attaches the active procedural graph; **Edit attached material in Nodes** loads it back to the node editor. Changes in Nodes must be captured again to update the saved asset.

**Current boundaries:** This is a first usable voxel asset editor, **not full Blockbench feature parity**. It does not yet have per-pixel UV painting, automatic multi-part UV atlas baking, arbitrary hierarchies or named groups, rigging/animation timelines, skeletal binding, mesh sculpting, plug-ins, Blockbench project-file compatibility, or direct 3D surface painting from the procedural PBR graph. The viewport now samples baked bark and foliage albedo/alpha using their UVs, and the asset stores per-role material graphs. Per-pixel painting, PBR-lit shading and bone animations remain future work.

## Native Voxel Tree Designer

Use **Library → Foliage** to generate **grid-based trees with harvestable wood and leaf cells**. Trunks are full 1 m wood hexels; branches use tapered six-sided wood meshes and thin twig ribbons while remaining bound to occupied wood hexels; leaves render crossed cutout cards. This implements the block-world interpretation of the workflow shown in the supplied Unreal Engine 5.7 tutorial, while remaining native Rust.

**Natural-world tree catalog:** The original 29 requested entries remain, plus ornamental **Cherry Blossom** and three new palms: **33 tree/bamboo entries (31 tree/fruit/ornamental varieties and 2 bamboo)**. `Shrub` stays separate (**34 total selectable presets**). The Tree Designer dropdown groups them by family.

| Group | Presets |
|---|---|
| Hardwood & broadleaf (11) | Acacia, Birch, Ebony, Kapok, Maple, Crimson King Maple, Oak, Purpleheart, Walnut, Willow, Cherry Blossom |
| Conifer & evergreen (5) | Bald Cypress, Mediterranean Cypress, Larch, Pine, Redwood |
| Fruit trees (12) | Red Apple, Pink Apple, Yellow Apple, Pear, Olive, Peach, Cherry, Mango, Orange, Lychee, Breadfruit, Pomegranate |
| Bamboo (2) | Green Bamboo, Brown Bamboo |
| Palms (3) | Coconut Palm, Date Palm, Fan Palm |
| Other foliage | Shrub (not one of the 33 tree/bamboo entries) |

**Ornamental Cherry Blossom (sakura):** A distinct, non-fruiting visual preset alongside the existing fruit-bearing Cherry. It has a spreading/vase-shaped crown and green summer leaves. In the **wgpu seasonal preview**, early Spring (season 0) draws procedural pink five-petal blossom clusters over the existing leaf cards. Blossoms fade by late Spring, without changes to seed geometry or voxel occupancy. Summer remains green, Autumn is bronze-red, and Winter uses a muted dormant color. The overlay applies to imported editable Assets that retain the Cherry Blossom leaf-species material. It is a preview shader effect, not separate harvestable flower hexels, particle shedding or a new fruit system. Conventional OBJ/PNG exports and the CPU preview remain static summer/base leaf artwork without the transient blossom overlay.

**Palm growth architecture:** Coconut, Date, and Fan Palm use a dedicated seeded generator, not the broadleaf branching algorithm. Their single nearly cylindrical trunk supports crown-only curved petioles. Coconut and Date Palms use distinct pinnate (feather) foliage masks; Fan Palm uses radial palmate leaf cards. The existing 1 m hexel occupancy, editable branch ownership (petioles are recorded as `MajorBranch` solely for structural editing), deterministic seed, GPU leaf-color interpolation, wind, save/load and OBJ/MTL exports still work. Frond count and reach appear in Tree Designer, and the normal woody-twig controls are hidden for palms. These presets **do not** yet create harvestable coconuts/dates, mature leaf scars or physically modeled individual leaflets; the palm leaves remain alpha-cutout cards.

**Game-scale structural presets:** Each entry has its own initial tree height, trunk radius, branch count, branch reach, canopy density, colors, and growth parameters. Dense broadleaf crowns now combine overlapping interior-filled lobes and an upper crown cap; Willow additionally distributes foliage along its descending shoots instead of relying exclusively on end clusters. Conifers receive intermediate needle groups along selected branches. Key shape families include broad round crowns, Acacia's high spreading umbrella, Kapok's tall trunk/upper crown, Pine/Larch/Redwood tiered conifers, a narrow Mediterranean Cypress, drooping Willow, small orchard fruit trees and upright bamboo culms. These are procedural **1 m hexel** shapes; they are not imported models or a botanical growth simulator. Species and seeds remain editable.

**Seasonal foliage colors (GPU preview):** Each tree/plant has a default species-tuned set of **four four-stop RGB gradients**, one for Spring, Summer, Autumn and Winter (shadow → base → midtone → highlight). The **Foliage Palette** stage and the Assets inspector let authors edit every stop, and the tree/asset JSON persists custom palettes. The **Season preview** slider covers 0=Spring, 1=Summer, 2=Autumn, 3=Winter and 4=Spring, with seamless transitions. `wgpu` uploads the 16 RGBA stops only when a material scene changes; WGSL interpolates stops and seasons per leaf pixel using card UVs, and one small uniform advances the visible season without remeshing or rebaking leaf textures. The authored leaf sprites keep their cutout alpha and texture detail. Seasonal coloring does **not** simulate leaf fall, leaf growth, weather or calendar time. The CPU fallback and conventional OBJ/PNG exports remain static (summer/base-tint appearance); JSON includes palettes for a future world renderer.

**Species and fruit-card materials:** Species retain their authored leaf-color tint even when a procedural leaf-material graph is assigned; the graph modulates light/dark detail instead of replacing green foliage with a bark-colored palette. Species use existing natural-material bark/leaf recipes where available and original, family-appropriate procedural leaf-card alpha silhouettes (lobed, oval, needle sprays, feathered, compound or narrow bamboo leaves). Red, Pink and Yellow Apple deliberately share an apple leaf shape but have separate ripe-fruit colors; other fruit tree presets also store fruit colors, represented by small **texture-only** accents. Physical fruit blocks, ripening, picking and item drops are **not** implemented. `FoliageMaterials` persists the chosen leaf species and fruit color through Asset conversion and texture baking.

**Game-standard hexels only:** All Foliage generation, preview and exports use **axial Q/R with integer vertical Z**, a 1 m across-flats footprint and a 1 m vertical layer. Trunk wood uses solid physical hexel geometry; narrower branches are visibly tapered inside their harvestable wood cells, and leaf hexels are logical removable cells represented by crossed alpha-cutout texture cards. Wood has priority over leaf occupancy. Diagonal branches are bridged with face-adjacent hexels. A full standard hexel is approximately 0.866 m³.

The ten editable stages cover species/growth, canopy carving, gravity/phototropism, trunk slope, overall dimensions, pruning, hexel wood construction, foliage colors/wind, leaf distribution and export. Foliage has **no cube or free-form cell-size options**; the separate Assets workspace retains general-purpose cube/hex modeling.

**Structural tree authoring:** Growth now creates a trunk first, then **parent-linked main limbs and smaller forks**, assigning each wood hexel a branch/trunk role. Leaf clusters retain the branch they attach to, and disconnected foliage islands are dropped. The generator records a local **tree origin**, the bottom-layer **root footprint**, branch attachment/tip cells and per-cell roles, and includes that metadata in exported tree JSON and editable Assets. Manual block add/paint/erase operations maintain per-cell metadata and grounding; the Assets inspector shows the structural role and branch ID of selected cells. This provides future groundwork for felling and placement clearance, **not** functioning world-space harvesting or forest scattering.

**Visual target:** Stylized, distinctive broadleaf, conifer, willow and bare-branch silhouettes, taking [MountainLabs' Voxel Tree Pack](https://mountainlabz.itch.io/voxel-tree-pack) as **visual inspiration only**. We generate original 1 m hexels and species-specific textured cutout cards; no external pack assets are included, and seasonal variations and direct world placement are future refinements.

**Preview:** The central viewport orbits the tree. Trunks and manually added wood render as real hex-prism exterior surfaces; generated major limbs render tapered six-sided sections and the thinnest twigs render crossed bark strips, all clipped by surviving wood-cell occupancy. Each occupied leaf hexel now produces **six independently positioned and oriented, double-sided leaf sprays** instead of two intersecting cards; each spray uses the same species-specific binary-alpha texture. All six disappear together when that hexel is erased. Opaque bark fully surrounding a leaf cell causes that card mesh to be omitted; neighboring transparent leaves do not count as opaque. The Foliage creator always uses texture-card leaves; only the generic Assets modeling tools can choose other leaf mesh styles. Very dense trees use reduced preview detail while **exports always include every occupied cell**. **Wind sway** animates leaf-card vertices in the CPU-based preview while keeping their bases anchored; it never moves the occupied voxel grid. This is a preview approximation, *not GPU vertex-shader displacement*.

**Exports:** **Export Voxel Tree** writes a world-grid hexel tree and associated files into an empty folder:
- `foliage.voxels.json`: **`hexel-game-tree-v1`** JSON containing explicit occupied Q/R/Z hexel cells, fixed across-flats/height dimensions, wood/leaf materials and the source tree settings. It can be reopened as an editable tree in **Assets**. Hex (x,y) represent axial (q,r).
- `foliage.obj`: full-hexel trunk surfaces, tapered branch/twig mesh sections and **double-sided UV-mapped cutout leaf-card meshes**. Every generated wood section retains its owning hexel.
- `foliage.mtl`: bark/foliage material slots. In cutout mode its leaf materials reference the two PNGs listed below.
- `foliage.json`: editable generation parameters including leaf mesh style and wind controls. **Save Tree** and **Open Tree** use the current foliage project format (the former arbitrary foliage grid settings are not supported).
- `foliage-leaves.png`, `foliage-leaves-alpha.png` (cutout mode): an **original generated RGBA sprig texture** and a grayscale alpha mask. Both are referenced in MTL; texture importers vary in how they apply color tint and opacity.

**Limitations:** This is a voxel-tree asset generator with mesh-based leaf cards and per-cell harvest/removal primitives, not a full voxel terrain, forest scatterer, playable harvesting/felling mechanic, plant growth simulator, or gameplay physics solver. It does not import Unreal Megaplants, support Nanite/PCG/skeletal animation, or yet paint individual tree cells. Foliage cell size is fixed at the world-standard 1 m; large canopies and excessive branching still have bounded generation budgets. The foliage editor remains separate from the 2D surface-material node graph.

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
