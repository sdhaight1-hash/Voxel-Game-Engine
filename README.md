# Texture Studio

A standalone **Rust desktop procedural texture editor** with a node-based workflow. The first release is a deliberately small, editable 2D texture generator — not a Blender plug-in or a web app.

> **Status:** Early prototype (v0.1). Check GitHub Actions build results before relying on the binaries.

## Features

- Node canvas with movable nodes and clickable input/output sockets.
- Tileable multi-octave **Noise**, **Checker**, **Color**, **Invert**, **Blend**, and **Output** nodes.
- Live 256 × 256 preview; export PNG at 256, 512, 1024, or 2048 pixels.
- Save and reload JSON project graphs.
- Local, offline CPU renderer; cycle-safe graph connections.

## Running the app

Install the latest stable [Rust](https://rustup.rs/), then run:

```bash
cargo run --release
```

For a compiled executable, run `cargo build --release`. The output is at `target/release/texture-studio` (or `texture-studio.exe` on Windows). In **Actions → Desktop builds → Run workflow**, you can build downloadable Windows, macOS, and Linux binaries. These are unsigned build artifacts, not installers.

## Node workflow

1. Add a generator or filter using the left palette.
2. Click a blue socket on the source node's right edge, then a yellow socket on the destination node's left edge.
3. Drag node headers to move nodes. Right-click an input to disconnect it.
4. Select a node to edit its settings in the right panel.
5. Connect the final chain to **Output**, then select a resolution and click **Export PNG**.

Example: `Noise → Blend (A)`, `Color → Blend (B)`, `Blend → Output`. Each input takes one cable. Missing inputs produce black.

## Source layout

- `src/lib.rs`: graph model, validation, deterministic tileable texture renderer, unit tests.
- `src/main.rs`: egui native desktop application, node canvas, preview, and project/image dialogs.

## Roadmap

Next: zoom/pan, undo/redo, node search, more generators/filters, multi-map PBR materials, GPU acceleration, 3D material preview, installers and release signing.

## License

No open-source license has been selected yet. The repository owner should choose one before taking external contributions or distributing modified versions.
