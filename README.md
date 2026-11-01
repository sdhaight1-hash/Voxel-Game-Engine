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
- **Projects:** save/load JSON node graphs; older projects with a single Output input remain readable.

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
5. From the top bar choose an export resolution and **Export PBR maps**; select a destination folder.
6. Save your editable project with **Save**.

To connect nodes, click the blue right-side output socket and then a left-side input socket. Right-click an input to disconnect its cable. Cyclic connections are rejected.

## Current limitations

- Procedural render is CPU-bound and can take time at 2048×2048, particularly with many nodes.
- Sphere view is a fast approximate shading preview, not full ray tracing, custom mesh rendering, or accurate tangent-space lighting.
- Nodes are single-image outputs. Multi-output nodes, subgraphs, undo/redo, external image textures, and painting are not implemented.
- Generated normal maps use fixed-height finite differences rather than a configurable strength node.
- Only a small initial set of nodes is provided; Material Maker offers a substantially broader library.

## Source

- `src/lib.rs`: deterministic tileable procedural graph renderer, input validation, PBR channels, and unit tests.
- `src/main.rs`: native egui interface, graph controls, sphere and node previews, project dialogs, and PNG output.
- `.github/workflows/ci.yml`: build/test validation on push.
- `.github/workflows/desktop-builds.yml`: manual downloadable desktop executables.

## License

No license has been selected. Choose one before accepting contributions or distributing derivative builds.
