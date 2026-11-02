# Nested material functions — issue #10 / Phase 4d

This is a **bounded authoring-time group compiler**, not a second node
runtime or recursive shader system. It extends the versioned function files
from Phase 4c and retains all previous material/asset export formats.

## How to author a nested function

1. Import a child `MaterialFunction` JSON using **Procedural Material Slots →
   Reusable material functions**. The child must have its **Albedo** output
   connected; only that output is used by a nested call.
2. Edit the parent Bark, Leaf Pattern or Leaf Cutout slot in **Nodes**.
   Add a normal **Color** node where the child output should enter the
   parent graph, and connect its output to the desired downstream Color
   input or the parent Material Output. Its color is only a placeholder.
   Apply the edited graph back to the material slot.
3. In the material-function panel choose the child ID/version, then choose
   **Color node #…** in the Nested material function section.
   Change **Instance noise frequency** if desired, set a new parent function
   ID/version, and choose **Export slot as nested function…**.
4. The resulting JSON contains the parent graph and an explicit
   `calls` entry: the placeholder node ID and a pinned child
   `MaterialFunctionInstance` (ID, version, overrides).
5. Import the parent JSON and its dependencies into the authoring session,
   then apply the parent version as an ordinary material function.
   The compiler expands the call into a standalone `Graph` before
   writing it to Bark, Leaf or Leaf Cutout. **Instance noise frequency**
   is forwarded to the chosen child function, including across multiple
   nested levels. Changing that slider changes the child's sampled pixels,
   not an unused Noise node left in the parent template.

The parent and its child versions are independently pinned. A dependency's
source file or later edits never silently replace a saved tree or asset's
resolved graph.

## Evaluation and safeguards

- The child graph's Material Output **Albedo input** supplies the signal
  replacing the selected parent Color node. Child nodes and links are
  remapped to unique parent node IDs. Its Output node is not copied.
  This preserves the existing typed socket validation, preview caches,
  four PBR channel positions and the original CPU and exact-UV samplers.
- Supports nested calls up to 8 levels and a maximum of 16 calls per
  function; flattened graphs are capped at 2,048 nodes.
- A call must target an actual **Color** node; duplicate targets,
  missing child versions, missing connected child Albedo, invalid
  overrides, directly recursive calls and indirectly cyclic references
  fail validation without partially applying to the material slot.
- The imported library permits at most 128 versioned entries. Import
  dependencies before applying a parent; function-library validation
  traverses all referenced dependencies and catches missing or cyclic
  versions.
- Saved FoliageMaterials and VoxelAsset contain the compiled flat graph
  plus optional authoring provenance, **not** runtime function calls.
  Wind, seasonal palettes and hexel geometry remain independent.
  Identical resolved graph recipes continue to reuse the existing
  bounded FoliageBakeCache across tree instances.

This first UI action creates **one call per exported parent function**.
The JSON library schema and resolver can represent several nonoverlapping
calls in the same template. Reusable typed function input/output sockets,
in-canvas group editing, draggable frames and automatic live library sync
remain later Phase 4 work.
