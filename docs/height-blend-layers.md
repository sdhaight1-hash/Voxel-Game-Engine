# Height-aware PBR layer blending — Issue #10 Phase 6b

## Artist workflow

Two additive nodes operate inside the **existing** serializable Rust Graph:

1. **Height Blend Mask** accepts **Height A: Scalar**, **Height B: Scalar**,
   and **Base Mask: Mask**, and outputs a typed **Mask**.
2. **Mask Mix** accepts **A: Color**, **B: Color**, and **Mix Mask: Mask**,
   returning a **Color**. Safe Scalar or Mask -> Color conversions continue to
   use the existing typed socket rules. No Color -> Scalar inference is added.

Wire the same Height Blend Mask into one Mask Mix **per PBR output channel**:
Albedo, Roughness, Metallic and Height. The same mask may also be used in an
authored leaf-opacity/cutout graph, subject to the existing Foliage material
ownership and subtractive cutout rules. Height fields sourced from legacy RGB
generators must pass through explicit **Luminance**. A procedural Mask may
come from **Threshold Mask**, **Distance to Mask** or **Shape Scatter**.

### Mathematical behavior

For normalized heights A/B, authored blend mask m, and Height Influence k
(0..1), the effective blend weight is:

    envelope = 4*m*(1-m)
    weight = clamp(m + (heightB-heightA)*k*envelope, 0, 1)

This is a **height-biased transition**, not physically simulated displacement.
The higher-height surface wins more of the transition region. At base mask 0
and 1 it is *exactly* 0 and 1; identical heights or zero influence reproduce
the authored mask exactly. All resulting PBR channels are mixed using the
same weight (linear mix of channel values), not independently biased. This
retains sharp authored material bounds.

The normal-map export remains **derived from the final blended Height output**,
using the project's existing normal convention. This slice does **not**
blend two independent tangent-space normal maps or add a Normal socket.
Likewise, linear channel interpolation inherits the existing CPU material
export color-space semantics; it is not a new GPU PBR shading model.

### Compatibility and limits

- These nodes are opt-in; old enum variants, link indices, graph JSON, 1,067
  material presets, all 43 Flora trees and cache ownership are unchanged.
- Existing 4-channel PBR output order is unchanged. A missing input continues
  to sample zero via the existing graph contract.
- One weight source feeding multiple channel mixes is important: separately
  computed masks can produce mismatched transitions.
- A full layered material resource/stack, color-space-aware PBR compositing,
  independent normal-map blending, imported bitmap resources and region IDs
  remain later Phase 6/7/9 work.
- This is **CPU authoring/bake**; GPU viewport roughness/normal support is a
  separate Phase 9 task.

Validation: typed connections, malformed influence, mask endpoints/flat
height, cross-channel exact-UV memo parity, JSON roundtrip and frozen legacy
fixtures. Full Rust desktop CI is required before merging.
