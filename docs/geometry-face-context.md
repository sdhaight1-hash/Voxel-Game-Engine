# Phase 7a — geometry-aware voxel face masks (Issue #10)

**First opt-in geometry-context slice.** A new Face Surface Mask node
outputs a typed Mask: 1.0 on selected voxel faces and 0.0 elsewhere.

Choose **Side walls**, **Top cap**, or **Bottom cap** in the node inspector.
Connect the same Mask to Mask Mix, Height Blend Mask or other existing typed
mask consumers. Reuse it across Albedo, Roughness, Metallic, Height.

The voxel atlas exporter and software 3D preview **evaluate the Graph per
face** when a Face Surface Mask exists, using the same CPU Rust sampler as
2D and foliage. Both Cube (4 sides + caps) and Hexagonal prism (6 sides +
caps) are supported, with both perimeter-wrap and repeat-per-side UV modes.
The Side selector matches every side, not a single compass orientation.

Face context is ephemeral runtime state omitted from serialized projects.
Context-free 2D, selected-node, and Flora bakes get a neutral **zero** mask.

## Compatibility and performance

Existing materials without this node keep the original 2D texture
bake/resample atlas path and the original three-texture 3D-preview path.
No existing material or species preset is rewritten.

For face-aware recipes, the atlas renders each face at a bounded source
resolution of up to 512 before resampling into its tile. The software
3D preview renders up to eight sets of Albedo/Roughness/Metallic textures,
each capped at 256x256. This is CPU-intensive relative to one shared tile
and deliberately opt-in. All outputs are deterministically bakeable.

The material preview cache includes transient face context in its key:
top-face renders never reuse a side-face render by accident. Saving a
graph does not save this context. No new shader or second evaluator exists.

## Not yet supported

World-space triplanar textures, real-meter UV scaling, vertex normals,
voxel neighbors, curvature and heightfield slope, foliage-mesh local
coordinates, advanced layered materials or full GPU PBR shaders all
need further Phase 7/9 work. These must be designed around consistent
physical units and mesh-space inputs, not invented from 2D UV.
