//! Native voxel-tree generator: occupied cube or hexagonal-prism cells are the
//! source of truth. No curved tubes, leaf cards, imported assets or GPU shaders.
//! Branches and canopies are voxelized into the same discrete world grid.

use crate::voxel::VoxelShape;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::f32::consts::{PI, TAU};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlantPreset { Oak, Pine, Birch, Willow, Maple, Shrub }

impl PlantPreset {
    pub const ALL: [Self; 6] = [Self::Oak, Self::Pine, Self::Birch,
        Self::Willow, Self::Maple, Self::Shrub];
    pub fn name(self) -> &'static str { match self {
        Self::Oak => "Oak", Self::Pine => "Pine", Self::Birch => "Birch",
        Self::Willow => "Willow", Self::Maple => "Maple", Self::Shrub => "Shrub",
    }}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FoliageStage {
    PresetLoader, Carve, Gravity, Slope, Scale, RemoveBranches,
    MeshBuilder, FoliagePalette, FoliageDistributor, Output,
}
impl FoliageStage {
    pub const ALL: [Self; 10] = [
        Self::PresetLoader, Self::Carve, Self::Gravity, Self::Slope, Self::Scale,
        Self::RemoveBranches, Self::MeshBuilder, Self::FoliagePalette,
        Self::FoliageDistributor, Self::Output,
    ];
    pub fn name(self) -> &'static str { match self {
        Self::PresetLoader => "Tree Preset",
        Self::Carve => "Carve",
        Self::Gravity => "Gravity / Phototropism",
        Self::Slope => "Slope",
        Self::Scale => "Scale",
        Self::RemoveBranches => "Remove Branches",
        Self::MeshBuilder => "Voxel Builder",
        Self::FoliagePalette => "Block Palette",
        Self::FoliageDistributor => "Leaf Voxel Distributor",
        Self::Output => "Voxel Export",
    }}
    pub fn group(self) -> &'static str { match self {
        Self::PresetLoader | Self::Carve | Self::Gravity | Self::Slope |
        Self::Scale | Self::RemoveBranches => "VOXEL GROWTH",
        Self::MeshBuilder => "VOXEL GRID",
        Self::FoliagePalette | Self::FoliageDistributor => "VOXEL FOLIAGE",
        Self::Output => "EXPORT",
    }}
}

fn default_voxel_size() -> f32 { 0.4 }
fn default_shape() -> VoxelShape { VoxelShape::Cube }

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct FoliageProject {
    pub preset: PlantPreset,
    pub seed: u32,
    pub height: f32,
    pub trunk_radius: f32,
    pub branch_count: u32,
    pub branch_length: f32,
    pub carve_bottom: f32,
    pub carve_top: f32,
    pub gravity: f32,
    pub phototropism: f32,
    pub slope: f32,
    pub overall_scale: f32,
    pub prune: f32,
    pub leaf_density: f32,
    pub leaf_scale: f32,
    pub leaf_angle: f32,
    pub leaf_variation: f32,
    pub bark_color: [u8; 3],
    pub leaf_color: [u8; 3],
    #[serde(default = "default_shape")]
    pub shape: VoxelShape,
    #[serde(default = "default_voxel_size")]
    pub voxel_size: f32,
}

