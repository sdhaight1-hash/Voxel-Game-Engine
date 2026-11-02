# Coordinate-aware multi-channel node memoization — issue #10

This is the next **experimental execution API** behind the established
`Graph` sampler. It does not replace or silently alter the default
single-channel material evaluator.

`Graph::render_channels_memoized_rgba(channels,width,height)` renders up
to four PBR output channels in one traversal. It retains evaluated upstream
node **f32** results inside one pixel, keyed by
`(node_id, u.to_bits(), v.to_bits())`. If two channels share a Noise,
Colorize, Mask or other node at the same coordinates, that result is
reused rather than recalculated.

## Why exact-UV memoization matters

Coordinate transforms (Warp, Rotate, Tile, etc.) and neighborhood filters
(Blur, Edge Detect, Normal Map, etc.) ask upstream nodes for coordinates
different from the current output pixel. A cached upstream RGBA raster
cannot generally answer those queries without interpolation and changed
pixels. This implementation memoizes only when **both** the node ID and
the original float UV bit pattern agree. It does not quantize intermediate
colors to u8, does not interpolate source images and does not assume
regular pixel-center sampling.

A short planning pass identifies nodes in the upstream dependency closure
of more than one requested output channel. Only those nodes are memoized.
Each selected shared node keeps only its most recently evaluated
exact-UV sample in a `Cell`. A fixed list of up to 64 shared nodes is
cleared between pixels; additional shared nodes evaluate normally without
caching. This avoids per-pixel hash maps, allocations and RefCell lookups. This is an in-flight evaluation cache, not a persistent preview
cache; the
existing `GraphRenderCache` remains responsible for complete rendered
outputs and the `FoliageBakeCache` remains responsible for full bark/leaf
material recipes.

## Verification and limitations

Tests compare every returned pixel with the existing
`render_output_channel_rgba` for all 1,039 bundled material presets,
and separately exercise shared seeded Noise, Warp, Rotate, Blur and Edge
Detect. Disconnected/default channels, invalid project state and invalid
dimensions are also tested. The API returns CPU profiling counters for
sample reuse, evaluations and maximum retained entries per pixel.

The CI ignored benchmark test compares four independent channel renders
against one memoized multi-output pass at 64px and 256px using the same
seeded shared branch. Any performance claim must be based on those numbers;
for lightweight graphs, HashMap/RefCell overhead may make the memoized
path slower despite fewer node evaluations.

**Integration gate:** do not route all exports or preview paths through
the new API until benchmark results justify that decision. Normal-map
derivation, user-defined graph evaluation, legacy file serialization and
single-output sampling are unchanged here. Long-lived per-node image
caching and full dependency-ordered execution remain future work.
