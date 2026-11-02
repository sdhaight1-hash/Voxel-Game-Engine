# Curve Remap — Issue #10 / Phase 4b

An editable **Scalar → Scalar** `Curve Remap` filter is now available
in the Nodes palette. It remaps an input value in normalized 0..1 to an
output value in 0..1, without changing any existing `Levels`, `Colorize`
or `LibraryNode` operations.

Use `Noise → Luminance → Curve Remap → Color Ramp → Albedo` to
art-direct the contrast/distribution of a procedural color palette.
You can also feed the curve into a `Threshold Mask` for controlled
transitions in leaf cutout authoring.

The default curve has 4 control points (0,0), (0.25,0.1), (0.75,0.9)
and (1,1). Its native inspector displays the actual sampled curve,
allows values and interior positions to be dragged with sliders, and
supports **2–8 points**. Input-position endpoints stay at 0 and 1, but
their output values may change.

### Interpolation and safety

Curve segments use cubic Hermite interpolation. Neighboring secants
determine monotone-preserving tangents; a change of slope direction
flattens the affected tangent. Final values are clamped to the two
bracketing authored values, preventing curve overshoot. Values always
remain within 0..1. Input/output are clamped to the authored domain.

The representation is bounded and inline to preserve the existing
`NodeKind: Copy` authoring architecture. The graph's existing
validation rejects malformed count, nonfinite coordinates, positions
outside the fixed endpoints, unordered points and values outside
0..1. The node and its control points use ordinary JSON serialization.
Exact-UV memoized and one-channel sampling run the same CPU logic.

No new dependency, GPU shader, tree geometry/occupancy change or
runtime seasonal-palette coupling is introduced. The curve may be
placed in the existing Bark/Leaf graph roles as authored shading.
Reusable graph functions and per-instance parameter overrides remain
later Phase 4 work.
