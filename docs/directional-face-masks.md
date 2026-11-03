# Phase 7b — object-local directional facing masks (#10)

The **Face Facing Mask** is an opt-in typed Mask node for the existing Rust
Graph evaluator. Its parameters are **Object-local direction** (+X, -X,
+Y, -Y, +Z, -Z) and **Facing sharpness** (exponent 0.25..8, default 1).

The exact face coverage is

    max(0, dot(outward_unit_face_normal, requested_axis)) ^ sharpness

This uses **real outward face normals** matching the same cube and hex
prism geometry and face ordering as the voxel atlas and CPU 3D preview:
the cube has 4 side normals and top/bottom caps; the hexagonal prism
has 6 side normals at 60-degree increments and top/bottom caps.

For example, a +Z mask selects top caps for snow or dust. A +Y mask
can emphasize side surfaces oriented toward local +Y, with gradual
side-facing coverage for hexagonal prism facets. Use the same mask to
drive existing Mask Mix across Albedo, Roughness, Metallic and Height
or combine it with Height Blend Mask for a conditional overlay.

## Coordinate and compatibility contract

- **Object-local axes** only. +Z is the geometry up axis; +X/+Y are the
  editor's object axes. The node does *not* know about a future object's
  orientation in a world or the geographic north pole.
- For plain 2D graph/node preview and existing Flora/leaf/bark material
  baking, no voxel face is available: the node returns neutral **0**.
  It does not fabricate geometry normals from UV or noise.
- Contextual cube/hex voxel atlas rendering and voxel software 3D preview
  use the **same existing face context** introduced in Phase 7a. A
  disconnected Face Facing Mask does not trigger per-face baking.
- Serialized node parameters contain only axis and sharpness. Face context
  itself is not saved; old project and preset JSON remains valid.
- The existing preview-cache key already contains the ephemeral shape and
  face selector, preventing accidental cache reuse between directions.
- No tree geometry is changed, no occupancy is modified, and the existing
  1,067 material / 43 Flora preset builders are unmodified.

## Correctness and cost

The analytic normal is a unit vector derived from face index/shape, with
no per-sample allocation. For the hex shape, compare each side's normal
against the cross/perpendicular of its real footprint edge. The software
preview remains bounded and only evaluates per-face textures if a
directional mask is connected to Material Output.

Regression gates cover normals vs mesh winding, both side-UV mappings,
all six directions, three sharpness values, all four PBR channels, 2D
fallback, JSON roundtrips, disconnected performance parity and a 3D
preview that differentiates +Z from -Z.

## Explicitly not addressed

This is **flat-face orientation**, not geometry-derived per-vertex normal
maps, branch curvature, world-space climate windwardness, ambient
occlusion, or geographic aspect. For organic tree bark, existing V UVs
track branch arc length in meters, while U is still an angular fraction
around the circumference. A truly **meters_per_repeat** setting must
be implemented at the tree mesh/material binding boundary (geometry
provides actual circumference/length; material controls repetition);
blindly scaling current UVs would cause stretch and seams. That work
remains a separately testable Phase 8 slice.

The hexagonal prism's six outward side normals are stored as a fixed
unit-vector table, so evaluating the Mask does not calculate trig per pixel.
