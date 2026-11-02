# Reusable versioned material functions — Issue #10 / Phase 4c

This is the first deliberately small step toward material groups and shared
subgraphs. It uses the existing Graph evaluator; there is **no new node runtime**.

## Authoring flow

In **Foliage** or **Assets**, expand **Procedural Material Slots →
Reusable material functions**:

1. Assign a material graph to Bark, Leaf Pattern or Leaf Cutout, using the
   existing preset selection or **Edit slot in Nodes → Apply**.
2. Give the template an ASCII ID (letters/numbers/`_`/`-`) and version
   number, then choose **Export selected slot as function…**. This saves a
   portable JSON `MaterialFunction` file and loads it into the current
   editor session. An existing file is not overwritten.
3. In another tree or asset, use **Import material function…** to load the
   JSON, select that ID/version, and choose **Apply to Bark/Leaf/Cutout**.
   When the template contains a Noise node, the inspector exposes the
   **Instance noise frequency** slider (1..64) and applies its override
   before baking. The first Noise node is exposed as `frequency` by
   default; advanced library JSON supports multiple explicitly exposed
   Noise-scale targets with unique keys.
4. Export a *new version number* when changing a shared function. Old
   references remain pinned and old resolved graphs remain unchanged.

A library can hold up to 128 versions and each function at most 16 exposed
parameters. Function IDs and version pairs are unique. Parameter IDs and
target nodes are validated; unknown keys, duplicated overrides, missing
versions and out-of-range values fail before modifying a material slot.

## Ownership and file format

`MaterialFunction` contains `id`, `version`, `name`, an ordinary `Graph`
and explicit exposed-parameter descriptors. A `MaterialFunctionInstance`
stores `function_id`, `version`, and overrides. Import/export currently
operates on individual JSON function files, **not** a synchronized cloud
library or automatically updating graph-wide repository.

On apply, the editor writes the **fully resolved standalone Graph** into
the same existing `FoliageMaterials.bark/leaf/leaf_opacity` slot used by
the older renderer. An optional `bark_function`, `leaf_function`, or
`opacity_function` record stores the versioned authoring provenance.
Removing the source file cannot break tree rendering, saved Asset JSON,
seasonal palettes, hexel traversal or shared bark/leaf cache keys.
Manual editing through **Edit slot in Nodes → Apply**, or choosing a new
preset, explicitly detaches the previous function reference.

No recursive references are possible in this first function schema:
templates contain ordinary `NodeKind` nodes only, not nested function
calls. General nested groups, dynamic function updates, scalar/curve/color
override kinds, global function sync and in-canvas group editing are
**deferred**, rather than pretending recursive compilation is implemented.

The new provenance fields are optional/defaulted; legacy tree and asset
files still deserialize unchanged. Identical *resolved* graphs continue
to share `FoliageBakeCache` RGBA entries across species, independent of
which template ID generated them. GPU palettes and wind do not cause
material-function rebakes.

## CI coverage

- Different Bark Noise frequencies from the same versioned template produce
  different pixels, with independently saved versions and snapshots.
- Re-import and asset JSON round trips render without the original library.
- Missing versions, duplicate IDs/versions, invalid node targets, duplicated
  override keys and out-of-range Noise scales are rejected.
- Identical rendered graphs from differently named templates reuse the
  existing bounded material bake cache.
- The 1,039 catalog presets and frozen legacy 256/512/1024/2048 hashes
  remain unchanged and run in the regular Rust CI suite.
