# Bézier control handles on the Nodes canvas — Issue #10 / Phase 5d

When an **SDF Bézier Stroke** node is selected, the Nodes canvas shows a
compact 200 px square **Bézier UV Handles** panel beside that node (or to
its left if there is not enough room). This is an editor-only overlay:
it does not introduce a new graph node, cache key, resource, data schema
or runtime parameter.

- The four visible dots represent **start anchor, handle A, handle B,
  end anchor**. Drag any dot to adjust its actual normalized [U,V]
  coordinates. Anchors are amber, handles blue. Every coordinate is
  clamped to the authored 0..1 domain.
- The curve line is drawn with the exact cubic centerline and
  shortest-periodic-control-point unwrap used by the existing CPU SDF
  evaluator. With **Repeat across U/V seams**, the preview wraps and
  breaks at the tile edge rather than drawing a false line through the
  middle of the panel.
- The overlay follows the selected node through the editor's existing
  graph pan/zoom. Its plot uses fixed on-screen dimensions so pointer
  movement maps predictably to fractional UV changes at any zoom.
- The overlay only exists while the Bézier node is selected, and is
  omitted for very small canvas viewports (the inspector sliders remain
  available). It is placed within the visible canvas where possible.
- Dragging these points uses the established `self.dirty` preview
  invalidation path, so graph previews and thumbnails refresh. Moving
  a handle does not change foliage tree/hexel geometry, GPU seasonal
  color palettes, or material roles.
- A cable drawn behind the overlay cannot intercept a point drag;
  cable operations outside the overlay work as before.

This slice edits **one cubic segment**; multiple curves per node,
marquee selection, snap guides, node group frames and arbitrary cubic
contour topology remain future work. Radii, tiling and subdivision
settings remain in the existing inspector. A drag changes the actual
control point values in the current Nodes graph, but the existing
**Apply** button is still required when editing a Bark/Leaf/Opacity
slot, preserving the established material authoring workflow.

Tests cover normalized drag mapping and clamping, periodic and ordinary
centerline consistency, and normal existing material/asset regression
suites. This UI has no PBR output or asset serialization modifications.
