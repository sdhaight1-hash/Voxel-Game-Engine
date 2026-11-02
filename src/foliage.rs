//! Native voxel-tree generator: occupied cube or hexagonal-prism cells are the
//! source of truth. Bark is solid grid geometry; foliage cells anchor custom
//! intersecting cutout cards. There is no dependency on imported game assets.

use crate::voxel::VoxelShape;
use crate::foliage_materials::FoliageMaterials;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::f32::consts::{PI, TAU};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlantPreset {
    Acacia, Birch, Ebony, Kapok, Maple, CrimsonKingMaple, Oak,
    Purpleheart, Walnut, Willow,
    BaldCypress, MediterraneanCypress, Larch, Pine, Redwood,
    RedApple, PinkApple, YellowApple, Pear, Olive, Peach, Cherry,
    Mango, Orange, Lychee, Breadfruit, Pomegranate,
    GreenBamboo, BrownBamboo,
    Shrub,
}

impl PlantPreset {
    /// The user's full natural-world catalog: 27 tree/fruit varieties + 2 bamboo.
    pub const TREE_CATALOG: [Self; 29] = [
        Self::Acacia, Self::Birch, Self::Ebony, Self::Kapok, Self::Maple,
        Self::CrimsonKingMaple, Self::Oak, Self::Purpleheart, Self::Walnut,
        Self::Willow, Self::BaldCypress, Self::MediterraneanCypress,
        Self::Larch, Self::Pine, Self::Redwood,
        Self::RedApple, Self::PinkApple, Self::YellowApple, Self::Pear,
        Self::Olive, Self::Peach, Self::Cherry, Self::Mango, Self::Orange,
        Self::Lychee, Self::Breadfruit, Self::Pomegranate,
        Self::GreenBamboo, Self::BrownBamboo,
    ];
    /// Shrub remains a separate non-tree authoring preset.
    pub const ALL: [Self; 30] = [
        Self::Acacia, Self::Birch, Self::Ebony, Self::Kapok, Self::Maple,
        Self::CrimsonKingMaple, Self::Oak, Self::Purpleheart, Self::Walnut,
        Self::Willow, Self::BaldCypress, Self::MediterraneanCypress,
        Self::Larch, Self::Pine, Self::Redwood,
        Self::RedApple, Self::PinkApple, Self::YellowApple, Self::Pear,
        Self::Olive, Self::Peach, Self::Cherry, Self::Mango, Self::Orange,
        Self::Lychee, Self::Breadfruit, Self::Pomegranate,
        Self::GreenBamboo, Self::BrownBamboo, Self::Shrub,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::Acacia=>"Acacia", Self::Birch=>"Birch", Self::Ebony=>"Ebony",
            Self::Kapok=>"Kapok", Self::Maple=>"Maple",
            Self::CrimsonKingMaple=>"Crimson King Maple", Self::Oak=>"Oak",
            Self::Purpleheart=>"Purpleheart", Self::Walnut=>"Walnut",
            Self::Willow=>"Willow", Self::BaldCypress=>"Bald Cypress",
            Self::MediterraneanCypress=>"Mediterranean Cypress",
            Self::Larch=>"Larch", Self::Pine=>"Pine", Self::Redwood=>"Redwood",
            Self::RedApple=>"Red Apple", Self::PinkApple=>"Pink Apple",
            Self::YellowApple=>"Yellow Apple", Self::Pear=>"Pear",
            Self::Olive=>"Olive", Self::Peach=>"Peach", Self::Cherry=>"Cherry",
            Self::Mango=>"Mango", Self::Orange=>"Orange", Self::Lychee=>"Lychee",
            Self::Breadfruit=>"Breadfruit", Self::Pomegranate=>"Pomegranate",
            Self::GreenBamboo=>"Green Bamboo", Self::BrownBamboo=>"Brown Bamboo",
            Self::Shrub=>"Shrub",
        }
    }
    pub fn group(self) -> &'static str {
        match self {
            Self::Acacia|Self::Birch|Self::Ebony|Self::Kapok|Self::Maple|
            Self::CrimsonKingMaple|Self::Oak|Self::Purpleheart|Self::Walnut|
            Self::Willow => "Hardwood & broadleaf",
            Self::BaldCypress|Self::MediterraneanCypress|Self::Larch|
            Self::Pine|Self::Redwood => "Conifer & evergreen",
            Self::GreenBamboo|Self::BrownBamboo => "Bamboo",
            Self::Shrub => "Other foliage",
            _ => "Fruit trees",
        }
    }
    /// Material-catalog titles use sentence-case species names.
    pub fn material_name(self) -> &'static str {
        match self {
            Self::CrimsonKingMaple=>"Crimson King maple",
            Self::BaldCypress=>"Bald cypress",
            Self::MediterraneanCypress=>"Mediterranean cypress",
            Self::RedApple=>"Red apple", Self::PinkApple=>"Pink apple",
            Self::YellowApple=>"Yellow apple",
            Self::GreenBamboo=>"Green bamboo", Self::BrownBamboo=>"Brown bamboo",
            Self::Shrub=>"Oak",
            _ => self.name(),
        }
    }
    pub fn is_conifer(self) -> bool {
        matches!(self,Self::BaldCypress|Self::MediterraneanCypress|
            Self::Larch|Self::Pine|Self::Redwood)
    }
    pub fn is_bamboo(self) -> bool {
        matches!(self,Self::GreenBamboo|Self::BrownBamboo)
    }
    /// Visual ripened-fruit color, not a spawned or harvestable fruit block.
    pub fn fruit_color(self) -> Option<[u8;3]> {
        match self {
            Self::RedApple=>Some([193,42,38]),
            Self::PinkApple=>Some([244,123,151]),
            Self::YellowApple=>Some([237,198,45]),
            Self::Pear=>Some([170,196,60]),
            Self::Olive=>Some([86,99,64]),
            Self::Peach=>Some([245,145,83]),
            Self::Cherry=>Some([173,28,48]),
            Self::Mango=>Some([238,177,49]),
            Self::Orange=>Some([243,140,31]),
            Self::Lychee=>Some([211,70,72]),
            Self::Breadfruit=>Some([139,181,82]),
            Self::Pomegranate=>Some([192,44,57]),
            _=>None,
        }
    }
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

pub const LEAF_TEXTURE_SIZE: usize = 64;

/// Each leaf cell has an occupancy coordinate but is free to use custom mesh
/// geometry instead of a cube. Legacy files without this option get the new
/// crossed-card default.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LeafGeometry { CrossedCutout, SolidVoxel }

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct LeafRenderSettings {
    pub geometry: LeafGeometry,
    /// Cosmetic vertex sway; never alters occupied block coordinates.
    pub wind_strength: f32,
    pub wind_speed: f32,
}

impl Default for LeafRenderSettings {
    fn default() -> Self {
        Self { geometry: LeafGeometry::CrossedCutout,
            wind_strength: 0.12, wind_speed: 1.25 }
    }
}
impl LeafRenderSettings {
    pub fn validate(&self) -> Result<(), String> {
        if !self.wind_strength.is_finite() || !(0.0..=0.5).contains(&self.wind_strength)
            || !self.wind_speed.is_finite() || !(0.0..=5.0).contains(&self.wind_speed) {
            return Err("Wind strength must be 0..0.5 and speed 0..5".into());
        }
        Ok(())
    }
}

/// Procedural species-specific cutout art for texture-card foliage. This only
/// changes the visible alpha mask; leaves remain editable 1m hexel cells.
fn leaf_segment(u:f32,v:f32,a:[f32;2],b:[f32;2],thickness:f32)->bool {
    let dx=b[0]-a[0];
    let dy=b[1]-a[1];
    let len=(dx*dx+dy*dy).max(1e-6);
    let t=(((u-a[0])*dx+(v-a[1])*dy)/len).clamp(0.0,1.0);
    (u-a[0]-t*dx).powi(2)+(v-a[1]-t*dy).powi(2) <= thickness*thickness
}

fn leaf_oval(u:f32,v:f32,cx:f32,cy:f32,rx:f32,ry:f32)->bool {
    ((u-cx)/rx).powi(2)+((v-cy)/ry).powi(2) <= 1.0
}

/// Pointed blade along a unit direction, with optional toothed birch edges.
fn leaf_blade(u:f32,v:f32,cx:f32,cy:f32,dir:[f32;2],
              length:f32,width:f32,serrated:bool)->bool {
    let x=u-cx;
    let y=v-cy;
    let along=x*dir[0]+y*dir[1];
    let across=-x*dir[1]+y*dir[0];
    if along.abs()>length {return false;}
    let taper=(1.0-(along/length).powi(2)).max(0.0).sqrt();
    let teeth=if serrated {1.0+0.15*(along/length*24.0).sin().abs()} else {1.0};
    across.abs()<=width*taper*teeth
}

