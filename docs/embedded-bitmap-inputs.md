# Phase 6d — Portable embedded PNG material sources (#10)

Add **Bitmap Color** or **Bitmap Alpha** in the native Nodes palette,
select the node, and click **Import PNG into project**. You may select
previously imported images from the same graph in the node inspector.
The original absolute file path is never saved or used when rendering.

- **Bitmap Color** emits unpremultiplied RGB via a typed Color socket.
- **Bitmap Alpha** emits A via a typed Mask socket for Shape Scatter,
  Height Blend Mask, Mask Mix or an authored foliage cutout.
- Nearest and bilinear filters both repeat U/V, including warped samples.
- An unassigned node (ID zero) is black/zero; a missing nonzero ID is invalid.
- The JSON project owns RGBA8 pixels, source dimensions, a safe filename
  label and a validated deterministic content fingerprint. Older projects
  have no image registry field and retain their original behavior.

**Bounds:** PNG input <=4 MiB; decoder internal limit 8 MiB; decoded
images 1–512 pixels per axis; at most 16 images and 8 MiB of decoded pixel
bytes in one graph. PNG is expanded to 8-bit RGBA, including indexed and
grayscale modes; animated PNGs are rejected. No extra dependencies.

**Compatibility:** graph preview caches include fingerprints of images
that affect their requested output. Flora bake caches include image
fingerprints in their recipes. Nested material functions copy/deduplicate
child image resources and remap child asset IDs, rather than picking an
unrelated parent image when IDs collide. All current material and Flora
preset graph builders remain untouched.

PNG input is decoded once on import, not during sample evaluation. The
first implementation stores decoded RGBA8 in JSON, not compressed PNG;
projects can be substantially larger. FNV-1a is a cache consistency
checksum, not a cryptographic content signature. Full texture-array
GPU integration, PBR lighting, ICC conversion, HDR and a compressed
asset-bundle format remain future tasks.
