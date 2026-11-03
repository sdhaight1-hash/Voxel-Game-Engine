# Phase 8a — opt-in physically scaled organic bark (Issue #10)

FoliageMaterials now owns a setting named bark_meters_per_repeat, optional
floating-point meters per tile (0.05 to 8.0 m). The default is absent, so
every previous tree, material preset, Flora prefab and saved asset continues
to use exactly the legacy UV and pixel sampling.

## Coordinate contract

The existing organic branch mesh has U = fraction around the wood ring
and V = cumulative branch-arc distance in METERS. Its six-sided tubes
now also carry per-vertex polygon circumference in METERS = 6 * radius,
including both ends of a tapered segment. When a triangle is clipped to
harvestable hexels, the same unwrapped V coordinate interpolates its
physical circumference. This uses the existing wood mesher and clipper,
not a duplicate geometry subsystem. Hexel occupancy is unchanged.

When the option is enabled, the wgpu preview converts the source bark
tile UVs using:

- tiles around ring = interpolated circumference_m / meters_per_repeat
- U = angular ring fraction * tiles around ring
- V = branch-arc meters / meters_per_repeat

A fractional number of tiles cannot perfectly meet at the closed ring
seam. The shader uses a smooth blend over U = 0.80..1.00 between the
ordinary physical sample and the same tile sampled after subtracting the
full ring's repeat count. The last angular fifth sacrifices locally exact
texel scale to eliminate discontinuities at U = 0/1 without changing
repeat counts along a taper.

No additional bark texture bake is performed. Changing only this setting
updates the scene's GPU uniform, leaving geometry, bark RGBA, leaf RGBA and
seasonal palette uploads untouched. The authoring UI supports Tree and
editable Asset Bark slots, and the option survives JSON roundtrips.

## Deliberate limits

- Applies to organic six-sided tube sides and small cylindrical branch
  connectors with measured circumference. Flat block bark, thin twig
  ribbons, tube caps and leaf cards retain their old UVs. No artificial
  radius is invented for those surfaces.
- Exported OBJ/MTL currently uses the original UVs. Standard OBJ UVs
  cannot encode a circumference-dependent, seam-blended shader.
  Interactive metric preview and OBJ output therefore do not yet claim
  pixel equivalence in the opt-in mode; a dedicated static export bake
  remains for a subsequent integration phase.
- Existing bark normal and roughness maps still export separately;
  Phase 9 must connect true PBR maps in the GPU viewer.
- Meters refer to the tree mesh's established 1m hexel coordinate system,
  not geographic/world scale. Projected wind and seasonal appearance
  remain independent of physical bark coordinates.

Regression tests cover JSON fallback, 0.05..8 validation, bake-cache
reuse after scale-only changes, wood hexel and mesh equivalence, clipped
physical circumference, material persistence after Asset conversion,
and GPU scene flags showing uniform-only scale changes.