fn leaf_pixel(species:PlantPreset,u:f32,v:f32)->(bool,bool) {
    // The twig is drawn darker; the second value is the leafy silhouette.
    let twig=leaf_segment(u,v,[0.50,0.90],[0.50,0.14],0.009);
    let leaves=match species {
        PlantPreset::Acacia => {
            (0..9).any(|i| {
                let y=0.23+i as f32*0.063;
                leaf_segment(u,v,[0.50,y],[0.29,y-0.040],0.010)
                    ||leaf_segment(u,v,[0.50,y],[0.71,y-0.040],0.010)
                    ||leaf_oval(u,v,0.30,y-0.043,0.020,0.026)
                    ||leaf_oval(u,v,0.70,y-0.043,0.020,0.026)
            })
        }
        PlantPreset::Kapok|PlantPreset::Breadfruit => {
            [[0.34,0.35],[0.66,0.51],[0.35,0.72]].iter().any(|p| {
                leaf_oval(u,v,p[0],p[1],0.060,0.074)
                    ||(0..5).any(|i|{
                        let angle=i as f32/5.0*TAU;
                        leaf_blade(u,v,p[0]+angle.cos()*0.056,
                            p[1]+angle.sin()*0.056,
                            [angle.cos(),angle.sin()],0.090,0.033,false)
                    })
            })
        }
        PlantPreset::Purpleheart|PlantPreset::Walnut|PlantPreset::Lychee => {
            (0..6).any(|i| {
                let y=0.23+i as f32*0.108;
                leaf_oval(u,v,0.39,y,0.070,0.037)
                    ||leaf_oval(u,v,0.61,y+0.025,0.070,0.037)
            })
        }
        PlantPreset::BaldCypress|PlantPreset::Larch|
        PlantPreset::Redwood|PlantPreset::MediterraneanCypress => {
            let count=if species==PlantPreset::MediterraneanCypress {18} else {13};
            (0..count).any(|i| {
                let y=0.17+i as f32*0.036;
                let span=if species==PlantPreset::MediterraneanCypress {
                    0.085
                } else if species==PlantPreset::Redwood {0.14} else {0.20};
                leaf_segment(u,v,[0.50,y],[0.50-span,y-0.055],0.010)
                    ||leaf_segment(u,v,[0.50,y],[0.50+span,y-0.055],0.010)
            })
        }
        PlantPreset::GreenBamboo|PlantPreset::BrownBamboo => {
            (0..5).any(|i|{
                let y=0.19+i as f32*0.14;
                leaf_blade(u,v,0.35,y,[-0.85,-0.53],0.15,0.028,false)
                    ||leaf_blade(u,v,0.65,y+0.04,[0.85,-0.53],0.15,0.028,false)
            })
        }
        PlantPreset::Ebony|PlantPreset::Mango|PlantPreset::Orange|
        PlantPreset::RedApple|PlantPreset::PinkApple|PlantPreset::YellowApple|
        PlantPreset::Pear|PlantPreset::Olive|PlantPreset::Peach|
        PlantPreset::Cherry|PlantPreset::Pomegranate => {
            let (rx,ry)=match species {
                PlantPreset::Olive=>(0.031,0.099),
                PlantPreset::Mango=>(0.050,0.124),
                PlantPreset::Pear=>(0.055,0.090),
                PlantPreset::Peach=>(0.044,0.113),
                PlantPreset::Pomegranate=>(0.040,0.063),
                _=>(0.067,0.078),
            };
            (0..5).any(|i| {
                let y=0.26+i as f32*0.128;
                let side=if i%2==0 {-1.0} else {1.0};
                leaf_oval(u,v,0.50+0.13*side,y,rx,ry)
                    ||leaf_oval(u,v,0.50-0.12*side,y+0.035,rx,ry)
            })
        }
        PlantPreset::Oak => {
            // Rounded, lobed broadleaf clusters along a vertical twig.
            [[0.36,0.33],[0.65,0.46],[0.35,0.61],[0.64,0.76]]
                .iter().any(|p| {
                    leaf_oval(u,v,p[0],p[1],0.067,0.104)
                    || leaf_oval(u,v,p[0]-0.052,p[1]-0.01,0.048,0.052)
                    || leaf_oval(u,v,p[0]+0.052,p[1]-0.01,0.048,0.052)
                    || leaf_oval(u,v,p[0],p[1]-0.077,0.038,0.052)
                })
        }
        PlantPreset::Pine => {
            // Fine needles in repeated diagonal sprays, not broadleaf ovals.
            (0..14).any(|i| {
                let y=0.23+i as f32*0.045;
                let reach=0.15+0.05*((i*7%5) as f32/4.0);
                leaf_segment(u,v,[0.50,y],[0.50-reach,y-0.085],0.009)
                    || leaf_segment(u,v,[0.50,y],[0.50+reach,y-0.085],0.009)
            })
        }
        PlantPreset::Birch => {
            // Sparse pointed, lightly toothed leaves on alternate sides.
            (0..5).any(|i| {
                let side=if i%2==0 {-1.0} else {1.0};
                let y=0.28+i as f32*0.125;
                leaf_blade(u,v,0.50+side*0.135,y,[side*0.8,-0.6],
                    0.108,0.053,true)
            })
        }
        PlantPreset::Willow => {
            // Long, narrow, down-swept blades on a hanging twig.
            (0..5).any(|i| {
                let y=0.22+i as f32*0.14;
                leaf_blade(u,v,0.38,y+0.065,[-0.42,0.91],
                    0.145,0.024,false)
                || leaf_blade(u,v,0.62,y+0.085,[0.42,0.91],
                    0.145,0.024,false)
            })
        }
        PlantPreset::Maple|PlantPreset::CrimsonKingMaple => {
            // Five pointed lobes radiating from each palmate leaf centre.
            [[0.36,0.34],[0.65,0.56],[0.37,0.77]]
                .iter().any(|p| {
                    leaf_oval(u,v,p[0],p[1],0.038,0.040)
                    || [[0.0,-1.0],[-0.75,-0.66],[0.75,-0.66],
                        [-0.94,0.34],[0.94,0.34]].iter().any(|dir| {
                        leaf_blade(u,v,p[0]+dir[0]*0.061,p[1]+dir[1]*0.061,
                            *dir,0.076,0.033,false)
                    })
                })
        }
        PlantPreset::Shrub => {
            // Clusters of small rounded leaves on short, branching twigs.
            [[0.31,0.33],[0.42,0.28],[0.57,0.34],[0.70,0.29],
             [0.28,0.48],[0.39,0.54],[0.62,0.49],[0.75,0.56],
             [0.34,0.71],[0.46,0.76],[0.59,0.68],[0.70,0.77]]
                .iter().any(|p| leaf_oval(u,v,p[0],p[1],0.051,0.044))
        }
    };
    let side_twigs=match species {
        PlantPreset::Pine|PlantPreset::BaldCypress|
        PlantPreset::MediterraneanCypress|PlantPreset::Larch|
        PlantPreset::Redwood => false,
        PlantPreset::Shrub|PlantPreset::GreenBamboo|PlantPreset::BrownBamboo => {
            leaf_segment(u,v,[0.50,0.78],[0.29,0.45],0.008)
                ||leaf_segment(u,v,[0.50,0.69],[0.70,0.38],0.008)
        }
        _ => {
            leaf_segment(u,v,[0.50,0.40],[0.36,0.33],0.007)
                ||leaf_segment(u,v,[0.50,0.55],[0.65,0.51],0.007)
                ||leaf_segment(u,v,[0.50,0.72],[0.35,0.70],0.007)
        }
    };
    (twig||side_twigs,leaves)
}

/// Original 64×64 RGBA texture-card artwork generated from species geometry.
/// Binary alpha avoids changing harvestable cells when texture details change.
pub fn leaf_cutout_for_species(species:PlantPreset)->Vec<u8> {
    let mut rgba=Vec::with_capacity(LEAF_TEXTURE_SIZE*LEAF_TEXTURE_SIZE*4);
    for y in 0..LEAF_TEXTURE_SIZE {
        for x in 0..LEAF_TEXTURE_SIZE {
            let u=(x as f32+0.5)/LEAF_TEXTURE_SIZE as f32;
            let v=(y as f32+0.5)/LEAF_TEXTURE_SIZE as f32;
            let (twig,leaves)=leaf_pixel(species,u,v);
            let shade=if leaves {
                0.83+0.14*((x*11+y*7)%17) as f32/16.0
            } else {0.58};
            let value=(shade*255.0) as u8;
            rgba.extend_from_slice(&[value,value,value,
                if leaves||twig {255} else {0}]);
        }
    }
    rgba
}

/// Default artwork for an otherwise unspecialized material is oak broadleaf.
pub fn leaf_cutout_rgba()->Vec<u8> {
    leaf_cutout_for_species(PlantPreset::Oak)
}

/// Grayscale alpha export for OBJ/MTL consumers that ignore PNG alpha.
pub fn leaf_opacity_rgba()->Vec<u8> {
    leaf_cutout_rgba().chunks_exact(4).flat_map(|px| {
        let a=px[3];[a,a,a,255]
    }).collect()
}

/// Preview-only vertex motion, anchored at the bottom of each leaf card.
/// Asset coordinates and static OBJ export are unchanged.
pub fn sway_leaf_vertex(point:[f32;3], cell:[i32;3], weight:f32,
                        time:f32, settings:LeafRenderSettings, size:f32) -> [f32;3] {
    if weight==0.0 || settings.wind_strength==0.0 {return point;}
    let phase=time*settings.wind_speed*3.0
        +cell[0] as f32*0.37+cell[1] as f32*0.71+cell[2] as f32*0.19;
    let shift=(phase.sin()+0.3*(phase*0.6).cos())*settings.wind_strength*size*weight;
    [point[0]+shift,point[1]+shift*0.45,point[2]]
}
/// Game foliage consists only of standard 1m hexagonal-prism cells:
/// one metre across opposite flats, and one metre vertically.
pub const WORLD_HEXEL_SIZE_METRES: f32 = 1.0;

/// Artist-selectable shape variation for standard-grid trees.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum BranchGrowth {
    Classic,
    #[default]
    /// Seeded canopy targets steer branch tips (limited space-colonization inspiration).
    CanopyGuided,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct FoliageProject {
    pub preset: PlantPreset,
    pub seed: u32,
    pub height: f32,
    pub trunk_radius: f32,
    pub branch_count: u32,
    pub branch_length: f32,
    #[serde(default)]
    pub branch_growth: BranchGrowth,
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
    #[serde(default)]
    pub leaf_render: LeafRenderSettings,
    #[serde(default)]
    pub materials: FoliageMaterials,
}

