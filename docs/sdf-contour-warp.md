# Periodic SDF contour distortion — Issue #10 / Phase 5c

Add **SDF Contour Warp** to the Nodes palette to distort shape outlines
without converting signed distances to color or clipping their interior.

The node accepts and outputs the typed **Distance** socket. It samples
the upstream signed-distance function at new UV coordinates displaced
by two independently seeded periodic noise fields. A zero UV offset is
an exact pass-through. The frequency (1–64), offset amount (0..0.25 in
normalized UV) and unsigned seed are editable in the inspector and
persist directly in graph JSON.

The noise fields repeat at the texture's U/V seams, so a tileable
Circle/Ellipse/Capsule/Bézier stroke remains tileable after distortion.
The sampler remains lazy and floating-point: it evaluates the original
shape at the warped UV, **not** a quantized raster of its output. The
distorted result preserves the inside/outside sign, but its magnitude
is no longer guaranteed to be a true Euclidean signed distance.
Use **Distance to Mask** after SDF Contour Warp to produce bounded
coverage for albedo, height, Bark or Leaf Cutout.

Example: `SDF Bézier Stroke → SDF Contour Warp → Distance to Mask
→ Material Output: Albedo / Height`.

This is not the older RGB **Warp** library node: that operation accepts
a Color input, whereas SDF Contour Warp requires Distance and preserves
the signed domain. Neither automatically converts Color to Distance.

Validation rejects out-of-range frequency, nonfinite/negative UV
amount and mismatched input types. Tests cover zero-amplitude byte
equivalence, deterministic periodic edge continuity, typed sockets,
JSON round trips, memoized multi-channel parity, and Bark/Leaf cutout
with a warped Bézier stroke.

The implementation is additive and does not affect legacy graph
presets, GPU tree-season palettes, hexel occupancy, wind or material
cache ownership. Displacement is normalized UV, not meters, and there
is no GPU signed-distance shader, path editor, or pixel-footprint-aware
antialiasing in this slice.
