# Phase 9f — OBJ/MTL PBR interoperability, face normals and tangent handedness (#10)

This is a **bounded enhancement of the existing static exports**.
It does not implement a new glTF backend, MikkTSpace, or pixel-identical
offline GPU rendering. It makes materials discoverable by common
extended-MTL readers and exports explicit, validated face normals.

## Exported files and semantics

**Export Voxel Tree** and **Export Asset** now use an enhanced OBJ+MTL
path. The existing library \`obj()\`, \`obj_with_bark_meters_per_repeat()\`
and \`mtl()\` functions stay byte-identical for legacy clients. The
enhanced APIs are \`FoliageMesh::obj_with_pbr_normals()\`,
\`foliage::mtl_pbr()\`, \`VoxelAsset::obj_pbr()\`, and \`VoxelAsset::mtl_pbr()\`.

- OBJ writes one normalized \`vn\` per actual, already-triangulated face
  (the same geometric cross-product normal as the wgpu preview), and
  \`f v/vt/vn\` triplets. It retains unchanged positions, per-face
  material assignments and the exact previous UVs, including the
  opt-in physically scaled bark coordinates and seam approximation.
- MTL keeps all existing \`Kd\`, \`map_Kd\` and \`map_d\` instructions.
  Extended PBR MTL adds \`Pr\` and \`map_Pr\` for roughness, \`Pm\` and
  \`map_Pm\` for bark metallic, and \`norm\` for tangent normal textures.
  These are **widely supported extensions**, not guaranteed by the
  original Wavefront MTL grammar. Importers may ignore them or infer
  different PBR/alpha settings.
- \`bark-metallic.png\` now derives directly from the existing bark
  graph's Metallic output (channel 2). Existing roughness/normal
  channels are reused unchanged, with no new material graph nodes.
- OBJ exports UV V as \`1 - v\`, so importers deriving a tangent basis
  from those UVs invert the bitangent relative to our wgpu preview.
  The additional \`bark-normal-obj.png\` and
  \`foliage-leaves-normal-obj.png\` invert only the **G** channel
  to compensate; the original \`-normal.png\` pixels remain unchanged
  for the preview. The transformation introduces only inherent 8-bit
  rounding. The \`norm\` MTL entries reference the OBJ-oriented variants.
- The exported file existence checks include all newly generated
  textures, preventing accidentally overwriting an existing asset.

## Important parity boundaries

- **OBJ has no vertex tangent attribute**, only geometric normals.
  Its external renderer reconstructs tangents, possibly with
  MikkTSpace. Our current renderer still uses **flat per-triangle UV
  tangents**, not a certified full MikkTSpace implementation. A hard
  edge/UV seam or strongly tapered wood can produce different shading
  in external DCC applications.
- Standard OBJ/MTL cannot reproduce the meter-scaled GPU **two-sample
  bark seam-color blend**: our existing static bark UVs use an
  approximate seam phase correction, as documented in Phase 8b.
- The editor's studio IBL, seasonal shader gradients, procedural wind,
  leaf backlighting, and ordered/MSAA alpha masks remain runtime
  features and cannot be encoded in an extended MTL file. A plain
  PBR MTL map reference is **not complete appearance parity**.
- For exact cross-application tangents and material semantics, the
  remaining design target should be an **explicit glTF 2.0 export** with
  certified MikkTSpace tangents (or validated reference tangents),
  metallic-roughness packing, alpha modes and a defined normal-map
  convention. That is deliberately **not** implied by this PR.

## Tests

Engine tests assert byte-identical old APIs; PBR OBJ normals are
finite, normalized and indexed correctly; UVs/materials/mesh cell
ownership remain unchanged; Tree/Asset/JSON roundtrip preserves
authored scale; every extended MTL reference has a corresponding
baked data map; OBJ normal-G inversion exactly matches the preexisting
native map while leaving its R/B/A channels untouched; and the full
Rust engine/desktop + Naga/WGSL CI gates remain mandatory.