impl Default for FoliageProject {
    fn default() -> Self { Self::from_preset(PlantPreset::Oak) }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Material { Bark, LeafDark, Leaf, LeafLight }

/// Every bark and leaf entry is a logical, harvestable occupied cell;
/// item drops and tree-felling policy remain responsibilities of game state.
impl Material {
    pub fn is_wood(self) -> bool { self == Self::Bark }
    pub fn is_foliage(self) -> bool { !self.is_wood() }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VoxelCell {
    /// Cube: (x,y,z). Hex: (axial q, axial r, z).
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub material: Material,
}

/// Role of a harvestable hexel in its authored tree structure.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TreePart {
    Trunk,
    MajorBranch,
    SmallBranch,
    Foliage,
    AuthoredWood,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TreeCell {
    pub cell: [i32; 3],
    pub part: TreePart,
    pub branch_id: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TreeBranch {
    pub id: u32,
    /// Branch 0 is the trunk; each other branch attaches to a parent.
    pub parent: Option<u32>,
    pub root: [i32; 3],
    pub tip: [i32; 3],
    pub part: TreePart,
}

/// Describes the original growth structure; gameplay occupancy is still
/// represented by actual world hexels, not separate branch entities.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TreeStructure {
    pub origin: [i32; 3],
    /// Bottom-layer wood hexes that require supporting terrain.
    pub root_footprint: Vec<[i32; 2]>,
    pub branches: Vec<TreeBranch>,
    pub cells: Vec<TreeCell>,
}

impl TreeStructure {
    pub fn validate(&self, voxels: &[VoxelCell]) -> Result<(), String> {
        if self.origin != [0, 0, 0] {
            return Err("Tree origin must be the base hexel [0, 0, 0]".into());
        }
        let occupied: BTreeMap<_,_> = voxels.iter().map(|v|
            ([v.x,v.y,v.z],v.material)).collect();
        if occupied.len() != voxels.len() || occupied.len() != self.cells.len() {
            return Err("Tree metadata must match occupied hexels".into());
        }
        let by_id:BTreeMap<_,_> = self.branches.iter().map(|b|(b.id,b)).collect();
        if by_id.len() != self.branches.len() || by_id.get(&0)
            .is_none_or(|b|b.parent.is_some()||b.part!=TreePart::Trunk) {
            return Err("Tree must contain one rooted trunk".into());
        }
        for branch in &self.branches {
            if branch.id!=0 && branch.parent
                .is_none_or(|p|p>=branch.id||!by_id.contains_key(&p)) {
                return Err("Branch parent must precede its child".into());
            }
        }
        let mut seen=BTreeSet::new();
        for part in &self.cells {
            let Some(material)=occupied.get(&part.cell) else {
                return Err("Tree role refers to an empty hexel".into());
            };
            if !seen.insert(part.cell) {return Err("Duplicate tree cell role".into());}
            if material.is_wood() == (part.part==TreePart::Foliage) {
                return Err("Tree role disagrees with wood/leaf material".into());
            }
            if let Some(id)=part.branch_id {
                let branch=by_id.get(&id).ok_or("Missing owner branch")?;
                if part.part!=TreePart::Foliage && part.part!=branch.part {
                    return Err("Wood role disagrees with its branch".into());
                }
            }
        }
        let base:BTreeSet<_>=voxels.iter().filter(|v|v.material.is_wood()&&v.z==0)
            .map(|v|[v.x,v.y]).collect();
        let footprint:BTreeSet<_>=self.root_footprint.iter().copied().collect();
        if base.is_empty() || base!=footprint || footprint.len()!=self.root_footprint.len() {
            return Err("Root footprint must match bottom-layer wood".into());
        }
        Ok(())
    }

