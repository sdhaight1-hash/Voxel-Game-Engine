# Typed reroute nodes — Issue #10 / Phase 4e

Three new Nodes palette entries — **Color Reroute**, **Scalar Reroute**,
and **Mask Reroute** — organize complex authored procedural materials without
altering their samples. Drag a reroute between an upstream output socket
and downstream input; connect using the existing cables and keyboard/mouse
interactions.

All three share one serialized `NodeKind::Reroute { kind }` with an
explicit `SocketType`. They have exactly one input and one output of
the **same** type. Unlike a conversion node, reroute never implicitly
converts Color into Scalar or Mask, and the graph validator rejects
unsupported types (Vector2/3, Gradient and Curve are still metadata-only
and do not have real scalar/texture execution paths).

Evaluation passes the incoming RGB-represented floating point vector
through unchanged. Scalar and Mask remain typed even though the current
legacy material evaluator still transports their sample as grayscale RGB.
The original `Graph::sample_memo` and exact-UV per-pixel memoizer can
traverse a reroute without quantization, interpolation or new raster
buffers. Socket colors continue to reflect the actual type.

The editor's Nodes palette provides each variant separately. To change
types, replace a reroute rather than silently repurposing existing
cables. Graph JSON persists the node and type; previous serialized
graphs, existing presets, raster hashes and material-function files
remain compatible. A saved graph with an invalid reroute type fails
validation.

Regression tests verify byte-for-byte albedo parity after insertion,
typed invalid wiring, Mask and Scalar preservation, JSON roundtrips,
memoized PBR channel parity and invalid unsupported socket types.

No new dependency, GPU shader, material ownership state, tree geometry,
seasonal palette, dynamic wind or instance/bake invalidation behavior is
introduced. Comment frames and full in-canvas group editing remain
separate future authoring work.