impl Default for FoliageProject {
    fn default() -> Self { Self::from_preset(PlantPreset::Oak) }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Material { Bark, LeafDark, Leaf, LeafLight }

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct VoxelCell {
    /// Cube: (x,y,z). Hex: (axial q, axial r, z).
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub material: Material,
}

#[derive(Clone, Debug)]
pub struct Triangle {
    pub points: [[f32; 3]; 3],
    pub material: Material,
}

#[derive(Clone, Debug)]
pub struct FoliageMesh {
    pub voxels: Vec<VoxelCell>,
    pub triangles: Vec<Triangle>,
    pub shape: VoxelShape,
    pub voxel_size: f32,
    pub height: f32,
}

impl FoliageProject {
    pub fn from_preset(preset: PlantPreset) -> Self {
        let (height, radius, branches, branch_length, gravity, density, leaf_scale, bark, leaves) =
            match preset {
                PlantPreset::Oak => (7.0, 0.35, 26, 0.65, 0.12, 0.75, 0.57,
                    [91, 67, 45], [67, 135, 52]),
                PlantPreset::Pine => (9.0, 0.27, 35, 0.54, 0.07, 0.8, 0.36,
                    [89, 71, 52], [40, 94, 58]),
                PlantPreset::Birch => (8.0, 0.22, 23, 0.44, 0.07, 0.64, 0.44,
                    [198, 193, 172], [96, 158, 54]),
                PlantPreset::Willow => (7.2, 0.28, 28, 0.75, 0.7, 0.94, 0.48,
                    [91, 86, 61], [91, 156, 65]),
                PlantPreset::Maple => (7.0, 0.32, 29, 0.66, 0.1, 0.8, 0.58,
                    [97, 70, 51], [112, 163, 52]),
                PlantPreset::Shrub => (2.8, 0.17, 32, 0.73, 0.18, 0.95, 0.42,
                    [91, 70, 48], [67, 142, 51]),
            };
        Self {
            preset, seed: 42, height, trunk_radius: radius, branch_count: branches,
            branch_length, carve_bottom: 0.15, carve_top: 0.02,
            gravity, phototropism: 0.15, slope: 0.0, overall_scale: 1.0, prune: 0.0,
            leaf_density: density, leaf_scale, leaf_angle: 0.12, leaf_variation: 0.32,
            bark_color: bark, leaf_color: leaves,
            shape: VoxelShape::Cube, voxel_size: default_voxel_size(),
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        for (name, x, min, max) in [
            ("height", self.height, 0.5, 30.0),
            ("trunk radius", self.trunk_radius, 0.02, 2.0),
            ("branch length", self.branch_length, 0.1, 1.5),
            ("carve bottom", self.carve_bottom, 0.0, 0.85),
            ("carve top", self.carve_top, 0.0, 0.85),
            ("gravity", self.gravity, -1.0, 1.0),
            ("phototropism", self.phototropism, 0.0, 1.0),
            ("slope", self.slope, -0.8, 0.8),
            ("overall scale", self.overall_scale, 0.1, 5.0),
            ("prune", self.prune, 0.0, 1.0),
            ("leaf density", self.leaf_density, 0.0, 1.0),
            ("leaf scale", self.leaf_scale, 0.02, 1.5),
            ("leaf angle", self.leaf_angle, -1.0, 1.0),
            ("leaf variation", self.leaf_variation, 0.0, 1.0),
            ("voxel size", self.voxel_size, 0.1, 1.5),
        ] {
            if !x.is_finite() || x < min || x > max {
                return Err(format!("{name} must be between {min} and {max}"));
            }
        }
        if self.carve_bottom + self.carve_top >= 0.95 {
            return Err("Carve bottom + top must leave at least 5% of the tree".into());
        }
        if !(1..=90).contains(&self.branch_count) {
            return Err("Branch count must be 1..90".into());
        }
        // Bounded volume makes every change predictable in the immediate-mode UI.
        if self.height * self.overall_scale / self.voxel_size > 96.0 {
            return Err("Tree exceeds 96 voxel levels: increase voxel size or reduce height/scale".into());
        }
        if self.branch_length * self.height * self.overall_scale / self.voxel_size > 70.0 {
            return Err("Branch reach exceeds 70 cells: increase voxel size or reduce branch length".into());
        }
        if self.leaf_scale * self.overall_scale * 2.3 / self.voxel_size > 12.0 {
            return Err("Leaf clusters exceed 12 cells in radius: increase voxel size or reduce leaf scale".into());
        }
        if self.trunk_radius * self.overall_scale / self.voxel_size > 8.0 {
            return Err("Trunk exceeds 8 cells in radius: increase voxel size or reduce trunk width".into());
        }
        Ok(())
    }

    /// Generate discrete occupied cells, then surface-triangulate exposed faces only.
    pub fn generate(&self) -> Result<FoliageMesh, String> {
        self.validate()?;
        let mut cells: BTreeMap<[i32; 3], Material> = BTreeMap::new();
        let s = self.voxel_size;
        let h = self.height * self.overall_scale;
        let trunk = |t: f32| -> [f32; 3] {
            [t * t * self.slope * h * 0.32,
             0.015 * h * t * t * (self.gravity - self.phototropism),
             t * h]
        };
        // Trunk and limbs are occupied bark voxels, not smooth cylinder geometry.
        for step in 0..=((h / (s * 0.35)).ceil() as u32) {
            let t = (step as f32 * s * 0.35 / h).min(1.0);
            let p = trunk(t);
            let radius = (self.trunk_radius * self.overall_scale *
                (1.0 - t * 0.80)).max(s * 0.20);
            paint_sphere(&mut cells, self.shape, s, p, radius, Material::Bark);
        }
        for i in 0..self.branch_count {
            let rand = |c| hash01(self.seed, i, c);
            if rand(0) < self.prune { continue; }
            let t = self.carve_bottom +
                (1.0 - self.carve_bottom - self.carve_top) *
                ((i as f32 + 0.5 + 0.25 * (rand(1) - 0.5)) / self.branch_count as f32);
            let base = trunk(t);
            let angle = TAU * (i as f32 * 0.6180339 + rand(2) * 0.2);
            let mut reach = self.branch_length * h * (0.38 + 0.55 * rand(3));
            let base_rise = match self.preset {
                PlantPreset::Pine => { reach *= (1.0 - t).max(0.18); 0.12 },
                PlantPreset::Willow => -0.12,
                PlantPreset::Birch => 0.30,
                PlantPreset::Shrub => 0.30,
                _ => 0.23,
            };
            let rise = reach * (base_rise + 0.65 * self.phototropism - 0.85 * self.gravity);
            let tip = [
                base[0] + reach * angle.cos(),
                base[1] + reach * angle.sin(),
                (base[2] + rise).max(s * 0.5),
            ];
            let mid = lerp3(base, tip, 0.56);
            let thickness = (self.trunk_radius * self.overall_scale *
                (0.28 + 0.25 * (1.0 - t))).max(s * 0.17);
            paint_line(&mut cells, self.shape, s, base, mid, thickness, Material::Bark);
            paint_line(&mut cells, self.shape, s, mid, tip, s * 0.18, Material::Bark);
            let fork_angle = angle + (rand(4) - 0.5) * 1.6;
            let fork_reach = reach * (0.28 + 0.2 * rand(5));
            let fork = [
                mid[0] + fork_reach * fork_angle.cos(),
                mid[1] + fork_reach * fork_angle.sin(),
                (mid[2] + fork_reach *
                    (0.32 + self.phototropism * 0.26 - self.gravity * 0.65)).max(s * 0.5),
            ];
            paint_line(&mut cells, self.shape, s, mid, fork, s * 0.17, Material::Bark);
            if self.leaf_density == 0.0 { continue; }
            // Clusters are solid, jittered voxel ellipsoids. Pine uses narrower
            // growth, willow hangs down; each voxel stores its leaf variation.
            let leaf_tips = [tip, fork];
            for (cluster, location) in leaf_tips.into_iter().enumerate() {
                let r = ((self.leaf_scale * self.overall_scale * 2.3)
                    * (0.75 + 0.5 * rand(6 + cluster as u32))).max(s * 0.82);
                let rz = match self.preset {
                    PlantPreset::Pine => r * 1.55,
                    PlantPreset::Willow => r * 1.3,
                    PlantPreset::Shrub => r * 0.7,
                    _ => r * 0.85,
                };
                let offset_z = match self.preset {
                    PlantPreset::Willow => -0.58 * rz,
                    PlantPreset::Pine => 0.18 * rz,
                    _ => 0.16 * rz,
                };
                let center = [location[0], location[1],
                    (location[2] + offset_z).max(s * 0.5)];
                paint_leaf_cluster(&mut cells, self.shape, s, center, r, rz,
                    self.leaf_density, self.leaf_variation, self.leaf_angle,
                    self.seed, i * 2 + cluster as u32);
            }
            if cells.len() > 40_000 {
                return Err("Voxel tree exceeds 40,000 blocks: increase voxel size or reduce canopy size".into());
            }
        }
        if cells.len() > 40_000 {
            return Err("Voxel tree exceeds 40,000 blocks: increase voxel size or reduce canopy size".into());
        }
        let voxels = cells.iter().map(|(pos, material)| VoxelCell {
            x: pos[0], y: pos[1], z: pos[2], material: *material,
        }).collect();
        let triangles = surface_triangles(&cells, self.shape, s);
        Ok(FoliageMesh { voxels, triangles, shape: self.shape,
            voxel_size: s, height: h })
    }
}

fn hash01(seed: u32, id: u32, channel: u32) -> f32 {
    let mut x = seed ^ id.wrapping_mul(0x9e3779b9) ^
        channel.wrapping_mul(0x85ebca6b);
    x ^= x >> 16; x = x.wrapping_mul(0x7feb352d);
    x ^= x >> 15; x = x.wrapping_mul(0x846ca68b); x ^= x >> 16;
    x as f32 / u32::MAX as f32
}
fn add3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn sub3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn mul3(a: [f32; 3], t: f32) -> [f32; 3] {
    [a[0] * t, a[1] * t, a[2] * t]
}
fn lerp3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    add3(a, mul3(sub3(b, a), t))
}

/// For hex grids, x/y are axial q/r; Z is an integer layer.
fn grid_position(shape: VoxelShape, s: f32, key: [i32; 3]) -> [f32; 3] {
    let x = key[0] as f32;
    let y = key[1] as f32;
    let (x, y) = match shape {
        VoxelShape::Cube => (x * s, y * s),
        VoxelShape::HexPrism => ((x + y * 0.5) * s, y * 0.8660254 * s),
    };
    [x, y, (key[2] as f32 + 0.5) * s]
}
fn round_hex(q: f32, r: f32) -> (i32, i32) {
    let x = q;
    let z = r;
    let y = -x - z;
    let mut rx = x.round();
    let ry = y.round();
    let mut rz = z.round();
    let dx = (rx - x).abs();
    let dy = (ry - y).abs();
    let dz = (rz - z).abs();
    if dx > dy && dx > dz { rx = -ry - rz; }
    else if dz >= dy { rz = -rx - ry; }
    (rx as i32, rz as i32)
}
fn grid_key(shape: VoxelShape, s: f32, world: [f32; 3]) -> [i32; 3] {
    let (x, y) = match shape {
        VoxelShape::Cube => ((world[0] / s).round() as i32,
            (world[1] / s).round() as i32),
        VoxelShape::HexPrism => {
            let r = world[1] / (s * 0.8660254);
            round_hex(world[0] / s - 0.5 * r, r)
        }
    };
    [x, y, (world[2] / s).floor() as i32]
}
fn paint_sphere(cells: &mut BTreeMap<[i32; 3], Material>, shape: VoxelShape,
                s: f32, center: [f32; 3], radius: f32, material: Material) {
    let key = grid_key(shape, s, center);
    let range = ((radius / s).ceil() as i32 + 2).min(32);
    for z in key[2] - range..=key[2] + range {
        if z < 0 { continue; }
        for y in key[1] - range..=key[1] + range {
            for x in key[0] - range..=key[0] + range {
                let pos = [x, y, z];
                let w = grid_position(shape, s, pos);
                let d = sub3(w, center);
                if d[0]*d[0] + d[1]*d[1] + d[2]*d[2] <=
                    (radius + 0.42*s).powi(2) {
                    if material == Material::Bark {
                        cells.insert(pos, material);
                    } else {
                        cells.entry(pos).or_insert(material);
                    }
                }
            }
        }
    }
}
fn paint_line(cells: &mut BTreeMap<[i32; 3], Material>, shape: VoxelShape,
              s: f32, a: [f32; 3], b: [f32; 3], radius: f32, material: Material) {
    let d = sub3(b, a);
    let distance = (d[0]*d[0] + d[1]*d[1] + d[2]*d[2]).sqrt();
    let steps = (distance / (s * 0.32)).ceil().max(1.0) as u32;
    for i in 0..=steps {
        paint_sphere(cells, shape, s, lerp3(a, b, i as f32 / steps as f32),
            radius, material);
    }
}
#[allow(clippy::too_many_arguments)]
fn paint_leaf_cluster(
    cells: &mut BTreeMap<[i32; 3], Material>, shape: VoxelShape, s: f32,
    center: [f32; 3], radius: f32, radius_z: f32,
    density: f32, variation: f32, angle: f32, seed: u32, cluster_id: u32,
) {
    let key = grid_key(shape, s, center);
    let reach = ((radius.max(radius_z) / s).ceil() as i32 + 2).min(32);
    for z in (key[2] - reach).max(0)..=key[2] + reach {
        for y in key[1] - reach..=key[1] + reach {
            for x in key[0] - reach..=key[0] + reach {
                let pos = [x, y, z];
                if cells.contains_key(&pos) { continue; } // bark is never replaced
                let w = grid_position(shape, s, pos);
                let d = sub3(w, center);
                let shear = angle * d[2] * 0.26;
                let nx = (d[0] + shear) / radius;
                let ny = d[1] / radius;
                let nz = d[2] / radius_z.max(0.05);
                let dist = nx*nx + ny*ny + nz*nz;
                let cell_seed = (x as u32).wrapping_mul(73856093) ^
                    (y as u32).wrapping_mul(19349663) ^
                    (z as u32).wrapping_mul(83492791);
                let jitter = hash01(seed ^ cell_seed, cluster_id, 3);
                let edge = 0.80 + variation * (jitter - 0.5) * 0.45;
                if dist > edge.max(0.3) || jitter > density { continue; }
                let color = hash01(seed, cell_seed, cluster_id);
                let material = if color < 0.28 { Material::LeafDark }
                    else if color > 0.77 { Material::LeafLight } else { Material::Leaf };
                cells.insert(pos, material);
            }
        }
    }
}

fn polygon(shape: VoxelShape, center: [f32; 3], s: f32, z: f32) -> Vec<[f32; 3]> {
    let footprint = shape.footprint();
    let radius = match shape {
        VoxelShape::Cube => s * 0.5,
        VoxelShape::HexPrism => s / 3.0_f32.sqrt(),
    };
    footprint.into_iter().map(|p| [
        center[0] + p[0] * radius,
        center[1] + p[1] * radius,
        center[2] + z * s * 0.5,
    ]).collect()
}
fn neighbor(shape: VoxelShape, face: usize) -> [i32; 3] {
    match shape {
        VoxelShape::Cube => match face {
            0 => [0, 1, 0], 1 => [-1, 0, 0],
            2 => [0, -1, 0], _ => [1, 0, 0],
        },
        VoxelShape::HexPrism => match face {
            0 => [0, 1, 0], 1 => [-1, 1, 0],
            2 => [-1, 0, 0], 3 => [0, -1, 0],
            4 => [1, -1, 0], _ => [1, 0, 0],
        }
    }
}
fn push_triangle(out: &mut Vec<Triangle>, a: [f32; 3], b: [f32; 3],
                 c: [f32; 3], material: Material) {
    out.push(Triangle { points: [a, b, c], material });
}
fn surface_triangles(
    cells: &BTreeMap<[i32; 3], Material>, shape: VoxelShape, s: f32,
) -> Vec<Triangle> {
    let mut triangles = Vec::new();
    for (cell, material) in cells {
        let c = grid_position(shape, s, *cell);
        let bottom = polygon(shape, c, s, -1.0);
        let top = polygon(shape, c, s, 1.0);
        let n = bottom.len();
        for face in 0..n {
            let d = neighbor(shape, face);
            let adjacent = [cell[0] + d[0], cell[1] + d[1], cell[2]];
            if cells.contains_key(&adjacent) { continue; }
            let next = (face + 1) % n;
            // Footprints are CCW; side quad normal points outward.
            push_triangle(&mut triangles, bottom[face], bottom[next],
                top[next], *material);
            push_triangle(&mut triangles, bottom[face], top[next],
                top[face], *material);
        }
        if !cells.contains_key(&[cell[0], cell[1], cell[2] + 1]) {
            for i in 1..n-1 {
                push_triangle(&mut triangles, top[0], top[i], top[i+1], *material);
            }
        }
        if !cells.contains_key(&[cell[0], cell[1], cell[2] - 1]) {
            for i in 1..n-1 {
                push_triangle(&mut triangles, bottom[0], bottom[i+1],
                    bottom[i], *material);
            }
        }
    }
    triangles
}

impl FoliageMesh {
    pub fn triangle_count(&self) -> usize { self.triangles.len() }
    pub fn voxel_count(&self) -> usize { self.voxels.len() }