    /// Pure placement eligibility hook; world generation must supply support
    /// and clearance lookups and handle actual stamping across chunks.
    pub fn fits_site(&self, mut supported:impl FnMut([i32;2])->bool,
                     mut clear:impl FnMut([i32;3])->bool)->bool {
        self.root_footprint.iter().copied().all(&mut supported)
            && self.cells.iter().all(|part|clear(part.cell))
    }
}

#[derive(Clone, Debug)]
pub struct Triangle {
    pub points: [[f32; 3]; 3],
    pub material: Material,
    pub uv: [[f32; 2]; 3],
    pub cutout: bool,
    pub wind_weight: [f32; 3],
    /// Occupied grid cell this visible face belongs to.
    pub cell: [i32; 3],
    /// Grid-space outward neighbor. Clicking the face adds a cell here.
    pub outward: [i32; 3],
}

#[derive(Clone, Debug)]
pub struct FoliageMesh {
    pub voxels: Vec<VoxelCell>,
    pub triangles: Vec<Triangle>,
    pub shape: VoxelShape,
    pub voxel_size: f32,
    pub height: f32,
    pub leaf_render: LeafRenderSettings,
    /// None for non-tree assets assembled in the general-purpose editor.
    pub structure: Option<TreeStructure>,
}

impl FoliageProject {
    pub fn from_preset(preset: PlantPreset) -> Self {
        // These proportions are tuned for the game's fixed 1 m hexels, not
        // the former sub-metre foliage grid. A thinner trunk still occupies
        // at least one real wood cell.
        // Species-specific playable silhouettes on the fixed 1 m hexel grid.
        // These are authored parameters; seed controls individual variation.
        let (height,radius,branches,branch_length,gravity,density,leaf_scale,bark,leaves)=
            match preset {
                PlantPreset::Acacia => (12.0,0.63,32,0.62,0.02,0.66,0.65,[118,89,58],[105,142,61]),
                PlantPreset::Birch => (15.0,0.46,30,0.35,0.02,0.67,0.59,[198,193,172],[96,158,54]),
                PlantPreset::Ebony => (16.0,0.72,40,0.49,0.04,0.88,0.72,[51,47,44],[54,113,66]),
                PlantPreset::Kapok => (24.0,1.16,46,0.55,0.04,0.85,0.88,[133,106,77],[67,143,72]),
                PlantPreset::Maple => (13.0,0.69,40,0.58,0.10,0.90,0.85,[97,70,51],[112,163,52]),
                PlantPreset::CrimsonKingMaple => (13.5,0.70,40,0.57,0.08,0.90,0.84,[94,69,62],[105,38,73]),
                PlantPreset::Oak => (13.0,0.72,38,0.55,0.08,0.82,0.82,[91,67,45],[67,135,52]),
                PlantPreset::Purpleheart => (19.0,0.88,42,0.54,0.05,0.82,0.82,[112,61,96],[57,124,65]),
                PlantPreset::Walnut => (15.0,0.77,42,0.57,0.11,0.80,0.77,[94,76,58],[79,125,61]),
                PlantPreset::Willow => (12.0,0.72,38,0.61,0.80,0.93,0.70,[91,86,61],[91,156,65]),
                PlantPreset::BaldCypress => (19.0,0.75,42,0.39,0.05,0.74,0.47,[119,88,64],[90,128,70]),
                PlantPreset::MediterraneanCypress => (17.0,0.53,42,0.19,0.02,0.91,0.37,[88,71,52],[39,86,58]),
                PlantPreset::Larch => (17.0,0.55,42,0.44,0.03,0.69,0.42,[112,90,61],[126,157,70]),
                PlantPreset::Pine => (18.0,0.53,48,0.46,0.06,0.78,0.44,[89,71,52],[40,94,58]),
                PlantPreset::Redwood => (27.0,1.28,66,0.36,0.04,0.76,0.52,[121,69,54],[47,91,67]),
                PlantPreset::RedApple => (8.0,0.40,26,0.58,0.10,0.89,0.67,[102,76,57],[78,144,63]),
                PlantPreset::PinkApple => (8.3,0.42,27,0.57,0.10,0.88,0.66,[104,78,58],[88,150,66]),
                PlantPreset::YellowApple => (8.1,0.41,26,0.60,0.12,0.89,0.66,[105,77,57],[107,156,68]),
                PlantPreset::Pear => (9.5,0.44,27,0.44,0.05,0.82,0.63,[114,84,62],[95,154,65]),
                PlantPreset::Olive => (7.5,0.51,34,0.60,0.14,0.80,0.57,[112,106,84],[106,131,104]),
                PlantPreset::Peach => (8.0,0.37,27,0.56,0.06,0.78,0.63,[107,73,65],[91,149,77]),
                PlantPreset::Cherry => (10.0,0.47,32,0.54,0.06,0.88,0.70,[116,69,61],[81,146,71]),
                PlantPreset::Mango => (13.0,0.76,40,0.59,0.07,0.92,0.80,[108,77,54],[57,122,60]),
                PlantPreset::Orange => (9.0,0.51,34,0.57,0.06,0.90,0.70,[112,81,57],[49,112,55]),
                PlantPreset::Lychee => (11.0,0.62,36,0.58,0.04,0.86,0.72,[103,75,58],[65,131,65]),
                PlantPreset::Breadfruit => (16.0,0.79,40,0.56,0.08,0.85,0.85,[113,80,59],[79,149,71]),
                PlantPreset::Pomegranate => (6.5,0.36,30,0.62,0.11,0.78,0.57,[100,83,57],[67,139,65]),
                PlantPreset::GreenBamboo => (11.0,0.23,15,0.20,0.03,0.72,0.43,[74,151,64],[86,159,63]),
                PlantPreset::BrownBamboo => (10.0,0.26,15,0.22,0.05,0.66,0.40,[152,121,69],[113,133,65]),
                PlantPreset::Shrub => (3.5,0.34,20,0.65,0.13,0.86,0.46,[91,70,48],[67,142,51]),
            };
        let (carve_bottom,carve_top,phototropism,leaf_angle,leaf_variation,branch_growth)=
            match preset {
                PlantPreset::Acacia=>(0.56,0.03,0.22,0.09,0.33,BranchGrowth::CanopyGuided),
                PlantPreset::Birch=>(0.44,0.08,0.28,0.09,0.34,BranchGrowth::CanopyGuided),
                PlantPreset::Ebony=>(0.36,0.06,0.14,0.13,0.24,BranchGrowth::CanopyGuided),
                PlantPreset::Kapok=>(0.62,0.04,0.23,0.14,0.25,BranchGrowth::CanopyGuided),
                PlantPreset::Maple=>(0.33,0.06,0.19,0.15,0.26,BranchGrowth::CanopyGuided),
                PlantPreset::CrimsonKingMaple=>(0.33,0.06,0.19,0.14,0.25,BranchGrowth::CanopyGuided),
                PlantPreset::Oak=>(0.36,0.06,0.20,0.12,0.28,BranchGrowth::CanopyGuided),
                PlantPreset::Purpleheart=>(0.43,0.04,0.18,0.12,0.27,BranchGrowth::CanopyGuided),
                PlantPreset::Walnut=>(0.36,0.05,0.16,0.16,0.31,BranchGrowth::CanopyGuided),
                PlantPreset::Willow=>(0.36,0.05,0.02,-0.25,0.24,BranchGrowth::Classic),
                PlantPreset::BaldCypress=>(0.08,0.04,0.10,0.05,0.22,BranchGrowth::Classic),
                PlantPreset::MediterraneanCypress=>(0.04,0.03,0.13,0.04,0.19,BranchGrowth::Classic),
                PlantPreset::Larch=>(0.09,0.05,0.13,0.06,0.25,BranchGrowth::Classic),
                PlantPreset::Pine=>(0.10,0.03,0.09,0.05,0.20,BranchGrowth::Classic),
                PlantPreset::Redwood=>(0.06,0.04,0.11,0.05,0.19,BranchGrowth::Classic),
                PlantPreset::RedApple|PlantPreset::PinkApple|PlantPreset::YellowApple=>
                    (0.22,0.07,0.16,0.10,0.28,BranchGrowth::CanopyGuided),
                PlantPreset::Pear=>(0.32,0.04,0.24,0.10,0.28,BranchGrowth::CanopyGuided),
                PlantPreset::Olive=>(0.17,0.07,0.11,0.12,0.36,BranchGrowth::Classic),
                PlantPreset::Peach=>(0.19,0.08,0.18,0.13,0.27,BranchGrowth::CanopyGuided),
                PlantPreset::Cherry=>(0.27,0.05,0.20,0.12,0.30,BranchGrowth::CanopyGuided),
                PlantPreset::Mango=>(0.32,0.04,0.15,0.12,0.23,BranchGrowth::CanopyGuided),
                PlantPreset::Orange=>(0.21,0.06,0.12,0.11,0.27,BranchGrowth::CanopyGuided),
                PlantPreset::Lychee=>(0.30,0.05,0.16,0.11,0.28,BranchGrowth::CanopyGuided),
                PlantPreset::Breadfruit=>(0.42,0.06,0.21,0.12,0.28,BranchGrowth::CanopyGuided),
                PlantPreset::Pomegranate=>(0.06,0.07,0.12,0.13,0.30,BranchGrowth::Classic),
                PlantPreset::GreenBamboo|PlantPreset::BrownBamboo=>
                    (0.53,0.04,0.10,0.08,0.25,BranchGrowth::Classic),
                PlantPreset::Shrub=>(0.05,0.08,0.13,0.18,0.40,BranchGrowth::Classic),
            };
        Self {
            preset, seed: 42, height, trunk_radius: radius, branch_count: branches,
            branch_length, branch_growth, carve_bottom, carve_top,
            gravity, phototropism, slope: 0.0, overall_scale: 1.0, prune: 0.0,
            leaf_density: density, leaf_scale, leaf_angle, leaf_variation,
            bark_color: bark, leaf_color: leaves,
            leaf_render: LeafRenderSettings::default(),
            materials: FoliageMaterials::for_species(preset),
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        self.leaf_render.validate()?;
        if self.leaf_render.geometry != LeafGeometry::CrossedCutout {
            return Err("Game foliage requires logical leaf cells with textured cutout cards".into());
        }
        self.materials.validate()?;
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
        if self.height * self.overall_scale / WORLD_HEXEL_SIZE_METRES > 96.0 {
            return Err("Tree exceeds 96 voxel levels: reduce height or overall scale".into());
        }
        if self.branch_length * self.height * self.overall_scale / WORLD_HEXEL_SIZE_METRES > 70.0 {
            return Err("Branch reach exceeds 70 cells: reduce branch length or overall scale".into());
        }
        if self.leaf_scale * self.overall_scale * 2.3 / WORLD_HEXEL_SIZE_METRES > 12.0 {
            return Err("Leaf clusters exceed 12 cells in radius: reduce leaf scale or overall scale".into());
        }
        if self.trunk_radius * self.overall_scale / WORLD_HEXEL_SIZE_METRES > 8.0 {
            return Err("Trunk exceeds 8 cells in radius: reduce trunk width or overall scale".into());
        }
        Ok(())
    }

    /// Radial crown envelope as a function of normalized attachment height.
    /// These multipliers affect real wood and leaf hexel occupancy.
    fn crown_reach(&self, t: f32) -> f32 {
        let crown_t = ((t-self.carve_bottom) /
            (1.0-self.carve_bottom-self.carve_top)).clamp(0.0,1.0);
        let center = 2.0*crown_t-1.0;
        match self.preset {
            PlantPreset::Acacia => 0.50+0.80*crown_t,
            PlantPreset::Kapok => 0.54+0.72*crown_t,
            PlantPreset::Oak|PlantPreset::Walnut|PlantPreset::Ebony =>
                0.82+0.35*(1.0-center*center),
            PlantPreset::Maple|PlantPreset::CrimsonKingMaple =>
                0.96+0.35*(1.0-center*center),
            PlantPreset::Pine|PlantPreset::BaldCypress|PlantPreset::Larch|
            PlantPreset::Redwood => 0.20+1.15*(1.0-crown_t),
            PlantPreset::MediterraneanCypress => 0.20+0.24*(1.0-crown_t),
            PlantPreset::Birch|PlantPreset::Pear => 0.66+0.24*(1.0-crown_t),
            PlantPreset::Willow => 1.22-0.16*crown_t,
            PlantPreset::Purpleheart|PlantPreset::Mango|PlantPreset::Breadfruit =>
                0.80+0.46*(1.0-center*center),
            PlantPreset::RedApple|PlantPreset::PinkApple|PlantPreset::YellowApple|
            PlantPreset::Peach|PlantPreset::Cherry|PlantPreset::Orange|
            PlantPreset::Lychee => 0.88+0.31*(1.0-center*center),
            PlantPreset::Olive|PlantPreset::Pomegranate|PlantPreset::Shrub =>
                1.14-0.18*crown_t,
            PlantPreset::GreenBamboo|PlantPreset::BrownBamboo =>
                0.30+0.12*crown_t,
        }
    }

    /// All foliage is directly authored on the game's 1m Q/R/Z hexel grid.
    /// Each wood or leaf entry is a real removable world-cell definition.
    pub fn generate(&self) -> Result<FoliageMesh, String> {
        self.validate()?;
        let mut cells: BTreeMap<[i32; 3], Material> = BTreeMap::new();
        let mut ownership:BTreeMap<[i32;3],(TreePart,Option<u32>)>=BTreeMap::new();
        let mut branches=Vec::new();
        let s = WORLD_HEXEL_SIZE_METRES;
        let shape = VoxelShape::HexPrism;
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
            paint_sphere(&mut cells, shape, s, p, radius, Material::Bark);
        }
        for cell in cells.keys() { ownership.insert(*cell,(TreePart::Trunk,Some(0))); }
        branches.push(TreeBranch {id:0,parent:None,root:[0,0,0],
            tip:grid_key(shape,s,trunk(1.0)),part:TreePart::Trunk});
        let mut targets = Vec::new();
        if self.branch_growth == BranchGrowth::CanopyGuided {
            let crown_radius = self.branch_length * h * 0.75;
            for k in 0..(self.branch_count * 3) {
                let angle = TAU * hash01(self.seed, k, 91);
                let distance = crown_radius * hash01(self.seed, k, 92).sqrt();
                targets.push([
                    angle.cos() * distance,
                    angle.sin() * distance,
                    h * (0.36 + 0.60 * hash01(self.seed, k, 93)),
                ]);
            }
        }
        for i in 0..self.branch_count {
            let rand = |c| hash01(self.seed, i, c);
            if rand(0) < self.prune { continue; }
            let (height_fraction,angle) = if self.preset.is_conifer() {
                // Six-way whorls staggered between layers instead of a
                // broadleaf spiral. The conifer envelope tapers upward.
                let layer=i/6;
                let layers=self.branch_count.div_ceil(6);
                let fraction=(layer as f32+0.48+0.12*(rand(1)-0.5))
                    /layers as f32;
                let azimuth=(i%6) as f32/6.0+layer as f32*0.083
                    +0.035*(rand(2)-0.5);
                (fraction,TAU*azimuth)
            } else {
                ((i as f32+0.5+0.25*(rand(1)-0.5))
                    /self.branch_count as f32,
                    TAU*(i as f32*0.6180339+rand(2)*0.2))
            };
            let t=self.carve_bottom+
                (1.0-self.carve_bottom-self.carve_top)*height_fraction;
            let base=trunk(t);
            let reach=self.branch_length*h*(0.38+0.55*rand(3))
                *self.crown_reach(t);
            let base_rise=match self.preset {
                PlantPreset::Pine|PlantPreset::Larch|PlantPreset::BaldCypress|
                PlantPreset::Redwood|PlantPreset::MediterraneanCypress=>0.12,
                PlantPreset::Willow=>-0.12,
                PlantPreset::Birch|PlantPreset::Pear=>0.43,
                PlantPreset::GreenBamboo|PlantPreset::BrownBamboo=>0.44,
                PlantPreset::Acacia=>0.08,
                PlantPreset::Shrub|PlantPreset::Pomegranate=>0.18,
                PlantPreset::Maple|PlantPreset::CrimsonKingMaple=>0.15,
                _=>0.23,
            };
            let rise = reach * (base_rise + 0.65 * self.phototropism - 0.85 * self.gravity);
            let mut tip = [
                base[0] + reach * angle.cos(),
                base[1] + reach * angle.sin(),
                (base[2] + rise).max(s * 0.5),
            ];
            if !targets.is_empty() {
                // One-pass canopy guidance, not a complete iterative
                // space-colonization implementation.
                let (chosen, _) = targets.iter().enumerate()
                    .min_by(|(_, a), (_, b)| {
                        let distance = |target: &&[f32; 3]| {
                            let d = sub3(**target, tip);
                            d[0]*d[0] + d[1]*d[1] + d[2]*d[2]
                        };
                        distance(a).total_cmp(&distance(b))
                    }).unwrap();
                // Don't erase the species' branch habit when guiding crowns.
                tip = lerp3(tip, targets.swap_remove(chosen), 0.55);
                tip[2] = tip[2].max(s * 0.5);
            }
            let mut mid=lerp3(base,tip,0.56);
            if self.preset==PlantPreset::Willow {
                // Outward-arching limb followed by hanging tips.
                mid[2]=(mid[2]+0.24*reach).min(h);
            }
            let thickness = (self.trunk_radius * self.overall_scale *
                (0.28 + 0.25 * (1.0 - t))).max(s * 0.17);
            let major_id=2*i+1;
            let fork_id=major_id+1;
            branches.push(TreeBranch {id:major_id,parent:Some(0),
                root:grid_key(shape,s,base),tip:grid_key(shape,s,tip),
                part:TreePart::MajorBranch});
            let mut main_cells=BTreeMap::new();
            paint_line(&mut main_cells,shape,s,base,mid,thickness,Material::Bark);
            paint_line(&mut main_cells,shape,s,mid,tip,s*0.18,Material::Bark);
            for (cell,material) in main_cells {
                let previous=cells.insert(cell,material);
                if previous.is_none() || previous.is_some_and(|m|m.is_foliage()) {
                    ownership.insert(cell,(TreePart::MajorBranch,Some(major_id)));
                }
            }
            let fork_angle = angle + (rand(4) - 0.5) * 1.6;
            let fork_reach=reach*(0.28+0.2*rand(5));
            let fork_rise=match self.preset {
                PlantPreset::Willow=>-0.82,
                PlantPreset::Birch|PlantPreset::Pear=>0.45,
                PlantPreset::GreenBamboo|PlantPreset::BrownBamboo=>0.34,
                PlantPreset::Pine|PlantPreset::Redwood|PlantPreset::Larch|
                PlantPreset::BaldCypress|PlantPreset::MediterraneanCypress=>0.10,
                PlantPreset::Shrub|PlantPreset::Pomegranate=>0.12,
                _=>0.32+self.phototropism*0.26-self.gravity*0.65,
            };
            let fork=[
                mid[0]+fork_reach*fork_angle.cos(),
                mid[1]+fork_reach*fork_angle.sin(),
                (mid[2]+fork_reach*fork_rise).max(s*0.5),
            ];
            branches.push(TreeBranch {id:fork_id,parent:Some(major_id),
                root:grid_key(shape,s,mid),tip:grid_key(shape,s,fork),
                part:TreePart::SmallBranch});
            let mut fork_cells=BTreeMap::new();
            paint_line(&mut fork_cells,shape,s,mid,fork,s*0.17,Material::Bark);
            for (cell,material) in fork_cells {
                let previous=cells.insert(cell,material);
                if previous.is_none() || previous.is_some_and(|m|m.is_foliage()) {
                    ownership.insert(cell,(TreePart::SmallBranch,Some(fork_id)));
                }
            }
            if self.leaf_density == 0.0 { continue; }
            // Foliage clusters are attached to their owning branch tips.
            // Only card visibility changes; cells stay harvestable.
            let leaf_tips = [tip, fork];
            for (cluster, location) in leaf_tips.into_iter().enumerate() {
                let r = ((self.leaf_scale * self.overall_scale * 2.3)
                    * (0.75 + 0.5 * rand(6 + cluster as u32))).max(s * 0.82);
                let rz=match self.preset {
                    PlantPreset::Pine|PlantPreset::Larch|PlantPreset::Redwood|
                    PlantPreset::BaldCypress|PlantPreset::MediterraneanCypress=>r*1.75,
                    PlantPreset::Willow=>r*1.65,
                    PlantPreset::Birch|PlantPreset::Pear=>r*1.12,
                    PlantPreset::GreenBamboo|PlantPreset::BrownBamboo=>r*1.25,
                    PlantPreset::Shrub|PlantPreset::Pomegranate=>r*0.65,
                    _=>r*0.85,
                };
                let offset_z = match self.preset {
                    PlantPreset::Willow => -0.58 * rz,
                    PlantPreset::Pine|PlantPreset::Larch|PlantPreset::Redwood|
                    PlantPreset::BaldCypress|PlantPreset::MediterraneanCypress =>
                        0.18 * rz,
                    _ => 0.16 * rz,
                };
                let center = [location[0], location[1],
                    (location[2] + offset_z).max(s * 0.5)];
                let mut cluster_cells=BTreeMap::new();
                paint_leaf_cluster(&mut cluster_cells,shape,s,center,r,rz,
                    self.leaf_density,self.leaf_variation,self.leaf_angle,
                    self.seed,i*2+cluster as u32);
                let owner=if cluster==0 {major_id} else {fork_id};
                for (cell,material) in cluster_cells {
                    if let std::collections::btree_map::Entry::Vacant(entry)=cells.entry(cell) {
                        entry.insert(material);
                        ownership.insert(cell,(TreePart::Foliage,Some(owner)));
                    }
                }
            }
            if cells.len() > 40_000 {
                return Err("Tree exceeds 40,000 game hexels: reduce size, branching or canopy density".into());
            }
        }
        if cells.len() > 40_000 {
            return Err("Tree exceeds 40,000 game hexels: reduce size, branching or canopy density".into());
        }
        retain_supported_foliage(&mut cells);
        ownership.retain(|cell,_|cells.contains_key(cell));
        let footprint:BTreeSet<_>=cells.iter()
            .filter(|(pos,material)|pos[2]==0 && material.is_wood())
            .map(|(pos,_)|[pos[0],pos[1]]).collect();
        let structure=TreeStructure {
            origin:[0,0,0],root_footprint:footprint.into_iter().collect(),
            branches,
            cells:ownership.into_iter().map(|(cell,(part,branch_id))|
                TreeCell {cell,part,branch_id}).collect(),
        };
        let voxels:Vec<VoxelCell> = cells.iter().map(|(pos, material)| VoxelCell {
            x: pos[0], y: pos[1], z: pos[2], material: *material,
        }).collect();
        structure.validate(&voxels)?;
        let triangles = surface_triangles(&cells, shape, s, self.leaf_render);
        Ok(FoliageMesh { voxels, triangles, shape,
            voxel_size: s, height: h, leaf_render: self.leaf_render,
            structure:Some(structure) })
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
    let mut previous = None;
    for i in 0..=steps {
        let point = lerp3(a, b, i as f32 / steps as f32);
        // Diagonal hex-and-height jumps do not share a full face. Bridge
        // consecutive wood positions with face-adjacent, harvestable cells.
        if shape == VoxelShape::HexPrism && material.is_wood() {
            let key = grid_key(shape, s, point);
            if let Some(last) = previous { bridge_hex_wood(cells, last, key); }
            previous = Some(key);
        }
        paint_sphere(cells, shape, s, point, radius, material);
    }
}

fn bridge_hex_wood(cells: &mut BTreeMap<[i32; 3], Material>,
                   mut from: [i32; 3], to: [i32; 3]) {
    const DIRECTIONS: [[i32; 2]; 6] =
        [[0, 1], [-1, 1], [-1, 0], [0, -1], [1, -1], [1, 0]];
    while from != to {
        if from[2] != to[2] {
            from[2] += (to[2] - from[2]).signum();
        } else {
            let distance = |q: i32, r: i32| {
                let dq = (q - to[0]).abs();
                let dr = (r - to[1]).abs();
                let ds = ((q + r) - (to[0] + to[1])).abs();
                (dq + dr + ds) / 2
            };
            let current = distance(from[0], from[1]);
            let step = DIRECTIONS.iter()
                .find(|d| distance(from[0] + d[0], from[1] + d[1]) < current)
                .expect("one hex neighbour reduces axial distance");
            from[0] += step[0];
            from[1] += step[1];
        }
        cells.insert(from, Material::Bark);
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

/// Remove detached leaf islands after stochastic canopy generation.
/// All retained leaf components touch at least one wood cell by a hex face.
fn retain_supported_foliage(cells:&mut BTreeMap<[i32;3],Material>) {
    const DIRECTIONS:[[i32;3];8]=[
        [0,1,0],[-1,1,0],[-1,0,0],[0,-1,0],[1,-1,0],[1,0,0],
        [0,0,1],[0,0,-1],
    ];
    let mut reachable=BTreeSet::new();
    let mut queue=VecDeque::new();
    for (pos,material) in cells.iter() {
        if !material.is_foliage() {continue;}
        if DIRECTIONS.iter().any(|d|cells.get(&[
            pos[0]+d[0],pos[1]+d[1],pos[2]+d[2]])
            .is_some_and(|m|m.is_wood())) {
            reachable.insert(*pos);
            queue.push_back(*pos);
        }
    }
    while let Some(pos)=queue.pop_front() {
        for direction in DIRECTIONS {
            let other=[pos[0]+direction[0],pos[1]+direction[1],pos[2]+direction[2]];
            if !reachable.contains(&other) &&
                cells.get(&other).is_some_and(|m|m.is_foliage()) {
                reachable.insert(other);
                queue.push_back(other);
            }
        }
    }
    cells.retain(|pos,material|material.is_wood() || reachable.contains(pos));
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
                 c: [f32; 3], material: Material, cell: [i32; 3],
                 outward: [i32; 3], uv:[[f32;2];3]) {
    out.push(Triangle { points: [a, b, c], material, cell, outward,
        uv, cutout: false, wind_weight: [0.0; 3] });
}
fn add_leaf_cards(triangles:&mut Vec<Triangle>, pos:[i32;3],
                  material:Material, shape:VoxelShape, size:f32) {
    let c=grid_position(shape,size,pos);
    let id=(pos[0] as u32).wrapping_mul(0x9e3779b9)
        ^(pos[1] as u32).wrapping_mul(0x85ebca6b)
        ^(pos[2] as u32).wrapping_mul(0xc2b2ae35);
    let quarter=(id.wrapping_mul(0x27d4eb2d)>>30) as f32;
    let rotation=quarter*PI*0.5;
    let tilt=((id>>8)&31) as f32/31.0*0.16-0.08;
    let width=size*(0.43+((id>>13)&7) as f32/100.0);
    let z0=c[2]-size*0.44;
    let z1=c[2]+size*0.47;
    for plane in 0..2 {
        let angle=rotation+PI*0.25+plane as f32*PI*0.5;
        let d=[angle.cos()*width,angle.sin()*width];
        let drift=size*tilt*(if plane==0 {1.0} else {-1.0});
        let quad=[
            [c[0]-d[0],c[1]-d[1],z0],
            [c[0]+d[0],c[1]+d[1],z0],
            [c[0]+d[0]+drift,c[1]+d[1]-drift,z1],
            [c[0]-d[0]+drift,c[1]-d[1]-drift,z1],
        ];
        let uv=[[0.0,1.0],[1.0,1.0],[1.0,0.0],[0.0,0.0]];
        let weights=[0.0,0.0,1.0,1.0];
        // Both sides: OBJ and preview work with backface culling enabled.
        for idx in [[0,1,2],[0,2,3],[2,1,0],[3,2,0]] {
            triangles.push(Triangle {
                points:idx.map(|i|quad[i]),
                material, uv:idx.map(|i|uv[i]),
                cutout:true, wind_weight:idx.map(|i|weights[i]),
                cell:pos, outward:[0,0,1],
            });
        }
    }
}

fn surface_triangles(
    cells: &BTreeMap<[i32; 3], Material>, shape: VoxelShape, s: f32,
    render: LeafRenderSettings,
) -> Vec<Triangle> {
    let mut triangles = Vec::new();
    for (cell, material) in cells {
        if *material != Material::Bark && render.geometry == LeafGeometry::CrossedCutout {
            // Solid surrounding blocks completely conceal a leaf. Leaf neighbors
            // are not opaque and must not cause the cards to disappear.
            let surrounded=(0..shape.sides() as usize).all(|face| {
                let d=neighbor(shape,face);
                cells.get(&[cell[0]+d[0],cell[1]+d[1],cell[2]])==Some(&Material::Bark)
            }) && [-1,1].iter().all(|dz|
                cells.get(&[cell[0],cell[1],cell[2]+dz])==Some(&Material::Bark));
            if !surrounded { add_leaf_cards(&mut triangles,*cell,*material,shape,s); }
            continue;
        }
        let c = grid_position(shape, s, *cell);
        let bottom = polygon(shape, c, s, -1.0);
        let top = polygon(shape, c, s, 1.0);
        let n = bottom.len();
        for face in 0..n {
            let d = neighbor(shape, face);
            let adjacent = [cell[0] + d[0], cell[1] + d[1], cell[2]];
            if cells.get(&adjacent)==Some(&Material::Bark) ||
                (render.geometry==LeafGeometry::SolidVoxel && cells.contains_key(&adjacent)) { continue; }
            let next = (face + 1) % n;
            // Footprints are CCW; side quad normal points outward.
            push_triangle(&mut triangles, bottom[face], bottom[next],
                top[next], *material, *cell, d,
                [[0.0,1.0],[1.0,1.0],[1.0,0.0]]);
            push_triangle(&mut triangles, bottom[face], top[next],
                top[face], *material, *cell, d,
                [[0.0,1.0],[1.0,0.0],[0.0,0.0]]);
        }
        if cells.get(&[cell[0],cell[1],cell[2]+1])!=Some(&Material::Bark) &&
            (render.geometry!=LeafGeometry::SolidVoxel ||
             !cells.contains_key(&[cell[0],cell[1],cell[2]+1])) {
            for i in 1..n-1 {
                push_triangle(&mut triangles, top[0], top[i], top[i+1],
                    *material, *cell, [0, 0, 1],[
                        [0.5,0.5],
                        [(top[i][0]-c[0])/s+0.5,(top[i][1]-c[1])/s+0.5],
                        [(top[i+1][0]-c[0])/s+0.5,(top[i+1][1]-c[1])/s+0.5],
                    ]);
            }
        }
        if cells.get(&[cell[0],cell[1],cell[2]-1])!=Some(&Material::Bark) &&
            (render.geometry!=LeafGeometry::SolidVoxel ||
             !cells.contains_key(&[cell[0],cell[1],cell[2]-1])) {
            for i in 1..n-1 {
                push_triangle(&mut triangles, bottom[0], bottom[i+1],
                    bottom[i], *material, *cell, [0, 0, -1],[
                        [0.5,0.5],
                        [(bottom[i+1][0]-c[0])/s+0.5,(bottom[i+1][1]-c[1])/s+0.5],
                        [(bottom[i][0]-c[0])/s+0.5,(bottom[i][1]-c[1])/s+0.5],
                    ]);
            }
        }
    }
    triangles
}

impl FoliageMesh {
    /// Rebuild a render/export surface from individually editable occupied cells.
    /// This is shared by procedural foliage and the general-purpose asset editor.
    pub fn from_voxels(shape: VoxelShape, voxel_size: f32,
                       voxels: Vec<VoxelCell>) -> Result<Self, String> {
        Self::from_voxels_with_render(shape,voxel_size,voxels,LeafRenderSettings::default())
    }
    pub fn from_voxels_with_render(shape: VoxelShape, voxel_size: f32,
                       voxels: Vec<VoxelCell>, render:LeafRenderSettings) -> Result<Self, String> {
        render.validate()?;
        if !voxel_size.is_finite() || !(0.1..=1.5).contains(&voxel_size) {
            return Err("Voxel size must be between 0.1 and 1.5".into());
        }
        if voxels.len() > 40_000 { return Err("Asset exceeds 40,000 voxels".into()); }
        let mut occupied = BTreeMap::new();
        for v in &voxels {
            if [v.x, v.y, v.z].iter().any(|p| !(-128..=128).contains(p)) {
                return Err("Voxel coordinates must be between -128 and 128".into());
            }
            if occupied.insert([v.x, v.y, v.z], v.material).is_some() {
                return Err("Two voxels occupy the same cell".into());
            }
        }
        let height = voxels.iter().map(|v| v.z).max().unwrap_or(0) as f32 * voxel_size;
        let triangles = surface_triangles(&occupied, shape, voxel_size, render);
        Ok(Self { voxels, triangles, shape, voxel_size, height, leaf_render:render,
            structure:None })
    }

    pub fn triangle_count(&self) -> usize { self.triangles.len() }
    pub fn voxel_count(&self) -> usize { self.voxels.len() }

    /// Explicit per-voxel representation: the fundamental export.
    /// OBJ is a view of these blocks for traditional mesh software.
    pub fn voxel_json(&self, project: &FoliageProject) -> Result<String, String> {
        if self.shape != VoxelShape::HexPrism || self.voxel_size != WORLD_HEXEL_SIZE_METRES ||
            self.leaf_render.geometry != LeafGeometry::CrossedCutout {
            return Err("Game tree export requires 1m hexels and card foliage".into());
        }
        let grid = "axial q/r hex grid plus integer z layer";
        serde_json::to_string_pretty(&serde_json::json!({
            "format": "hexel-game-tree-v1",
            "cell_dimensions_m": {"across_flats": 1.0, "height": 1.0},
            "shape": self.shape,
            "voxel_size": self.voxel_size,
            "coordinate_system": grid,
            "height": self.height,
            "voxels": self.voxels,
            "leaf_render": self.leaf_render,
            "settings": project,
            "structure": self.structure,
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
        for face in &self.triangles {
            for uv in face.uv {
                output.push_str(&format!("vt {:.6} {:.6}\n",uv[0],1.0-uv[1]));
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
            output.push_str(&format!("f {p}/{p} {}/{} {}/{}\n", p+1,p+1,p+2,p+2));
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
        let textured = (name=="bark" && project.materials.bark.is_some())
            || (name!="bark" && project.leaf_render.geometry==LeafGeometry::CrossedCutout);
        let kd=if textured {[1.0;3]} else {
            [c[0] as f32/255.0,c[1] as f32/255.0,c[2] as f32/255.0]
        };
        text.push_str(&format!(
            "newmtl {name}\nKd {:.4} {:.4} {:.4}\nKa 0 0 0\n",
            kd[0],kd[1],kd[2]
        ));
        if name=="bark" && project.materials.bark.is_some() {
            text.push_str("map_Kd bark-albedo.png\n");
        } else if name!="bark" && project.leaf_render.geometry == LeafGeometry::CrossedCutout {
            text.push_str("map_Kd foliage-leaves.png\nmap_d foliage-leaves-alpha.png\n");
        }
        text.push('\n');
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_species_generate_1m_hexel_wood_and_card_foliage() {
        for preset in PlantPreset::ALL {
            let tree = FoliageProject::from_preset(preset);
            let mesh = tree.generate().unwrap();
            assert_eq!(mesh.shape, VoxelShape::HexPrism, "{preset:?}");
            assert_eq!(mesh.voxel_size, WORLD_HEXEL_SIZE_METRES);
            assert_eq!(mesh.leaf_render.geometry, LeafGeometry::CrossedCutout);
            assert!(mesh.voxel_count() > 4, "{preset:?}");
            assert!(mesh.voxels.iter().any(|c| c.material.is_wood()));
            assert!(mesh.voxels.iter().any(|c| c.material.is_foliage()));
            let unique: std::collections::HashSet<_> =
                mesh.voxels.iter().map(|v| (v.x, v.y, v.z)).collect();
            assert_eq!(unique.len(), mesh.voxel_count());
            assert!(mesh.triangle_count() > 0);
        }
    }

    #[test]
    fn species_presets_have_distinct_game_scale_growth_profiles() {
        let oak = FoliageProject::from_preset(PlantPreset::Oak);
        let pine = FoliageProject::from_preset(PlantPreset::Pine);
        let birch = FoliageProject::from_preset(PlantPreset::Birch);
        let willow = FoliageProject::from_preset(PlantPreset::Willow);
        let maple = FoliageProject::from_preset(PlantPreset::Maple);
        let shrub = FoliageProject::from_preset(PlantPreset::Shrub);

        assert!(pine.height > birch.height && birch.height > oak.height);
        assert!(shrub.height < oak.height * 0.4);
        assert!(oak.trunk_radius > birch.trunk_radius);
        assert!(oak.carve_bottom > pine.carve_bottom);
        assert!(birch.carve_bottom > oak.carve_bottom);
        assert!(pine.branch_length < willow.branch_length);
        assert!(willow.gravity > oak.gravity);
        assert!(maple.leaf_density > birch.leaf_density);
        assert_eq!(pine.branch_growth, BranchGrowth::Classic);
        assert_eq!(willow.branch_growth, BranchGrowth::Classic);
        assert_eq!(oak.branch_growth, BranchGrowth::CanopyGuided);
        for preset in PlantPreset::ALL {
            let tree = FoliageProject::from_preset(preset);
            tree.validate().unwrap();
            let a = tree.generate().unwrap();
            let b = tree.generate().unwrap();
            assert_eq!(a.voxels, b.voxels, "non-deterministic {preset:?}");
            assert_eq!(a.shape, VoxelShape::HexPrism);
            assert_eq!(a.voxel_size, WORLD_HEXEL_SIZE_METRES);
            assert!(a.voxels.iter().any(|v| v.material.is_wood()), "{preset:?}");
            assert!(a.voxels.iter().any(|v| v.material.is_foliage()), "{preset:?}");
        }
    }

    #[test]
    fn species_have_characteristic_whorls_taper_and_droop() {
        let branches_for=|preset| {
            FoliageProject::from_preset(preset).generate().unwrap()
                .structure.unwrap().branches.into_iter()
                .filter(|b|b.part==TreePart::MajorBranch)
                .collect::<Vec<_>>()
        };
        let pine=branches_for(PlantPreset::Pine);
        let mut whorls=BTreeMap::<i32,usize>::new();
        for b in &pine {*whorls.entry(b.root[2]).or_default()+=1;}
        assert!(whorls.values().filter(|&&n|n>=4).count()>=4,
            "Pine branches should cluster into horizontal tiers");
        let distance=|b:&TreeBranch| {
            let q=(b.tip[0]-b.root[0]).abs();
            let r=(b.tip[1]-b.root[1]).abs();
            let s=(b.tip[0]+b.tip[1]-b.root[0]-b.root[1]).abs();
            (q+r+s)/2
        };
        let half=pine.len()/2;
        let lower=pine[..half].iter().map(distance).sum::<i32>() as f32/half as f32;
        let upper=pine[half..].iter().map(distance).sum::<i32>() as f32
            /(pine.len()-half) as f32;
        assert!(lower>upper+1.0,"Pine must taper: {lower} versus {upper}");
        let willow=branches_for(PlantPreset::Willow);
        let droop=willow.iter().filter(|b|b.tip[2]<b.root[2]).count();
        assert!(droop*2>willow.len(),"Willow branches should descend");
        let birch=branches_for(PlantPreset::Birch);
        let ascending=birch.iter().filter(|b|b.tip[2]>b.root[2]).count();
        assert!(ascending*2>birch.len(),"Birch branches should rise");
        let shrub=FoliageProject::from_preset(PlantPreset::Shrub).generate().unwrap();
        let oak=FoliageProject::from_preset(PlantPreset::Oak).generate().unwrap();
        assert!(shrub.height<oak.height*0.4);
        assert!(shrub.voxels.iter().any(|v|v.material.is_foliage()));
    }

    #[test]
    fn each_visible_face_is_a_real_cube_or_hex_prism_surface() {
        for shape in [VoxelShape::Cube, VoxelShape::HexPrism] {
            let mut cells = BTreeMap::new();
            cells.insert([0, 0, 0], Material::Bark);
            let faces = surface_triangles(&cells, shape, 1.0, LeafRenderSettings::default());
            let expected = if shape == VoxelShape::Cube { 12 } else { 20 };
            assert_eq!(faces.len(), expected);
            cells.insert([0, 0, 1], Material::Leaf);
            let faces = surface_triangles(&cells, shape, 1.0, LeafRenderSettings::default());
            assert_eq!(faces.len(), expected+8, "leaf is four 2-sided triangles per plane");
            assert_eq!(faces.iter().filter(|t|t.cutout).count(),8);
            let solid=LeafRenderSettings {
                geometry:LeafGeometry::SolidVoxel,..Default::default() };
            let legacy = surface_triangles(&cells,shape,1.0,solid);
            assert_eq!(legacy.len(),expected*2-if shape==VoxelShape::Cube {4} else {8});

        }
    }

    #[test]
    fn cutout_texture_is_binary_with_real_air_gaps() {
        let tex=leaf_cutout_rgba();
        assert_eq!(tex.len(),LEAF_TEXTURE_SIZE*LEAF_TEXTURE_SIZE*4);
        assert!(tex.chunks_exact(4).any(|p|p[3]==0));
        assert!(tex.chunks_exact(4).any(|p|p[3]==255));
        assert!(tex.chunks_exact(4).all(|p|p[3]==0 || p[3]==255));
        let mask=leaf_opacity_rgba();
        assert_eq!(mask.len(),tex.len());
        assert!(mask.chunks_exact(4).all(|p|p[3]==255));
    }

    #[test]
    fn species_cutout_art_is_valid_and_major_families_differ() {
        let sample=|species| {
            let rgba=leaf_cutout_for_species(species);
            assert_eq!(rgba.len(),LEAF_TEXTURE_SIZE*LEAF_TEXTURE_SIZE*4);
            let alphas:Vec<_>=rgba.chunks_exact(4).map(|p|p[3]).collect();
            assert!(alphas.iter().all(|&a|a==0||a==255),"{species:?}");
            let filled=alphas.iter().filter(|&&a|a==255).count();
            assert!(filled>40 && filled<3000,"{species:?}: {filled}");
            alphas
        };
        for species in PlantPreset::ALL { sample(species); }
        let shapes=[
            PlantPreset::Oak,PlantPreset::Pine,PlantPreset::Maple,
            PlantPreset::Willow,PlantPreset::Acacia,PlantPreset::GreenBamboo,
        ];
        for (i,&a) in shapes.iter().enumerate() {
            for &b in &shapes[..i] {
                assert_ne!(sample(a),sample(b),"{a:?} and {b:?} must differ");
            }
        }
        // Three apple varieties have the same leaf morphology.
        assert_eq!(sample(PlantPreset::RedApple),sample(PlantPreset::PinkApple));
        assert_eq!(sample(PlantPreset::RedApple),sample(PlantPreset::YellowApple));
    }

    #[test]
    fn leaves_are_crossed_cutout_meshes_with_uvs_and_static_wind_anchor() {
        for shape in [VoxelShape::Cube,VoxelShape::HexPrism] {
            let model=FoliageMesh::from_voxels(shape,1.0,vec![
                VoxelCell{x:0,y:0,z:0,material:Material::Leaf}]).unwrap();
            assert_eq!(model.triangle_count(),8);
            assert!(model.triangles.iter().all(|t|t.cutout));
            assert!(model.triangles.iter().any(|t|t.wind_weight.contains(&1.0)));
            assert!(model.triangles.iter().any(|t|t.wind_weight.contains(&0.0)));
            assert!(model.obj().contains("vt 0.000000"));
            assert!(model.obj().contains("f 1/1"));
            let first=&model.triangles[0];
            let changed=sway_leaf_vertex(first.points[2],first.cell,
                1.0,0.37,model.leaf_render,model.voxel_size);
            assert_ne!(changed,first.points[2]);
            assert_eq!(sway_leaf_vertex(first.points[0],first.cell,
                0.0,0.37,model.leaf_render,model.voxel_size),first.points[0]);
        }
    }

    #[test]
    fn leaf_surrounded_by_opaque_bark_is_removed_but_surrounded_by_leaves_remains() {
        for surroundings in [Material::Bark, Material::Leaf] {
            let mut cells=BTreeMap::new();
            cells.insert([0,0,0],Material::Leaf);
            for face in 0..4 {
                let d=neighbor(VoxelShape::Cube,face);
                cells.insert(d,surroundings);
            }
            cells.insert([0,0,1],surroundings);
            cells.insert([0,0,-1],surroundings);
            let result=surface_triangles(&cells,VoxelShape::Cube,1.0,LeafRenderSettings::default());
            let center=result.iter().filter(|t|t.cell==[0,0,0]).count();
            assert_eq!(center, if surroundings==Material::Bark {0} else {8});
        }
    }

    #[test]
    fn cutout_export_materials_reference_both_pngs() {
        let project=FoliageProject::default();
        let obj=project.generate().unwrap().obj();
        assert!(obj.contains("vt "));
        assert!(obj.contains("f 1/1"));
        let mat=mtl(&project);
        assert!(mat.contains("map_Kd foliage-leaves.png"));
        assert!(mat.contains("map_d foliage-leaves-alpha.png"));
        let mut solid=project;
        solid.leaf_render.geometry=LeafGeometry::SolidVoxel;
        assert!(!mtl(&solid).contains("map_d"));
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
    fn exported_voxel_tree_is_deterministic_and_game_grid_only() {
        let tree = FoliageProject::from_preset(PlantPreset::Willow);
        let a = tree.generate().unwrap();
        let b = tree.generate().unwrap();
        assert_eq!(a.voxel_json(&tree).unwrap(), b.voxel_json(&tree).unwrap());
        assert_eq!(a.obj(), b.obj());
        let json: serde_json::Value =
            serde_json::from_str(&a.voxel_json(&tree).unwrap()).unwrap();
        assert_eq!(json["voxels"].as_array().unwrap().len(), a.voxel_count());
        assert_eq!(json["format"].as_str(), Some("hexel-game-tree-v1"));
        assert_eq!(json["shape"].as_str(), Some("HexPrism"));
        assert_eq!(json["voxel_size"].as_f64(), Some(1.0));
        assert_eq!(json["cell_dimensions_m"]["height"].as_f64(), Some(1.0));
        assert_eq!(a.obj().lines().filter(|l|l.starts_with("f ")).count(),
            a.triangle_count());
    }

    #[test]
    fn game_hexels_are_one_metre_with_textured_harvestable_leaf_cells() {
        let project = FoliageProject::from_preset(PlantPreset::Oak);
        let world = project.generate().unwrap();
        assert_eq!(world.shape, VoxelShape::HexPrism);
        assert_eq!(world.voxel_size, WORLD_HEXEL_SIZE_METRES);
        assert_eq!(world.leaf_render.geometry, LeafGeometry::CrossedCutout);
        assert!(world.voxels.iter().any(|v| v.material.is_wood()));
        assert!(world.voxels.iter().any(|v| v.material.is_foliage()));
        let leaves: std::collections::HashSet<_> = world.voxels.iter()
            .filter(|v| v.material.is_foliage())
            .map(|v| [v.x, v.y, v.z]).collect();
        assert!(world.triangles.iter().any(|t| t.cutout && leaves.contains(&t.cell)));
        let a = grid_position(VoxelShape::HexPrism, 1.0, [0, 0, 0]);
        let b = grid_position(VoxelShape::HexPrism, 1.0, [1, 0, 0]);
        assert!((b[0] - a[0] - 1.0).abs() < 1e-5);
        let above = grid_position(VoxelShape::HexPrism, 1.0, [0, 0, 1]);
        assert!((above[2] - a[2] - 1.0).abs() < 1e-5);
    }

    #[test]
    fn canopy_guidance_changes_branches_but_remains_deterministic() {
        let mut project = FoliageProject::from_preset(PlantPreset::Oak);
        project.branch_growth = BranchGrowth::Classic;
        let classic = project.generate().unwrap();
        project.branch_growth = BranchGrowth::CanopyGuided;
        let first = project.generate().unwrap();
        let again = project.generate().unwrap();
        assert_eq!(first.voxels, again.voxels);
        assert_ne!(classic.voxels, first.voxels);
    }

    #[test]
    fn generated_world_wood_is_face_connected() {
        for growth in [BranchGrowth::Classic, BranchGrowth::CanopyGuided] {
            let mut project = FoliageProject::from_preset(PlantPreset::Oak);
            project.branch_growth = growth;
            let tree = project.generate().unwrap();
            let wood: std::collections::HashSet<_> = tree.voxels.iter()
                .filter(|v| v.material.is_wood())
                .map(|v| [v.x, v.y, v.z]).collect();
            let start = *wood.iter().next().unwrap();
            let mut visited = std::collections::HashSet::from([start]);
            let mut pending = vec![start];
            while let Some(at) = pending.pop() {
                for direction in [
                    [0,1,0],[-1,1,0],[-1,0,0],[0,-1,0],[1,-1,0],[1,0,0],
                    [0,0,-1],[0,0,1],
                ] {
                    let adjacent = [
                        at[0] + direction[0],
                        at[1] + direction[1],
                        at[2] + direction[2],
                    ];
                    if wood.contains(&adjacent) && visited.insert(adjacent) {
                        pending.push(adjacent);
                    }
                }
            }
            assert_eq!(wood.len(), visited.len(), "disconnected wood: {growth:?}");
        }
    }

    #[test]
    fn all_species_have_valid_branch_hierarchy_and_supported_leaves() {
        const NEIGHBORS:[[i32;3];8]=[
            [0,1,0],[-1,1,0],[-1,0,0],[0,-1,0],[1,-1,0],[1,0,0],
            [0,0,1],[0,0,-1],
        ];
        for preset in PlantPreset::ALL {
            let mesh=FoliageProject::from_preset(preset).generate().unwrap();
            let tree=mesh.structure.as_ref().expect("procedural tree has structure");
            tree.validate(&mesh.voxels).unwrap();
            assert!(tree.branches.iter().any(|b|b.part==TreePart::MajorBranch));
            assert!(tree.branches.iter().any(|b|b.part==TreePart::SmallBranch));
            assert!(tree.cells.iter().any(|c|c.part==TreePart::Trunk));
            assert!(tree.cells.iter().any(|c|c.part==TreePart::Foliage));
            let wood:BTreeSet<_>=mesh.voxels.iter().filter(|v|v.material.is_wood())
                .map(|v|[v.x,v.y,v.z]).collect();
            // Branch endpoints/attachments are actual connected wood cells.
            for b in &tree.branches {
                assert!(wood.contains(&b.root),"{preset:?}: missing branch root");
                assert!(wood.contains(&b.tip),"{preset:?}: missing branch tip");
            }
            let all:BTreeSet<_>=tree.cells.iter().map(|c|c.cell).collect();
            let mut reachable=wood.clone();
            let mut queue:VecDeque<_>=wood.into_iter().collect();
            while let Some(pos)=queue.pop_front() {
                for d in NEIGHBORS {
                    let n=[pos[0]+d[0],pos[1]+d[1],pos[2]+d[2]];
                    if all.contains(&n) && reachable.insert(n) {
                        queue.push_back(n);
                    }
                }
            }
            assert_eq!(reachable.len(),mesh.voxel_count(),
                "{preset:?}: unsupported foliage island");
        }
    }

    #[test]
    fn structure_site_checks_ground_footprint_and_volume_clearance() {
        let mesh=FoliageProject::from_preset(PlantPreset::Oak).generate().unwrap();
        let structure=mesh.structure.as_ref().unwrap();
        assert!(structure.fits_site(|_|true,|_|true));
        let missing=structure.root_footprint[0];
        assert!(!structure.fits_site(|pos|pos!=missing,|_|true));
        let blocked=structure.cells.last().unwrap().cell;
        assert!(!structure.fits_site(|_|true,|pos|pos!=blocked));
        let mut bad=structure.clone();
        bad.cells[0].branch_id=Some(999_999);
        assert!(bad.validate(&mesh.voxels).is_err());
        let mut bad=structure.clone();
        bad.root_footprint.clear();
        assert!(bad.validate(&mesh.voxels).is_err());
    }

    #[test]
    fn game_foliage_rejects_solid_leaf_meshes() {
        let mut project = FoliageProject::default();
        project.leaf_render.geometry = LeafGeometry::SolidVoxel;
        assert!(project.validate().is_err());
    }

    #[test]
    fn oversized_and_invalid_game_tree_settings_are_rejected() {
        let mut tree = FoliageProject::default();
        tree.height = f32::NAN;
        assert!(tree.validate().is_err());
        tree.height = 30.0;
        tree.overall_scale = 5.0;
        assert!(tree.validate().is_err());
        tree.overall_scale = 1.0;
        tree.height = 7.0;
        tree.leaf_scale = 1.5;
        tree.overall_scale = 5.0;
        assert!(tree.validate().is_err());
    }

}
