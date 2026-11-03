//! Native texturing support for block voxels and hexagonal-prism voxels.
//! Produces face-resolved UV atlases; the preview uses the same UV convention.
//! Coordinates: z is vertical, x/y are horizontal. Top cap is face N; bottom N+1.

use crate::Graph;
use serde::{Deserialize, Serialize};
use std::f32::consts::PI;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum VoxelShape {
    Cube,
    HexPrism,
}

impl Default for VoxelShape {
    fn default() -> Self { Self::Cube }
}

impl VoxelShape {
    pub fn sides(self) -> usize {
        match self { Self::Cube => 4, Self::HexPrism => 6 }
    }

    pub fn name(self) -> &'static str {
        match self { Self::Cube => "Cube", Self::HexPrism => "Hexagonal prism" }
    }

    pub fn columns(self) -> usize {
        match self { Self::Cube => 3, Self::HexPrism => 4 }
    }

    pub fn rows(self) -> usize { 2 }

    pub fn face_count(self) -> usize { self.sides() + 2 }

    /// Clockwise-wound footprint vertices when viewed from above.
    /// Vertices are used by the atlas and software preview.
    pub fn footprint(self) -> Vec<[f32; 2]> {
        let n = self.sides();
        if self == Self::Cube {
            // Full [-1,1] square: cap UVs cover the same domain as the atlas.
            return vec![[1.0, 1.0], [-1.0, 1.0], [-1.0, -1.0], [1.0, -1.0]];
        }
        (0..n).map(|i| {
            let a = 2.0 * PI * i as f32 / n as f32 + PI / 6.0;
            [a.cos(), a.sin()]
        }).collect()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SideMapping {
    /// Continuously map the texture once around all side walls.
    Wrap,
    /// Repeat the full tile on every side. Useful for seamless tileable materials.
    Repeat,
}

impl Default for SideMapping {
    fn default() -> Self { Self::Wrap }
}

/// Geometric surface selector for CPU-authored voxel face masks.
#[derive(Clone,Copy,Debug,PartialEq,Eq,Serialize,Deserialize)]
pub enum FaceSurface { Side, Top, Bottom }

impl FaceSurface {
    pub fn matches(self,shape:VoxelShape,face:usize)->bool {
        let sides=shape.sides();
        match self {
            Self::Side=>face<sides,
            Self::Top=>face==sides,
            Self::Bottom=>face==sides+1,
        }
    }
}

/// Cardinal *object-local* direction. +Z is geometric up. These are not
/// geographic/world bearings if an asset is rotated in a future game engine.
#[derive(Clone,Copy,Debug,PartialEq,Eq,Serialize,Deserialize)]
pub enum FaceAxis { PosX, NegX, PosY, NegY, PosZ, NegZ }

impl FaceAxis {
    pub fn vector(self)->[f32;3] {
        match self {
            Self::PosX=>[1.0,0.0,0.0], Self::NegX=>[-1.0,0.0,0.0],
            Self::PosY=>[0.0,1.0,0.0], Self::NegY=>[0.0,-1.0,0.0],
            Self::PosZ=>[0.0,0.0,1.0], Self::NegZ=>[0.0,0.0,-1.0],
        }
    }

    /// Hemisphere-projected facing coverage. Sharper exponents narrow the
    /// effect but do not change exact front/back or cap endpoints.
    pub fn coverage(self,normal:[f32;3],sharpness:f32)->f32 {
        let dir=self.vector();
        let facing=(normal[0]*dir[0]+normal[1]*dir[1]+normal[2]*dir[2])
            .clamp(0.0,1.0);
        facing.powf(sharpness)
    }
}

/// Deterministic outward face normal shared by the atlas and software
/// preview. Cube side ordering follows footprint winding, and hex sides
/// correspond to the midpoint normals of consecutive footprint vertices.
/// No per-pixel Vec allocation is needed for this analytic face context.
pub fn face_normal(shape:VoxelShape,face:usize)->Option<[f32;3]> {
    let sides=shape.sides();
    if face==sides {return Some([0.0,0.0,1.0]);}
    if face==sides+1 {return Some([0.0,0.0,-1.0]);}
    if face>=sides {return None;}
    Some(match shape {
        VoxelShape::Cube => match face {
            0=>[0.0,1.0,0.0],1=>[-1.0,0.0,0.0],
            2=>[0.0,-1.0,0.0],_=>[1.0,0.0,0.0],
        },
        VoxelShape::HexPrism => {
            // Six static radial normals; no sin/cos in the per-pixel sampler.
            const H:f32=0.866_025_4;
            const NORMALS:[[f32;3];6]=[
                [0.5,H,0.0],[-0.5,H,0.0],[-1.0,0.0,0.0],
                [-0.5,-H,0.0],[0.5,-H,0.0],[1.0,0.0,0.0],
            ];
            NORMALS[face]
        }
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct VoxelSettings {
    pub shape: VoxelShape,
    pub side_mapping: SideMapping,
    /// Flat 2D texturing: no voxel mesh, UV atlas, or 3D preview.
    /// Missing in older project files, so defaults to false.
    pub geometryless: bool,
}

impl Default for VoxelSettings {
    fn default() -> Self {
        Self { shape: VoxelShape::Cube, side_mapping: SideMapping::Wrap, geometryless: false }
    }
}

#[derive(Debug, Serialize)]
pub struct FaceUv {
    pub name: String,
    pub index: usize,
    pub column: usize,
    pub row: usize,
    pub uv_rect: [f32; 4], // (u_min, v_min, u_max, v_max), v down
}

#[derive(Debug, Serialize)]
pub struct AtlasLayout {
    pub format_version: u32,
    pub shape: VoxelShape,
    pub side_mapping: SideMapping,
    pub tile_size: u32,
    pub width: u32,
    pub height: u32,
    pub columns: usize,
    pub rows: usize,
    pub faces: Vec<FaceUv>,
    pub notes: String,
}

pub fn layout(settings: VoxelSettings, tile_size: u32) -> Result<AtlasLayout, String> {
    if settings.geometryless {
        return Err("Geometryless 2D materials do not have voxel UV atlases".into());
    }
    if !(1..=2048).contains(&tile_size) {
        return Err("Atlas tile size must be 1–2048".into());
    }
    let columns = settings.shape.columns();
    let rows = settings.shape.rows();
    let sides = settings.shape.sides();
    let faces = (0..settings.shape.face_count()).map(|index| {
        let col = index % columns;
        let row = index / columns;
        let name = if index < sides { format!("side_{index}") }
            else if index == sides { "top".into() } else { "bottom".into() };
        FaceUv {
            name, index, column: col, row,
            uv_rect: [
                col as f32 / columns as f32,
                row as f32 / rows as f32,
                (col + 1) as f32 / columns as f32,
                (row + 1) as f32 / rows as f32,
            ],
        }
    }).collect();
    Ok(AtlasLayout {
        format_version: 1,
        shape: settings.shape,
        side_mapping: settings.side_mapping,
        tile_size,
        width: tile_size * columns as u32,
        height: tile_size * rows as u32,
        columns, rows, faces,
        notes: "Each face uses local UV [0,1]x[0,1]; V goes down. Side 0..N-1 follows footprint vertex i to (i+1)%N, top is face N and bottom N+1. Caps use x/y planar UV. Transparent pixels are outside hex cap footprints. Atlas UV rectangles are the face domains; sample inside tiles to avoid bleed.".into(),
    })
}

/// Local face UV to source-material UV; walls are continuous around a voxel when
/// Wrap is chosen. Caps map planar x/y to the same 0..1 material domain.
pub fn source_uv(settings: VoxelSettings, face: usize, u: f32, v: f32) -> [f32; 2] {
    let sides = settings.shape.sides();
    let mapped_u = if face < sides {
        match settings.side_mapping {
            SideMapping::Wrap => (face as f32 + u) / sides as f32,
            SideMapping::Repeat => u,
        }
    } else { u };
    [mapped_u.rem_euclid(1.0), v.rem_euclid(1.0)]
}

/// Check a cap pixel lies inside the voxel's horizontal footprint.
pub fn cap_contains(shape: VoxelShape, u: f32, v: f32) -> bool {
    if shape == VoxelShape::Cube { return true; }
    let px = 2.0 * u - 1.0;
    let py = 1.0 - 2.0 * v;
    let vertices = shape.footprint();
    let mut inside = false;
    for i in 0..vertices.len() {
        let a = vertices[i];
        let b = vertices[(i + 1) % vertices.len()];
        if (a[1] > py) != (b[1] > py)
            && px < (b[0] - a[0]) * (py - a[1]) / (b[1] - a[1]) + a[0]
        {
            inside = !inside;
        }
    }
    inside
}

fn sample_bilinear(pixels: &[u8], width: u32, uv: [f32; 2]) -> [u8; 4] {
    let x = uv[0].rem_euclid(1.0) * width as f32 - 0.5;
    let y = uv[1].rem_euclid(1.0) * width as f32 - 0.5;
    let ix = x.floor() as i32;
    let iy = y.floor() as i32;
    let fx = x - ix as f32;
    let fy = y - iy as f32;
    let at = |dx: i32, dy: i32, channel: usize| -> f32 {
        let px = (ix + dx).rem_euclid(width as i32) as usize;
        let py = (iy + dy).rem_euclid(width as i32) as usize;
        pixels[(py * width as usize + px) * 4 + channel] as f32
    };
    std::array::from_fn(|channel| {
        let top = at(0, 0, channel) * (1.0 - fx) + at(1, 0, channel) * fx;
        let bottom = at(0, 1, channel) * (1.0 - fx) + at(1, 1, channel) * fx;
        (top * (1.0 - fy) + bottom * fy).round().clamp(0.0, 255.0) as u8
    })
}

/// Construct an actual per-face atlas, returning RGBA pixels and deterministic layout.
/// The base texture is evaluated once and resampled onto the voxel's UV faces.
pub fn atlas_from_pixels(
    base: &[u8], base_size: u32, settings: VoxelSettings, tile_size: u32
) -> Result<(AtlasLayout, Vec<u8>), String> {
    if base_size == 0 || base.len() != base_size as usize * base_size as usize * 4 {
        return Err("Expected square RGBA source texture".into());
    }
    let atlas = layout(settings, tile_size)?;
    let mut pixels = vec![0u8; atlas.width as usize * atlas.height as usize * 4];
    for face in &atlas.faces {
        for py in 0..tile_size {
            for px in 0..tile_size {
                let u = (px as f32 + 0.5) / tile_size as f32;
                let v = (py as f32 + 0.5) / tile_size as f32;
                if face.index >= settings.shape.sides() && !cap_contains(settings.shape, u, v) {
                    continue;
                }
                let rgba = sample_bilinear(base, base_size,
                    source_uv(settings, face.index, u, v));
                let ax = face.column as u32 * tile_size + px;
                let ay = face.row as u32 * tile_size + py;
                let start = (ay as usize * atlas.width as usize + ax as usize) * 4;
                pixels[start..start + 4].copy_from_slice(&rgba);
            }
        }
    }
    Ok((atlas, pixels))
}

pub fn render_atlas(
    graph: &Graph, channel: usize, settings: VoxelSettings, tile_size: u32
) -> Result<(AtlasLayout, Vec<u8>), String> {
    if settings.geometryless {
        return Err("Geometryless 2D materials export regular square PBR maps, not atlases".into());
    }
    if graph.uses_face_context() {
        // Geometry-dependent nodes need the target face at evaluation time.
        // Do not change the old bake/resample path for legacy recipes.
        graph.validate()?;
        if channel>4 {return Err("Unsupported voxel atlas channel".into());}
        let atlas=layout(settings,tile_size)?;
        let source_size=tile_size.min(512).max(64);
        let mut pixels=vec![0u8;atlas.width as usize*atlas.height as usize*4];
        for face in &atlas.faces {
            let face_graph=graph.for_voxel_face(settings.shape,face.index);
            let source=if channel==4 {
                face_graph.render_normal_rgba(source_size,source_size)?
            } else {
                face_graph.render_output_channel_rgba(channel,source_size,source_size)?
            };
            for py in 0..tile_size {
                for px in 0..tile_size {
                    let u=(px as f32+0.5)/tile_size as f32;
                    let v=(py as f32+0.5)/tile_size as f32;
                    if face.index>=settings.shape.sides()
                        &&!cap_contains(settings.shape,u,v) {continue;}
                    let rgba=sample_bilinear(&source,source_size,
                        source_uv(settings,face.index,u,v));
                    let ax=face.column as u32*tile_size+px;
                    let ay=face.row as u32*tile_size+py;
                    let index=(ay as usize*atlas.width as usize+ax as usize)*4;
                    pixels[index..index+4].copy_from_slice(&rgba);
                }
            }
        }
        return Ok((atlas,pixels));
    }
    // Capping the intermediate source at 512 keeps export memory predictable.
    let source_size = tile_size.min(512).max(64);
    let pixels = if channel == 4 {
        graph.render_normal_rgba(source_size, source_size)?
    } else {
        graph.render_output_channel_rgba(channel, source_size, source_size)?
    };
    atlas_from_pixels(&pixels, source_size, settings, tile_size)
}

/// Simple isometric software rasterizer. Each visible voxel face uses the same
/// UV convention as the atlas export. No GPU or 3D engine required.
pub fn render_voxel_preview(
    graph: &Graph, settings: VoxelSettings, size: u32, yaw: f32
) -> Result<Vec<u8>, String> {
    if settings.geometryless {
        return Err("Geometryless materials have no 3D preview; use the 2D output".into());
    }
    if !(32..=512).contains(&size) {
        return Err("Voxel preview size must be 32–512".into());
    }
    // Per-face material graphs are opt-in. Without them the legacy preview
    // still bakes only three shared 2D textures, byte-for-byte unchanged.
    let contextual=graph.uses_face_context();
    let (albedo,roughness,metallic)=if contextual {
        (Vec::new(),Vec::new(),Vec::new())
    } else {
        (graph.render_output_channel_rgba(0,size,size)?,
         graph.render_output_channel_rgba(1,size,size)?,
         graph.render_output_channel_rgba(2,size,size)?)
    };
    let face_size=size.min(256);
    let faces:Option<Vec<(Vec<u8>,Vec<u8>,Vec<u8>)>>=if contextual {
        Some((0..settings.shape.face_count()).map(|face|{
            let g=graph.for_voxel_face(settings.shape,face);
            Ok((
                g.render_output_channel_rgba(0,face_size,face_size)?,
                g.render_output_channel_rgba(1,face_size,face_size)?,
                g.render_output_channel_rgba(2,face_size,face_size)?,
            ))
        }).collect::<Result<Vec<_>,String>>()?)
    } else {None};
    let mut image = vec![0u8; size as usize * size as usize * 4];
    let mut depths = vec![-f32::INFINITY; size as usize * size as usize];
    for rgba in image.chunks_exact_mut(4) {
        rgba.copy_from_slice(&[21, 25, 33, 255]);
    }
    let elevation = 0.58_f32;
    let camera = [yaw.cos() * elevation.cos(), yaw.sin() * elevation.cos(), elevation.sin()];
    let right = [-yaw.sin(), yaw.cos(), 0.0];
    let up = [-yaw.cos() * elevation.sin(), -yaw.sin() * elevation.sin(), elevation.cos()];
    let scale = size as f32 * 0.28;
    let project = |xyz: [f32; 3]| -> ([f32; 2], f32) {
        let dot = |a: [f32; 3], b: [f32; 3]| a[0]*b[0]+a[1]*b[1]+a[2]*b[2];
        (
            [size as f32 * 0.5 + dot(xyz, right)*scale,
             size as f32 * 0.53 - dot(xyz, up)*scale],
            dot(xyz, camera),
        )
    };

    let mut raster = |a: ([f32; 3],[f32; 2]), b: ([f32; 3],[f32; 2]), c: ([f32; 3],[f32; 2]),
                      face: usize, normal: [f32; 3]| {
        let (a2, da) = project(a.0);
        let (b2, db) = project(b.0);
        let (c2, dc) = project(c.0);
        let orient = (b2[0]-a2[0])*(c2[1]-a2[1])-(b2[1]-a2[1])*(c2[0]-a2[0]);
        if orient.abs() < 0.0001 { return; }
        let min_x = a2[0].min(b2[0]).min(c2[0]).floor().max(0.0) as u32;
        let max_x = a2[0].max(b2[0]).max(c2[0]).ceil().min((size-1) as f32) as u32;
        let min_y = a2[1].min(b2[1]).min(c2[1]).floor().max(0.0) as u32;
        let max_y = a2[1].max(b2[1]).max(c2[1]).ceil().min((size-1) as f32) as u32;
        let lamp = [0.38, 0.36, 0.85];
        let diffuse = (normal[0]*lamp[0] + normal[1]*lamp[1] + normal[2]*lamp[2]).max(0.0);
        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let p = [x as f32 + 0.5, y as f32 + 0.5];
                let edge = |v0: [f32; 2], v1: [f32; 2]| {
                    (v1[0]-v0[0])*(p[1]-v0[1])-(v1[1]-v0[1])*(p[0]-v0[0])
                };
                let w0 = edge(b2,c2) / orient;
                let w1 = edge(c2,a2) / orient;
                let w2 = 1.0-w0-w1;
                if w0 < -0.00001 || w1 < -0.00001 || w2 < -0.00001 { continue; }
                let depth = da*w0 + db*w1 + dc*w2;
                let dest = (y*size+x) as usize;
                if depth <= depths[dest] { continue; }
                depths[dest] = depth;
                let uv = source_uv(settings, face,
                    a.1[0]*w0+b.1[0]*w1+c.1[0]*w2,
                    a.1[1]*w0+b.1[1]*w1+c.1[1]*w2);
                let (color_map,rough_map,metal_map,map_size)=if let Some(faces)=&faces {
                    let (a,r,m)=&faces[face];
                    (a,r,m,face_size)
                } else {(&albedo,&roughness,&metallic,size)};
                let base=sample_bilinear(color_map,map_size,uv);
                let rough=sample_bilinear(rough_map,map_size,uv)[0] as f32/255.0;
                let metal=sample_bilinear(metal_map,map_size,uv)[0] as f32/255.0;
                let shine = (normal[0]*0.2+normal[1]*0.3+normal[2]*0.93).max(0.0)
                    .powf(10.0 + (1.0-rough)*55.0) * (1.0-rough*0.7);
                for c in 0..3 {
                    let base = base[c] as f32/255.0;
                    let reflectance = 0.04*(1.0-metal)+base*metal;
                    let lit = base*(0.22+0.78*diffuse)*(1.0-0.58*metal)
                        + reflectance*shine;
                    image[dest*4+c] = (lit.clamp(0.0,1.0)*255.0).round() as u8;
                }
            }
        }
    };

    let ring = settings.shape.footprint();
    let sides = ring.len();
    for i in 0..sides {
        let a = ring[i];
        let b = ring[(i+1)%sides];
        let v0 = ([a[0],a[1],-1.0],[0.0,1.0]);
        let v1 = ([b[0],b[1],-1.0],[1.0,1.0]);
        let v2 = ([b[0],b[1], 1.0],[1.0,0.0]);
        let v3 = ([a[0],a[1], 1.0],[0.0,0.0]);
        let dx=b[0]-a[0];
        let dy=b[1]-a[1];
        let len=(dx*dx+dy*dy).sqrt();
        let normal=[dy/len,-dx/len,0.0]; // outward depends on winding
        // Footprint increases CCW, outward is the right-hand perpendicular.
        raster(v0,v1,v2,i,normal);
        raster(v0,v2,v3,i,normal);
    }
    for (face,z,normal) in [(sides,1.0,[0.0,0.0,1.0]),(sides+1,-1.0,[0.0,0.0,-1.0])] {
        let center = ([0.0,0.0,z],[0.5,0.5]);
        for i in 0..sides {
            let a=ring[i];
            let b=ring[(i+1)%sides];
            let va=([a[0],a[1],z],[(a[0]+1.0)*0.5,(1.0-a[1])*0.5]);
            let vb=([b[0],b[1],z],[(b[0]+1.0)*0.5,(1.0-b[1])*0.5]);
            raster(center,va,vb,face,normal);
        }
    }
    Ok(image)
}

/// Write a UV-unwrapped voxel mesh compatible with this atlas (OBJ V is upward).
/// Each side owns 4 vertices and each cap is fan-triangulated.
pub fn export_obj(meta: &AtlasLayout, material_file: &str) -> String {
    let mut output = format!("# Texture Studio native voxel mesh
mtllib {material_file}
o voxel
usemtl voxel_material
");
    let ring = meta.shape.footprint();
    let n = ring.len();
    let mut next_index = 1usize;
    let mut vertex = |out: &mut String, face: usize, xyz: [f32;3], uv: [f32;2]| -> usize {
        let f = &meta.faces[face];
        let u = f.uv_rect[0] + uv[0] * (f.uv_rect[2]-f.uv_rect[0]);
        let v = f.uv_rect[1] + uv[1] * (f.uv_rect[3]-f.uv_rect[1]);
        let idx = next_index;
        next_index += 1;
        out.push_str(&format!("v {:.6} {:.6} {:.6}
vt {:.8} {:.8}
",xyz[0],xyz[1],xyz[2],u,1.0-v));
        idx
    };
    for i in 0..n {
        let a=ring[i];
        let b=ring[(i+1)%n];
        output.push_str(&format!("g side_{i}
"));
        let p = [
            vertex(&mut output,i,[a[0],a[1],-1.0],[0.0,1.0]),
            vertex(&mut output,i,[b[0],b[1],-1.0],[1.0,1.0]),
            vertex(&mut output,i,[b[0],b[1],1.0],[1.0,0.0]),
            vertex(&mut output,i,[a[0],a[1],1.0],[0.0,0.0]),
        ];
        output.push_str(&format!("f {0}/{0} {1}/{1} {2}/{2}
f {0}/{0} {2}/{2} {3}/{3}
",p[0],p[1],p[2],p[3]));
    }
    for (face,z) in [(n,1.0),(n+1,-1.0)] {
        output.push_str(if face==n {"g top
"}else{"g bottom
"});
        for i in 0..n {
            let a=ring[i];
            let b=ring[(i+1)%n];
            let center=vertex(&mut output,face,[0.0,0.0,z],[0.5,0.5]);
            let av=vertex(&mut output,face,[a[0],a[1],z],[(a[0]+1.0)*0.5,(1.0-a[1])*0.5]);
            let bv=vertex(&mut output,face,[b[0],b[1],z],[(b[0]+1.0)*0.5,(1.0-b[1])*0.5]);
            if face == n {
                output.push_str(&format!("f {0}/{0} {1}/{1} {2}/{2}
",center,av,bv));
            } else {
                output.push_str(&format!("f {0}/{0} {2}/{2} {1}/{1}
",center,av,bv));
            }
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn geometryless_rejects_mesh_and_atlas_but_keeps_square_texture_rendering() {
        let settings = VoxelSettings { geometryless: true, ..Default::default() };
        assert!(layout(settings, 32).is_err());
        assert!(render_atlas(&Graph::default(), 0, settings, 32).is_err());
        assert!(render_voxel_preview(&Graph::default(), settings, 64, 0.8).is_err());
        let graph = Graph::default();
        assert_eq!(graph.render_output_channel_rgba(0, 8, 8).unwrap().len(), 8*8*4);
    }

    #[test]
    fn geometryless_is_backward_compatible_and_round_trips() {
        let old = r#"{"shape":"Cube","side_mapping":"Wrap"}"#;
        let old: VoxelSettings = serde_json::from_str(old).unwrap();
        assert!(!old.geometryless);
        let geometryless = VoxelSettings { geometryless: true, ..old };
        let decoded: VoxelSettings =
            serde_json::from_str(&serde_json::to_string(&geometryless).unwrap()).unwrap();
        assert_eq!(decoded, geometryless);
    }

    #[test]
    fn cube_faces_and_dimensions() {
        let settings=VoxelSettings::default();
        let meta=layout(settings,16).unwrap();
        assert_eq!((meta.width,meta.height,meta.faces.len()),(48,32,6));
        assert_eq!(meta.faces[4].name,"top");
        assert_eq!(meta.faces[5].name,"bottom");
    }

    #[test]
    fn hex_faces_and_transparent_corners() {
        let settings=VoxelSettings{shape:VoxelShape::HexPrism,..VoxelSettings::default()};
        let base=vec![255;32*32*4];
        let (meta,atlas)=atlas_from_pixels(&base,32,settings,32).unwrap();
        assert_eq!((meta.width,meta.height,meta.faces.len()),(128,64,8));
        let top=&meta.faces[6];
        let alpha=|x:u32,y:u32|atlas[((y*meta.width+x)*4+3) as usize];
        let ox=top.column as u32*32;
        let oy=top.row as u32*32;
        assert_eq!(alpha(ox,oy),0);
        assert_eq!(alpha(ox+16,oy+16),255);
        assert_eq!(alpha(ox+16,oy+15),255);
    }

    #[test]
    fn wrapped_side_uvs_share_seams() {
        for shape in [VoxelShape::Cube,VoxelShape::HexPrism] {
            let s=VoxelSettings {shape,side_mapping:SideMapping::Wrap, geometryless: false};
            for face in 0..shape.sides() {
                let next=(face+1)%shape.sides();
                assert!((source_uv(s,face,1.0,0.5)[0]-
                    source_uv(s,next,0.0,0.5)[0]).abs() < 0.00001);
            }
        }
    }

    #[test]
    fn repeat_sides_do_not_share_global_uv() {
        let s=VoxelSettings{shape:VoxelShape::HexPrism,side_mapping:SideMapping::Repeat,..Default::default()};
        assert_eq!(source_uv(s,0,0.25,0.5),source_uv(s,5,0.25,0.5));
    }

    #[test]
    fn preview_uses_real_material_pixels() {
        let graph=Graph::default();
        for shape in [VoxelShape::Cube,VoxelShape::HexPrism] {
            let preview=render_voxel_preview(&graph,VoxelSettings{shape,..VoxelSettings::default()},64,0.8).unwrap();
            assert_eq!(preview.len(),64*64*4);
            assert!(preview.chunks_exact(4).any(|px|px[0]!=21));
        }
    }

    #[test]
    fn cube_cap_uv_covers_entire_material_square() {
        let p=VoxelShape::Cube.footprint();
        assert!(p.iter().any(|v| *v==[1.0,1.0]));
        assert!(p.iter().any(|v| *v==[-1.0,-1.0]));
    }

    #[test]
    fn obj_exports_correct_face_and_triangle_counts() {
        for shape in [VoxelShape::Cube,VoxelShape::HexPrism] {
            let meta=layout(VoxelSettings{shape,..Default::default()},32).unwrap();
            let obj=export_obj(&meta,"voxel.mtl");
            let triangles=obj.lines().filter(|line|line.starts_with("f ")).count();
            assert_eq!(triangles,shape.sides()*4);
            assert_eq!(obj.lines().filter(|line|line.starts_with("vt ")).count(),
                obj.lines().filter(|line|line.starts_with("v ")).count());
            assert!(obj.contains("mtllib voxel.mtl"));
        }
    }

    #[test]
    fn invalid_atlas_size_rejected() {
        assert!(layout(VoxelSettings::default(),0).is_err());
        assert!(layout(VoxelSettings::default(),4096).is_err());
    }
}