    /// Explicit per-voxel representation: the fundamental export.
    /// OBJ is a view of these blocks for traditional mesh software.
    pub fn voxel_json(&self, project: &FoliageProject) -> Result<String, String> {
        let grid = match self.shape {
            VoxelShape::Cube => "x/y/z Cartesian unit grid",
            VoxelShape::HexPrism => "axial q/r hex grid plus integer z layer",
        };
        serde_json::to_string_pretty(&serde_json::json!({
            "format": "texture-studio-voxel-tree-v1",
            "shape": self.shape,
            "voxel_size": self.voxel_size,
            "coordinate_system": grid,
            "height": self.height,
            "voxels": self.voxels,
            "settings": project,
        })).map_err(|e| e.to_string())
    }

    /// Only exposed faces are exported; shared internal faces are removed.
    /// Every triangle belongs to a snapped cube or hexagonal-prism voxel.
    pub fn obj(&self) -> String {
        let mut output = String::from(
            "# Voxel tree surface mesh - generated from occupied grid cells\n\
             mtllib foliage.mtl\n\
             o voxel_tree\n",
        );
        for face in &self.triangles {
            for p in &face.points {
                output.push_str(&format!("v {:.6} {:.6} {:.6}\n",
                    p[0], p[1], p[2]));
            }
        }
        let mut previous = None;
        for (i, face) in self.triangles.iter().enumerate() {
            if previous != Some(face.material) {
                let name = match face.material {
                    Material::Bark => "bark",
                    Material::LeafDark => "leaf_dark",
                    Material::Leaf => "leaf",
                    Material::LeafLight => "leaf_light",
                };
                output.push_str(&format!("usemtl {name}\n"));
                previous = Some(face.material);
            }
            let p = i * 3 + 1;
            output.push_str(&format!("f {p} {} {}\n", p+1, p+2));
        }
        output
    }
}

pub fn mtl(project: &FoliageProject) -> String {
    let bark = project.bark_color;
    let leaf = project.leaf_color;
    let colors = [
        ("bark", bark),
        ("leaf_dark", leaf.map(|v| (v as f32 * 0.72) as u8)),
        ("leaf", leaf),
        ("leaf_light", leaf.map(|v| (v as f32 * 1.22).min(255.0) as u8)),
    ];
    let mut text = String::from("# Voxel tree bark and leaf block palette\n");
    for (name, c) in colors {
        text.push_str(&format!(
            "newmtl {name}\nKd {:.4} {:.4} {:.4}\nKa 0 0 0\n\n",
            c[0] as f32 / 255.0, c[1] as f32 / 255.0, c[2] as f32 / 255.0
        ));
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_six_species_create_bark_and_leaf_voxels() {
        for preset in PlantPreset::ALL {
            for shape in [VoxelShape::Cube, VoxelShape::HexPrism] {
                let mut tree = FoliageProject::from_preset(preset);
                tree.shape = shape;
                let mesh = tree.generate().unwrap();
                assert!(mesh.voxel_count() > 50, "{preset:?} {shape:?}");
                assert!(mesh.voxels.iter().any(|c| c.material == Material::Bark));
                assert!(mesh.voxels.iter().any(|c| c.material != Material::Bark));
                let unique: std::collections::HashSet<_> =
                    mesh.voxels.iter().map(|v| (v.x, v.y, v.z)).collect();
                assert_eq!(unique.len(), mesh.voxel_count());
                assert!(mesh.triangle_count() > 0);
            }
        }
    }

    #[test]
    fn each_visible_face_is_a_real_cube_or_hex_prism_surface() {
        for shape in [VoxelShape::Cube, VoxelShape::HexPrism] {
            let mut cells = BTreeMap::new();
            cells.insert([0, 0, 0], Material::Bark);
            let faces = surface_triangles(&cells, shape, 1.0);
            let expected = if shape == VoxelShape::Cube { 12 } else { 20 };
            assert_eq!(faces.len(), expected);
            cells.insert([0, 0, 1], Material::Leaf);
            let faces = surface_triangles(&cells, shape, 1.0);
            assert_eq!(faces.len(), expected*2 - if shape == VoxelShape::Cube { 4 } else { 8 });
        }
    }

    #[test]
    fn branch_pruning_and_leaf_density_change_voxel_occupancy() {
        let base = FoliageProject::default();
        let full = base.generate().unwrap();
        let mut bare = base.clone();
        bare.leaf_density = 0.0;
        assert!(bare.generate().unwrap().voxels.iter().all(|v|v.material == Material::Bark));
        let mut pruned = base.clone();
        pruned.prune = 1.0;
        let pruned_mesh = pruned.generate().unwrap();
        assert!(pruned_mesh.voxel_count() < full.voxel_count());
        assert!(pruned_mesh.voxels.iter().all(|v|v.material == Material::Bark));
    }

    #[test]
    fn hex_axial_neighbors_are_exactly_adjacent_without_gaps() {
        let c = grid_position(VoxelShape::HexPrism, 1.0, [0, 0, 0]);
        for face in 0..6 {
            let d = neighbor(VoxelShape::HexPrism, face);
            let next = grid_position(VoxelShape::HexPrism, 1.0, d);
            let distance = sub3(c, next);
            assert!(((distance[0]*distance[0]+distance[1]*distance[1]).sqrt()-1.0).abs()<1e-5);
        }
        for q in -5..=5 {
            for r in -5..=5 {
                let key = [q,r,2];
                assert_eq!(grid_key(VoxelShape::HexPrism, 1.0,
                    grid_position(VoxelShape::HexPrism, 1.0, key)),key);
            }
        }
    }

    #[test]
    fn exported_voxel_data_matches_geometry_and_is_deterministic() {
        for shape in [VoxelShape::Cube, VoxelShape::HexPrism] {
            let mut tree = FoliageProject::from_preset(PlantPreset::Willow);
            tree.shape = shape;
            let a = tree.generate().unwrap();
            let b = tree.generate().unwrap();
            assert_eq!(a.voxel_json(&tree).unwrap(), b.voxel_json(&tree).unwrap());
            assert_eq!(a.obj(), b.obj());
            let json: serde_json::Value =
                serde_json::from_str(&a.voxel_json(&tree).unwrap()).unwrap();
            assert_eq!(json["voxels"].as_array().unwrap().len(), a.voxel_count());
            assert_eq!(json["shape"].as_str(),Some(match shape {
                VoxelShape::Cube => "Cube",VoxelShape::HexPrism => "HexPrism"}));
            assert_eq!(a.obj().lines().filter(|l|l.starts_with("f ")).count(),
                a.triangle_count());
        }
    }

    #[test]
    fn old_foliage_projects_load_with_default_voxel_grid_settings() {
        let mut old = serde_json::to_value(FoliageProject::default()).unwrap();
        let value = old.as_object_mut().unwrap();
        value.remove("shape");
        value.remove("voxel_size");
        value.insert("radial_segments".into(),serde_json::json!(6));
        let loaded: FoliageProject = serde_json::from_value(old).unwrap();
        assert_eq!(loaded.shape,VoxelShape::Cube);
        assert_eq!(loaded.voxel_size,0.4);
        loaded.generate().unwrap();
    }

    #[test]
    fn oversized_and_invalid_resolution_are_rejected() {
        let mut tree = FoliageProject::default();
        tree.voxel_size = 0.1;
        tree.overall_scale = 5.0;
        assert!(tree.validate().is_err());
        tree.voxel_size = f32::NAN;
        assert!(tree.validate().is_err());
        tree.voxel_size = 0.4;
        tree.overall_scale = 1.0;
        tree.leaf_scale = 1.5;
        tree.voxel_size = 0.1;
        assert!(tree.validate().is_err());
    }
}
