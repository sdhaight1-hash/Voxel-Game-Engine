//! Native voxel-tree generator: occupied cube or hexagonal-prism cells are the
//! source of truth. Bark is solid grid geometry; foliage cells anchor custom
//! intersecting cutout cards. There is no dependency on imported game assets.

use crate::voxel::VoxelShape;
use crate::tree_growth_rules::{LeafPlacement, Phyllotaxy, TreeGrowthRules, TrunkGrowthRules};
use crate::foliage_materials::FoliageMaterials;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::f32::consts::TAU;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlantPreset {
    Acacia, Birch, Ebony, Kapok, Maple, CrimsonKingMaple, Oak,
    Purpleheart, Walnut, Willow, CherryBlossom,
    KentuckyCoffeetree, EasternCottonwood, FremontCottonwood, BlackCottonwood,
    BaldCypress, MediterraneanCypress, Larch, Pine, Redwood,
    RedApple, PinkApple, YellowApple, Pear, Olive, Peach, Cherry,
    Mango, Orange, Lychee, Breadfruit, Pomegranate,
    ArabicaCoffee, CaturraCoffee, MaragogipeCoffee, CastilloCoffee, TabiCoffee,
    GreenBamboo, BrownBamboo,
    CoconutPalm, DatePalm, FanPalm,
    Shrub,
}

impl PlantPreset {
    /// 40 tree/plant varieties and 2 bamboo, plus a separate shrub.
    pub const TREE_CATALOG: [Self; 42] = [
        Self::Acacia, Self::Birch, Self::Ebony, Self::Kapok, Self::Maple,
        Self::CrimsonKingMaple, Self::Oak, Self::Purpleheart, Self::Walnut,
        Self::Willow, Self::CherryBlossom, Self::KentuckyCoffeetree,
        Self::EasternCottonwood, Self::FremontCottonwood, Self::BlackCottonwood,
        Self::BaldCypress, Self::MediterraneanCypress,
        Self::Larch, Self::Pine, Self::Redwood,
        Self::RedApple, Self::PinkApple, Self::YellowApple, Self::Pear,
        Self::Olive, Self::Peach, Self::Cherry, Self::Mango, Self::Orange,
        Self::Lychee, Self::Breadfruit, Self::Pomegranate,
        Self::ArabicaCoffee, Self::CaturraCoffee, Self::MaragogipeCoffee,
        Self::CastilloCoffee, Self::TabiCoffee,
        Self::GreenBamboo, Self::BrownBamboo,
        Self::CoconutPalm, Self::DatePalm, Self::FanPalm,
    ];
    /// Shrub remains a separate non-tree authoring preset.
    pub const ALL: [Self; 43] = [
        Self::Acacia, Self::Birch, Self::Ebony, Self::Kapok, Self::Maple,
        Self::CrimsonKingMaple, Self::Oak, Self::Purpleheart, Self::Walnut,
        Self::Willow, Self::CherryBlossom, Self::KentuckyCoffeetree,
        Self::EasternCottonwood, Self::FremontCottonwood, Self::BlackCottonwood,
        Self::BaldCypress, Self::MediterraneanCypress,
        Self::Larch, Self::Pine, Self::Redwood,
        Self::RedApple, Self::PinkApple, Self::YellowApple, Self::Pear,
        Self::Olive, Self::Peach, Self::Cherry, Self::Mango, Self::Orange,
        Self::Lychee, Self::Breadfruit, Self::Pomegranate,
        Self::ArabicaCoffee, Self::CaturraCoffee, Self::MaragogipeCoffee,
        Self::CastilloCoffee, Self::TabiCoffee,
        Self::GreenBamboo, Self::BrownBamboo,
        Self::CoconutPalm, Self::DatePalm, Self::FanPalm, Self::Shrub,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::Acacia=>"Acacia", Self::Birch=>"Birch", Self::Ebony=>"Ebony",
            Self::Kapok=>"Kapok", Self::Maple=>"Maple",
            Self::CrimsonKingMaple=>"Crimson King Maple", Self::Oak=>"Oak",
            Self::Purpleheart=>"Purpleheart", Self::Walnut=>"Walnut",
            Self::Willow=>"Willow", Self::CherryBlossom=>"Cherry Blossom",
            Self::KentuckyCoffeetree=>"Kentucky Coffeetree",
            Self::EasternCottonwood=>"Eastern Cottonwood",
            Self::FremontCottonwood=>"Fremont Cottonwood",
            Self::BlackCottonwood=>"Black Cottonwood",
            Self::BaldCypress=>"Bald Cypress",
            Self::MediterraneanCypress=>"Mediterranean Cypress",
            Self::Larch=>"Larch", Self::Pine=>"Pine", Self::Redwood=>"Redwood",
            Self::RedApple=>"Red Apple", Self::PinkApple=>"Pink Apple",
            Self::YellowApple=>"Yellow Apple", Self::Pear=>"Pear",
            Self::Olive=>"Olive", Self::Peach=>"Peach", Self::Cherry=>"Cherry",
            Self::Mango=>"Mango", Self::Orange=>"Orange", Self::Lychee=>"Lychee",
            Self::Breadfruit=>"Breadfruit", Self::Pomegranate=>"Pomegranate",
            Self::ArabicaCoffee=>"Arabica Coffee", Self::CaturraCoffee=>"Caturra Coffee",
            Self::MaragogipeCoffee=>"Maragogipe Coffee",
            Self::CastilloCoffee=>"Castillo Coffee", Self::TabiCoffee=>"Tabi Coffee",
            Self::GreenBamboo=>"Green Bamboo", Self::BrownBamboo=>"Brown Bamboo",
            Self::CoconutPalm=>"Coconut Palm", Self::DatePalm=>"Date Palm",
            Self::FanPalm=>"Fan Palm",
            Self::Shrub=>"Shrub",
        }
    }
    pub fn group(self) -> &'static str {
        match self {
            Self::Acacia|Self::Birch|Self::Ebony|Self::Kapok|Self::Maple|
            Self::CrimsonKingMaple|Self::Oak|Self::Purpleheart|Self::Walnut|
            Self::Willow|Self::CherryBlossom|Self::KentuckyCoffeetree|
            Self::EasternCottonwood|Self::FremontCottonwood|
            Self::BlackCottonwood => "Hardwood & broadleaf",
            Self::BaldCypress|Self::MediterraneanCypress|Self::Larch|
            Self::Pine|Self::Redwood => "Conifer & evergreen",
            Self::GreenBamboo|Self::BrownBamboo => "Bamboo",
            Self::CoconutPalm|Self::DatePalm|Self::FanPalm => "Palms",
            Self::ArabicaCoffee|Self::CaturraCoffee|Self::MaragogipeCoffee|
            Self::CastilloCoffee|Self::TabiCoffee => "Coffee plants",
            Self::Shrub => "Other foliage",
            _ => "Fruit trees",
        }
    }
    /// Material-catalog titles match each species' own bark/leaf recipes.
    /// Legacy saved projects still retain their stored graph snapshots.
    pub fn material_name(self) -> &'static str {
        match self {
            Self::CrimsonKingMaple=>"Crimson King maple",
            Self::BaldCypress=>"Bald cypress",
            Self::MediterraneanCypress=>"Mediterranean cypress",
            Self::RedApple=>"Red apple", Self::PinkApple=>"Pink apple",
            Self::YellowApple=>"Yellow apple",
            Self::GreenBamboo=>"Green bamboo", Self::BrownBamboo=>"Brown bamboo",
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
    pub fn is_palm(self) -> bool {
        matches!(self,Self::CoconutPalm|Self::DatePalm|Self::FanPalm)
    }
    pub fn is_coffee(self) -> bool {
        matches!(self,Self::ArabicaCoffee|Self::CaturraCoffee|
            Self::MaragogipeCoffee|Self::CastilloCoffee|Self::TabiCoffee)
    }
    pub fn is_cottonwood(self) -> bool {
        matches!(self,Self::EasternCottonwood|Self::FremontCottonwood|
            Self::BlackCottonwood)
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
            Self::ArabicaCoffee|Self::CaturraCoffee|Self::MaragogipeCoffee|
            Self::CastilloCoffee|Self::TabiCoffee=>Some([173,35,42]),
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

/// Broad deltoid Populus blades versus Black Cottonwood's ovate blades.
fn cottonwood_blade(u:f32,v:f32,cx:f32,cy:f32,rx:f32,ry:f32,ovate:bool)->bool {
    let dx=(u-cx)/rx;
    let dy=(v-cy)/ry;
    if dy.abs()>1.0 {return false;}
    let width=if ovate {
        (1.0-dy*dy).max(0.0).sqrt()*(1.0+0.11*dy)
    } else {
        (0.06+0.94*((dy+1.0)*0.5).powf(0.68)).min(1.0)
    };
    dx.abs()<=width*(1.0+0.06*(dy*29.0).sin().abs())
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
        PlantPreset::Kapok => {
            // Palmately compound leaves: leaflets radiate from one petiole.
            [[0.37,0.35],[0.64,0.61]].iter().any(|p| {
                (0..7).any(|i| {
                    let a=i as f32/7.0*TAU;
                    leaf_blade(u,v,p[0]+a.cos()*0.062,
                        p[1]+a.sin()*0.062,[a.cos(),a.sin()],
                        0.120,0.031,false)
                })
            })
        }
        PlantPreset::Breadfruit => {
            // One large, deeply lobed simple leaf, not Kapok leaflets.
            [[0.35,0.35],[0.66,0.61]].iter().any(|p| {
                leaf_oval(u,v,p[0],p[1],0.085,0.13)
                    ||(0..5).any(|i| {
                        let side=if i%2==0 {-1.0} else {1.0};
                        let y=p[1]-0.085+i as f32*0.037;
                        leaf_blade(u,v,p[0]+side*0.065,y,
                            [side,0.05],0.088,0.028,false)
                    })
            })
        }
        PlantPreset::Purpleheart => {
            // Peltogyne leaves typically have one paired set of leaflets.
            leaf_oval(u,v,0.36,0.49,0.12,0.074)
                ||leaf_oval(u,v,0.64,0.49,0.12,0.074)
                ||leaf_segment(u,v,[0.50,0.55],[0.35,0.49],0.010)
                ||leaf_segment(u,v,[0.50,0.55],[0.65,0.49],0.010)
        }
        PlantPreset::KentuckyCoffeetree => {
            // Twice-compound rachises with paired secondary leaflets.
            (0..5).any(|i| {
                let y=0.21+i as f32*0.135;
                [-1.0_f32,1.0].iter().any(|&side| {
                    let end=[0.50+side*0.25,y-0.032];
                    leaf_segment(u,v,[0.50,y],end,0.008)
                        ||(0..3).any(|j| {
                            let t=0.27+j as f32*0.30;
                            let x=0.50+side*0.25*t;
                            leaf_oval(u,v,x,y-0.032*t-0.027,0.026,0.027)
                                ||leaf_oval(u,v,x,y-0.032*t+0.025,0.027,0.026)
                        })
                })
            })
        }
        PlantPreset::Walnut|PlantPreset::Lychee => {
            (0..6).any(|i| {
                let y=0.23+i as f32*0.108;
                leaf_oval(u,v,0.39,y,0.070,0.037)
                    ||leaf_oval(u,v,0.61,y+0.025,0.070,0.037)
            })
        }
        PlantPreset::Larch => {
            // Tufts of needles on short spur shoots, unlike flat cypress sprays.
            (0..5).any(|i| {
                let y=0.25+i as f32*0.13;
                (0..9).any(|j| {
                    let a=TAU*j as f32/9.0;
                    leaf_segment(u,v,[0.50,y],
                        [0.50+a.cos()*0.16,y+a.sin()*0.10],0.009)
                })
            })
        }
        PlantPreset::MediterraneanCypress => {
            // Dense, fine scale-leaf spray: narrow, upright outer silhouette.
            (0..21).any(|i| {
                let y=0.16+i as f32*0.031;
                let span=0.036+0.032*((i%4) as f32/3.0);
                leaf_segment(u,v,[0.50,y],[0.50-span,y-0.020],0.012)
                    ||leaf_segment(u,v,[0.50,y],[0.50+span,y-0.020],0.012)
            })
        }
        PlantPreset::BaldCypress|PlantPreset::Redwood => {
            (0..13).any(|i| {
                let y=0.17+i as f32*0.036;
                let span=if species==PlantPreset::Redwood {0.14} else {0.20};
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
        PlantPreset::ArabicaCoffee|PlantPreset::CaturraCoffee|
        PlantPreset::MaragogipeCoffee|PlantPreset::CastilloCoffee|
        PlantPreset::TabiCoffee => {
            // Glossy opposite elliptical Coffea leaves with pointed tips.
            let large=species==PlantPreset::MaragogipeCoffee;
            let (length,width)=if large {(0.135,0.067)} else {(0.105,0.050)};
            (0..4).any(|i| {
                let y=0.23+i as f32*0.175;
                leaf_blade(u,v,0.36,y,[-0.62,-0.78],
                    length,width,false)
                    ||leaf_blade(u,v,0.64,y,[0.62,-0.78],
                        length,width,false)
                    ||leaf_segment(u,v,[0.50,y],[0.36,y],0.006)
                    ||leaf_segment(u,v,[0.50,y],[0.64,y],0.006)
            })
        }
        PlantPreset::EasternCottonwood|PlantPreset::FremontCottonwood|
        PlantPreset::BlackCottonwood => {
            // Eastern/Fremont deltoid blades; Western black cottonwood ovate.
            let black=species==PlantPreset::BlackCottonwood;
            let fremont=species==PlantPreset::FremontCottonwood;
            [[0.35,0.30],[0.67,0.47],[0.34,0.63],[0.66,0.77]]
                .iter().any(|p| {
                    cottonwood_blade(u,v,p[0],p[1],
                        if fremont {0.105} else if black {0.080} else {0.095},
                        if black {0.125} else {0.099},black)
                })
        }
        PlantPreset::Ebony|PlantPreset::Mango|PlantPreset::Orange|
        PlantPreset::RedApple|PlantPreset::PinkApple|PlantPreset::YellowApple|
        PlantPreset::Pear|PlantPreset::Olive|PlantPreset::Peach|
        PlantPreset::Cherry|PlantPreset::CherryBlossom|PlantPreset::Pomegranate => {
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
        PlantPreset::CoconutPalm|PlantPreset::DatePalm => {
            // Feather-like pinnate blades, paired along a central rachis.
            // Date palm leaflets are denser and stiffer than coconut leaves.
            let count=if species==PlantPreset::DatePalm {17} else {13};
            (0..count).any(|i| {
                let t=i as f32/(count-1) as f32;
                let y=0.18+0.68*t;
                let span=0.22*(1.0-(2.0*t-1.0).abs()*0.55);
                leaf_blade(u,v,0.50-span*0.42,y-0.025,
                    [-0.87,-0.50],span*0.58,0.019,false)
                    ||leaf_blade(u,v,0.50+span*0.42,y-0.025,
                        [0.87,-0.50],span*0.58,0.019,false)
                    ||leaf_segment(u,v,[0.5,y],[0.5-span,y-0.040],0.007)
                    ||leaf_segment(u,v,[0.5,y],[0.5+span,y-0.040],0.007)
            })
        }
        PlantPreset::FanPalm => {
            // Fan blades radiate from a single petiole tip, not a rachis.
            (0..13).any(|i| {
                let a=std::f32::consts::PI*(1.0+i as f32/12.0);
                let dir=[a.cos(),a.sin()];
                leaf_blade(u,v,0.50+dir[0]*0.21,0.61+dir[1]*0.21,
                    dir,0.20,0.033,false)
                    ||leaf_segment(u,v,[0.50,0.61],
                        [0.50+dir[0]*0.34,0.61+dir[1]*0.34],0.008)
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
        PlantPreset::Redwood|PlantPreset::CoconutPalm|
        PlantPreset::DatePalm|PlantPreset::FanPalm => false,
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
    /// Pipe-model influence on authored woody branch radii; palms and bamboo
    /// use their anatomical stem/frond rules instead.
    pub wood_pipe_blend: f32,
    /// Area/pipe conservation exponent (2 = cross-sectional area).
    pub wood_pipe_exponent: f32,
    /// Optional artist overrides. Absent in older saved foliage projects.
    /// Values are constrained around the currently selected species.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub growth_rules_override: Option<TreeGrowthRules>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trunk_rules_override: Option<TrunkGrowthRules>,
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

/// A continuous wood segment in millimetres (deterministic and Eq/JSON safe).
/// Render geometry is derived from these points, but 1m hexel cells remain
/// authoritative for harvesting and persistence.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BranchSpan {
    pub branch_id: u32,
    pub from_mm: [i32; 3],
    pub to_mm: [i32; 3],
    pub start_radius_mm: u16,
    pub end_radius_mm: u16,
}

impl BranchSpan {
    fn new(branch_id:u32, a:[f32;3], b:[f32;3], start:f32, end:f32)->Self {
        Self {
            branch_id,
            from_mm:a.map(|p|(p*1000.0).round() as i32),
            to_mm:b.map(|p|(p*1000.0).round() as i32),
            start_radius_mm:(start*1000.0).round() as u16,
            end_radius_mm:(end*1000.0).round() as u16,
        }
    }
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
    /// Tapered trunk-spine (id 0) and branch spans; old saves without the spine
    /// continue to show a solid hexel trunk. No path is regenerated after
    /// a player removes a wood hexel.
    #[serde(default)]
    pub spans: Vec<BranchSpan>,
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
        if self.spans.len()>self.branches.len()*3+128 {
            return Err("Too many branch or trunk segments for the tree".into());
        }
        for span in &self.spans {
            let max_radius=if span.branch_id==0 {8_000} else {500};
            if !by_id.contains_key(&span.branch_id) ||
                span.start_radius_mm==0 || span.end_radius_mm==0 ||
                span.start_radius_mm>max_radius ||
                span.end_radius_mm>max_radius ||
                span.from_mm==span.to_mm ||
                span.from_mm.iter().chain(span.to_mm.iter())
                    .any(|&v|!(-129_000..=129_000).contains(&v)) {
                return Err("Invalid tapered branch segment".into());
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
                PlantPreset::CherryBlossom => (9.5,0.48,36,0.58,0.05,0.90,0.74,[107,73,65],[91,147,74]),
                PlantPreset::KentuckyCoffeetree => (23.0,0.91,40,0.55,0.06,0.74,0.78,[91,84,75],[79,139,67]),
                PlantPreset::EasternCottonwood => (26.0,1.00,54,0.49,0.04,0.84,0.78,[117,108,94],[75,150,67]),
                PlantPreset::FremontCottonwood => (23.0,0.93,50,0.53,0.05,0.81,0.75,[132,115,96],[99,167,72]),
                PlantPreset::BlackCottonwood => (28.0,1.08,55,0.41,0.04,0.78,0.70,[104,99,94],[72,127,79]),
                PlantPreset::BaldCypress => (19.0,0.75,42,0.39,0.05,0.74,0.47,[119,88,64],[90,128,70]),
                PlantPreset::MediterraneanCypress => (17.0,0.53,42,0.19,0.02,0.91,0.37,[88,71,52],[39,86,58]),
                PlantPreset::Larch => (17.0,0.55,42,0.44,0.03,0.69,0.42,[112,90,61],[126,157,70]),
                PlantPreset::Pine => (18.0,0.53,48,0.46,0.06,0.78,0.44,[89,71,52],[40,94,58]),
                PlantPreset::Redwood => (27.0,1.28,66,0.36,0.04,0.76,0.52,[121,69,54],[47,91,67]),
                PlantPreset::RedApple => (8.0,0.40,26,0.58,0.10,0.89,0.67,[102,76,57],[78,144,63]),
                PlantPreset::PinkApple => (8.0,0.40,26,0.58,0.10,0.89,0.67,[104,78,58],[88,150,66]),
                PlantPreset::YellowApple => (8.0,0.40,26,0.58,0.10,0.89,0.67,[105,77,57],[107,156,68]),
                PlantPreset::Pear => (9.5,0.44,27,0.44,0.05,0.82,0.63,[114,84,62],[95,154,65]),
                PlantPreset::Olive => (7.5,0.51,34,0.60,0.14,0.80,0.57,[112,106,84],[106,131,104]),
                PlantPreset::Peach => (8.0,0.37,27,0.56,0.06,0.78,0.63,[107,73,65],[91,149,77]),
                PlantPreset::Cherry => (10.0,0.47,32,0.54,0.06,0.88,0.70,[116,69,61],[81,146,71]),
                PlantPreset::Mango => (13.0,0.76,40,0.59,0.07,0.92,0.80,[108,77,54],[57,122,60]),
                PlantPreset::Orange => (9.0,0.51,34,0.57,0.06,0.90,0.70,[112,81,57],[49,112,55]),
                PlantPreset::Lychee => (11.0,0.62,36,0.58,0.04,0.86,0.72,[103,75,58],[65,131,65]),
                PlantPreset::Breadfruit => (16.0,0.79,40,0.56,0.08,0.85,0.85,[113,80,59],[79,149,71]),
                PlantPreset::Pomegranate => (6.5,0.36,30,0.62,0.11,0.78,0.57,[100,83,57],[67,139,65]),
                PlantPreset::ArabicaCoffee => (5.8,0.27,29,0.43,0.02,0.87,0.55,[101,91,75],[42,108,60]),
                PlantPreset::CaturraCoffee => (3.5,0.21,27,0.43,0.02,0.91,0.54,[104,93,77],[48,119,63]),
                PlantPreset::MaragogipeCoffee => (7.5,0.35,30,0.47,0.02,0.86,0.74,[107,94,79],[46,114,65]),
                PlantPreset::CastilloCoffee => (4.3,0.24,30,0.43,0.02,0.92,0.57,[103,91,75],[47,117,63]),
                PlantPreset::TabiCoffee => (6.8,0.32,32,0.44,0.02,0.87,0.61,[101,89,71],[44,111,62]),
                PlantPreset::GreenBamboo => (11.0,0.23,15,0.20,0.03,0.72,0.43,[74,151,64],[86,159,63]),
                PlantPreset::BrownBamboo => (11.0,0.23,15,0.20,0.03,0.72,0.43,[152,121,69],[113,133,65]),
                PlantPreset::CoconutPalm => (17.0,0.44,18,0.34,0.08,0.88,0.48,[127,107,77],[64,139,69]),
                PlantPreset::DatePalm => (14.0,0.57,24,0.30,0.04,0.91,0.47,[135,103,71],[70,126,66]),
                PlantPreset::FanPalm => (16.0,0.48,20,0.29,0.03,0.86,0.56,[140,112,86],[79,150,84]),
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
                PlantPreset::CherryBlossom=>(0.39,0.05,0.21,0.13,0.29,BranchGrowth::CanopyGuided),
                PlantPreset::KentuckyCoffeetree=>(0.38,0.08,0.16,0.12,0.31,BranchGrowth::Classic),
                PlantPreset::EasternCottonwood=>(0.29,0.06,0.19,0.12,0.31,BranchGrowth::CanopyGuided),
                PlantPreset::FremontCottonwood=>(0.32,0.07,0.17,0.12,0.29,BranchGrowth::CanopyGuided),
                PlantPreset::BlackCottonwood=>(0.42,0.06,0.23,0.11,0.29,BranchGrowth::CanopyGuided),
                PlantPreset::Mango=>(0.32,0.04,0.15,0.12,0.23,BranchGrowth::CanopyGuided),
                PlantPreset::Orange=>(0.21,0.06,0.12,0.11,0.27,BranchGrowth::CanopyGuided),
                PlantPreset::Lychee=>(0.30,0.05,0.16,0.11,0.28,BranchGrowth::CanopyGuided),
                PlantPreset::Breadfruit=>(0.42,0.06,0.21,0.12,0.28,BranchGrowth::CanopyGuided),
                PlantPreset::Pomegranate=>(0.06,0.07,0.12,0.13,0.30,BranchGrowth::Classic),
                PlantPreset::ArabicaCoffee|PlantPreset::CaturraCoffee|
                PlantPreset::MaragogipeCoffee|PlantPreset::CastilloCoffee|
                PlantPreset::TabiCoffee=>
                    (0.12,0.08,0.14,0.10,0.22,BranchGrowth::Classic),
                PlantPreset::GreenBamboo|PlantPreset::BrownBamboo=>
                    (0.53,0.04,0.10,0.08,0.25,BranchGrowth::Classic),
                PlantPreset::CoconutPalm|PlantPreset::DatePalm|PlantPreset::FanPalm=>
                    (0.80,0.02,0.08,0.06,0.22,BranchGrowth::Classic),
                PlantPreset::Shrub=>(0.05,0.08,0.13,0.18,0.40,BranchGrowth::Classic),
            };
        Self {
            preset, seed: 42, height, trunk_radius: radius, branch_count: branches,
            branch_length, branch_growth,
            wood_pipe_blend:0.35, wood_pipe_exponent:2.3,
            carve_bottom, carve_top,
            growth_rules_override: None, trunk_rules_override: None,
            gravity, phototropism, slope: 0.0, overall_scale: 1.0, prune: 0.0,
            leaf_density: density, leaf_scale, leaf_angle, leaf_variation,
            bark_color: bark, leaf_color: leaves,
            leaf_render: if preset.is_cottonwood() {
                // Flattened Populus petioles flutter in wind; visual sway
                // approximates this without changing the harvestable cells.
                LeafRenderSettings{wind_strength:0.18,wind_speed:2.0,
                    ..LeafRenderSettings::default()}
            } else {LeafRenderSettings::default()},
            materials: FoliageMaterials::for_species(preset),
        }
    }

    pub fn effective_growth_rules(&self) -> TreeGrowthRules {
        self.growth_rules_override.unwrap_or_else(|| self.preset.growth_rules())
    }

    pub fn effective_trunk_rules(&self) -> TrunkGrowthRules {
        self.trunk_rules_override.unwrap_or_else(|| self.preset.trunk_growth_rules())
    }

    pub fn set_growth_rules(&mut self, rules: TreeGrowthRules) -> Result<(), String> {
        let defaults=self.preset.growth_rules();
        if !rules.within_species(defaults) {
            return Err("Growth rules must stay within selected species limits".into());
        }
        self.growth_rules_override=(rules!=defaults).then_some(rules);
        Ok(())
    }

    pub fn set_trunk_rules(&mut self, rules: TrunkGrowthRules) -> Result<(), String> {
        let defaults=self.preset.trunk_growth_rules();
        if !rules.within_species(defaults) {
            return Err("Trunk rules must stay within selected species limits".into());
        }
        self.trunk_rules_override=(rules!=defaults).then_some(rules);
        Ok(())
    }

    pub fn reset_growth_rules(&mut self) {
        self.growth_rules_override=None;
        self.trunk_rules_override=None;
    }

    pub fn validate(&self) -> Result<(), String> {
        if !self.effective_growth_rules().within_species(self.preset.growth_rules())
            || !self.effective_trunk_rules().within_species(self.preset.trunk_growth_rules()) {
            return Err("Custom tree growth rules exceed species limits".into());
        }
        self.leaf_render.validate()?;
        if self.leaf_render.geometry != LeafGeometry::CrossedCutout {
            return Err("Game foliage requires logical leaf cells with textured cutout cards".into());
        }
        self.materials.validate()?;
        for (name, x, min, max) in [
            ("height", self.height, 0.5, 30.0),
            ("trunk radius", self.trunk_radius, 0.02, 2.0),
            ("branch length", self.branch_length, 0.1, 1.5),
            ("pipe-model blend", self.wood_pipe_blend, 0.0, 1.0),
            ("pipe-model exponent", self.wood_pipe_exponent, 1.5, 3.0),
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
            PlantPreset::CherryBlossom => 0.63+0.80*crown_t,
            PlantPreset::KentuckyCoffeetree => 0.73+0.48*(1.0-center*center),
            PlantPreset::EasternCottonwood => 0.62+0.75*(1.0-center*center),
            PlantPreset::FremontCottonwood => 0.75+0.65*(1.0-center*center),
            PlantPreset::BlackCottonwood => 0.44+0.69*(1.0-center*center),
            PlantPreset::Willow => 1.22-0.16*crown_t,
            PlantPreset::Purpleheart|PlantPreset::Mango|PlantPreset::Breadfruit =>
                0.80+0.46*(1.0-center*center),
            PlantPreset::RedApple|PlantPreset::PinkApple|PlantPreset::YellowApple|
            PlantPreset::Peach|PlantPreset::Cherry|PlantPreset::Orange|
            PlantPreset::Lychee => 0.88+0.31*(1.0-center*center),
            PlantPreset::ArabicaCoffee|PlantPreset::CaturraCoffee|
            PlantPreset::MaragogipeCoffee|PlantPreset::CastilloCoffee|
            PlantPreset::TabiCoffee => 0.73+0.26*(1.0-center*center),
            PlantPreset::Olive|PlantPreset::Pomegranate|PlantPreset::Shrub =>
                1.14-0.18*crown_t,
            PlantPreset::GreenBamboo|PlantPreset::BrownBamboo =>
                0.30+0.12*crown_t,
            PlantPreset::CoconutPalm|PlantPreset::DatePalm|PlantPreset::FanPalm =>
                0.55+0.60*crown_t,
        }
    }

    /// Palm-specific architecture: a solitary, nearly cylindrical stem with
    /// curved leaf petioles at its crown. Petioles use the existing branch
    /// metadata only for editable hexel ownership; they are not woody forks.
    fn generate_palm(&self) -> Result<FoliageMesh, String> {
        let shape=VoxelShape::HexPrism;
        let size=WORLD_HEXEL_SIZE_METRES;
        let rules=self.effective_growth_rules();
        let trunk_rules=self.effective_trunk_rules();
        let h=(self.height*self.overall_scale*
            (1.0+(hash01(self.seed,0,172)*2.0-1.0)*
                trunk_rules.height_seed_variation)).min(96.0*size);
        let radius=(self.trunk_radius*self.overall_scale*
            (1.0+(hash01(self.seed,0,173)*2.0-1.0)*
                trunk_rules.radius_seed_variation)).min(8.0*size);
        let lean_x=(self.slope*0.24+
            (hash01(self.seed,0,170)*2.0-1.0)*rules.lean_fraction)*h;
        let lean_y=(hash01(self.seed,0,171)*2.0-1.0)*
            rules.lean_fraction*h;
        let trunk=|t:f32| [
            t*t*lean_x,t*t*lean_y,t*h
        ];
        let mut cells=BTreeMap::<[i32;3],Material>::new();
        let mut ownership=BTreeMap::<[i32;3],(TreePart,Option<u32>)>::new();
        let mut branches=Vec::new();
        let mut spans=Vec::new();
        for step in 0..=((h/(size*0.35)).ceil() as u32) {
            let t=(step as f32*size*0.35/h).min(1.0);
            let flare=(1.0-t/0.18).max(0.0).powi(2);
            let r=(radius*(1.0-t*trunk_rules.taper)*
                (1.0+trunk_rules.base_flare*flare))
                .clamp(size*0.20,8.0*size);
            paint_sphere(&mut cells,shape,size,trunk(t),r,Material::Bark);
        }
        for &cell in cells.keys() {
            ownership.insert(cell,(TreePart::Trunk,Some(0)));
        }
        branches.push(TreeBranch {
            id:0,parent:None,root:[0,0,0],
            tip:grid_key(shape,size,trunk(1.0)),part:TreePart::Trunk
        });
        // Preserve an explicit, deterministic trunk centerline for smooth render
        // geometry. Hexel occupancy remains the authority and is unchanged.
        // 1m segments retain authored trunk lean, taper and basal flare.
        let levels=(h/size).ceil().max(1.0) as u32;
        let radius_at=|t:f32| {
            let flare=(1.0-t/0.18).max(0.0).powi(2);
            (radius*(1.0-t*trunk_rules.taper)*
                (1.0+trunk_rules.base_flare*flare))
                .clamp(size*0.20,8.0*size)
        };
        for k in 0..levels {
            let a=k as f32/levels as f32;
            let b=(k+1) as f32/levels as f32;
            spans.push(BranchSpan::new(0,trunk(a),trunk(b),
                radius_at(a),radius_at(b)));
        }


        for i in 0..self.branch_count {
            let rand=|channel|hash01(self.seed,i,channel);
            if rand(0)<self.prune {continue;}
            let angle=TAU*(i as f32/self.branch_count as f32+
                0.11*(rand(1)-0.5));
            // A series of densely packed leaf bases near the apical bud,
            // rather than branches evenly distributed down the trunk.
            let base_t=(0.92+0.065*rand(2)-0.02*self.carve_top).min(0.995);
            let base=trunk(base_t);
            let reach=self.branch_length*h*rules.reach_factor*
                (0.80+0.30*rand(3));
            let pitch=(rules.primary_pitch[0]+
                (rules.primary_pitch[1]-rules.primary_pitch[0])*rand(4)
                -0.15*self.gravity+0.10*self.phototropism)
                .clamp(rules.primary_pitch[0],rules.primary_pitch[1]);
            let tip=[
                base[0]+reach*angle.cos(),base[1]+reach*angle.sin(),
                (base[2]+reach*pitch).clamp(size*0.5,96.0*size),
            ];
            let mut mid=lerp3(base,tip,0.53);
            // Arching rachis: lift above the straight base-to-tip segment.
            mid[2]=(mid[2]+reach*(0.20+0.14*rand(5)))
                .min(96.0*size);
            let id=i+1;
            let petiole_radius=(radius*0.29).clamp(0.08,0.23);
            let tip_radius=(petiole_radius*0.40).max(0.035);
            branches.push(TreeBranch {
                id,parent:Some(0),root:grid_key(shape,size,base),
                tip:grid_key(shape,size,tip),part:TreePart::MajorBranch,
            });
            spans.push(BranchSpan::new(id,base,mid,petiole_radius,
                petiole_radius*0.72));
            spans.push(BranchSpan::new(id,mid,tip,petiole_radius*0.72,
                tip_radius));
            let mut petiole_cells=BTreeMap::new();
            paint_line(&mut petiole_cells,shape,size,base,mid,
                petiole_radius,Material::Bark);
            paint_line(&mut petiole_cells,shape,size,mid,tip,
                tip_radius,Material::Bark);
            for (cell,material) in petiole_cells {
                let old=cells.insert(cell,material);
                if old.is_none() || old.is_some_and(Material::is_foliage) {
                    ownership.insert(cell,(TreePart::MajorBranch,Some(id)));
                }
            }
            if self.leaf_density==0.0 {continue;}
            let fan=self.preset==PlantPreset::FanPalm;
            let positions:&[f32]=if fan {&[0.79,0.98]} else {
                &[0.46,0.62,0.78,0.94]
            };
            for (j,&t) in positions.iter().enumerate() {
                let location=lerp3(mid,tip,t);
                let r=(self.leaf_scale*self.overall_scale*
                    rules.foliage_factor*
                    if fan {2.25} else {1.60}).max(size*0.82);
                let mut leaf_cells=BTreeMap::new();
                paint_leaf_cluster(&mut leaf_cells,shape,size,location,
                    r,if fan {r*1.05} else {r*0.65},self.leaf_density,
                    self.leaf_variation,self.leaf_angle,self.seed,i*8+j as u32);
                for (cell,material) in leaf_cells {
                    if let std::collections::btree_map::Entry::Vacant(v)=cells.entry(cell) {
                        v.insert(material);
                        ownership.insert(cell,(TreePart::Foliage,Some(id)));
                    }
                }
            }
            if cells.len()>40_000 {
                return Err("Palm exceeds 40,000 hexels: reduce fronds or foliage density".into());
            }
        }
        retain_supported_foliage(&mut cells);
        ownership.retain(|cell,_|cells.contains_key(cell));
        let root_footprint=cells.iter()
            .filter(|(p,m)|p[2]==0&&m.is_wood())
            .map(|(p,_)|[p[0],p[1]]).collect();
        let structure=TreeStructure {
            origin:[0,0,0],root_footprint,branches,
            cells:ownership.into_iter().map(|(cell,(part,branch_id))|
                TreeCell{cell,part,branch_id}).collect(),spans
        };
        let voxels=cells.iter().map(|(p,m)|
            VoxelCell{x:p[0],y:p[1],z:p[2],material:*m}).collect::<Vec<_>>();
        structure.validate(&voxels)?;
        let triangles=tree_surface_triangles(&cells,shape,size,
            self.leaf_render,Some(&structure));
        Ok(FoliageMesh{voxels,triangles,shape,voxel_size:size,
            height:h,leaf_render:self.leaf_render,structure:Some(structure)})
    }

    /// All foliage is directly authored on the game's 1m Q/R/Z hexel grid.
    /// Each wood or leaf entry is a real removable world-cell definition.
    pub fn generate(&self) -> Result<FoliageMesh, String> {
        self.validate()?;
        if self.preset.is_palm() {return self.generate_palm();}
        let mut cells: BTreeMap<[i32; 3], Material> = BTreeMap::new();
        let mut ownership:BTreeMap<[i32;3],(TreePart,Option<u32>)>=BTreeMap::new();
        let mut branches=Vec::new();
        let mut spans=Vec::new();
        let s = WORLD_HEXEL_SIZE_METRES;
        let shape = VoxelShape::HexPrism;
        let rules = self.effective_growth_rules();
        let trunk_rules = self.effective_trunk_rules();
        // Per-individual dimensions vary independently of branch/leaf hashes.
        // Constrain values to the existing author's 96-level / 8-cell limits.
        let h=(self.height*self.overall_scale*(1.0+
            (hash01(self.seed,0,172)*2.0-1.0)*
            trunk_rules.height_seed_variation)).min(96.0*s);
        let trunk_radius=(self.trunk_radius*self.overall_scale*(1.0+
            (hash01(self.seed,0,173)*2.0-1.0)*
            trunk_rules.radius_seed_variation)).min(8.0*s);
        // Seed changes individual trunk habit, not its species or material.
        // Squaring t keeps the trunk anchored at the world-space origin.
        let seed_lean_x=(hash01(self.seed,0,170)*2.0-1.0)*rules.lean_fraction*h;
        let seed_lean_y=(hash01(self.seed,0,171)*2.0-1.0)*rules.lean_fraction*h;
        let trunk = |t: f32| -> [f32; 3] {
            [t * t * (self.slope * h * 0.32 + seed_lean_x),
             t * t * (0.015*h*(self.gravity-self.phototropism) + seed_lean_y),
             t * h]
        };
        // Trunk and limbs are occupied bark voxels, not smooth cylinder geometry.
        for step in 0..=((h / (s * 0.35)).ceil() as u32) {
            let t = (step as f32 * s * 0.35 / h).min(1.0);
            let p = trunk(t);
            // Taper is species-defined; some mature trunks broaden near
            // their base, bamboo stays close to cylindrical.
            let flare=(1.0-t/0.18).max(0.0).powi(2);
            let radius=(trunk_radius*(1.0-t*trunk_rules.taper)*
                (1.0+trunk_rules.base_flare*flare)).clamp(s*0.20,8.0*s);
            paint_sphere(&mut cells, shape, s, p, radius, Material::Bark);
        }
        for cell in cells.keys() { ownership.insert(*cell,(TreePart::Trunk,Some(0))); }
        branches.push(TreeBranch {id:0,parent:None,root:[0,0,0],
            tip:grid_key(shape,s,trunk(1.0)),part:TreePart::Trunk});
        // Preserve an explicit, deterministic trunk centerline for smooth render
        // geometry. Hexel occupancy remains the authority and is unchanged.
        // 1m segments retain authored trunk lean, taper and basal flare.
        let levels=(h/s).ceil().max(1.0) as u32;
        let radius_at=|t:f32| {
            let flare=(1.0-t/0.18).max(0.0).powi(2);
            (trunk_radius*(1.0-t*trunk_rules.taper)*
                (1.0+trunk_rules.base_flare*flare))
                .clamp(s*0.20,8.0*s)
        };
        for k in 0..levels {
            let a=k as f32/levels as f32;
            let b=(k+1) as f32/levels as f32;
            spans.push(BranchSpan::new(0,trunk(a),trunk(b),
                radius_at(a),radius_at(b)));
        }

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
            let (height_fraction,angle) = match rules.phyllotaxy {
                Phyllotaxy::Whorled | Phyllotaxy::CulmNodes |
                Phyllotaxy::CrownFronds => {
                    // One node layer per tier; bamboo branches at internodes,
                    // conifers branch in whorls around a leading trunk.
                    let n=if rules.phyllotaxy==Phyllotaxy::CulmNodes {3} else {6};
                    let layer=i/n;
                    let layers=self.branch_count.div_ceil(n);
                    let fraction=(layer as f32+0.48+0.12*(rand(1)-0.5))
                        /layers as f32;
                    let azimuth=(i%n) as f32/n as f32+
                        layer as f32*0.083+0.035*(rand(2)-0.5);
                    (fraction,TAU*azimuth)
                }
                Phyllotaxy::Opposite => {
                    let pair=i/2;
                    let pairs=self.branch_count.div_ceil(2);
                    let fraction=(pair as f32+0.5+0.12*(rand(1)-0.5))
                        /pairs as f32;
                    let azimuth=pair as f32*0.25+(i%2) as f32*0.5
                        +0.02*(rand(2)-0.5);
                    (fraction,TAU*azimuth)
                }
                Phyllotaxy::Spiral => (
                    (i as f32+0.5+0.25*(rand(1)-0.5))
                        /self.branch_count as f32,
                    TAU*(i as f32*0.6180339+rand(2)*0.2),
                ),
            };
            let t=self.carve_bottom+
                (1.0-self.carve_bottom-self.carve_top)*height_fraction;
            let base=trunk(t);
            let reach=self.branch_length*h*(0.38+0.55*rand(3))
                *self.crown_reach(t)*rules.reach_factor;
            let preferred=(rules.primary_pitch[0]+rules.primary_pitch[1])*0.5;
            let rise=reach*(preferred+0.38*self.phototropism-
                0.42*self.gravity+0.15*(rand(20)-0.5))
                .clamp(rules.primary_pitch[0],rules.primary_pitch[1]);
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
                tip[2] = (tip[2]-base[2])
                    .clamp(reach*rules.primary_pitch[0],
                        reach*rules.primary_pitch[1])+base[2];
                tip[2] = tip[2].max(s * 0.5);
            }
            let mut mid=lerp3(base,tip,0.56);
            if self.preset==PlantPreset::Willow {
                // Outward-arching limb followed by hanging tips.
                mid[2]=(mid[2]+0.24*reach).min(h);
            }
            let thickness = (trunk_radius *
                (0.28 + 0.25 * (1.0 - t))).max(s * 0.17);
            // Visible branch radii are independent of the 1m wood cell.
            // Major branches transition to thin tips, rather than building
            // staircases of full-sized hexagonal prisms.
            let major_radius=(thickness*0.65).clamp(0.12,0.42);
            let tip_radius=(major_radius*0.45).max(0.05);
            let fork_radius=(major_radius*0.35).clamp(0.045,0.16);
            // Reserve IDs even when a twig is omitted, preserving stable
            // parent/child identity across pruning changes.
            let major_id=3*i+1;
            let fork_id=major_id+1;
            branches.push(TreeBranch {id:major_id,parent:Some(0),
                root:grid_key(shape,s,base),tip:grid_key(shape,s,tip),
                part:TreePart::MajorBranch});
            spans.push(BranchSpan::new(major_id,base,mid,major_radius,
                major_radius*0.78));
            spans.push(BranchSpan::new(major_id,mid,tip,major_radius*0.78,
                tip_radius));
            let mut main_cells=BTreeMap::new();
            paint_line(&mut main_cells,shape,s,base,mid,major_radius,Material::Bark);
            paint_line(&mut main_cells,shape,s,mid,tip,tip_radius,Material::Bark);
            for (cell,material) in main_cells {
                let previous=cells.insert(cell,material);
                if previous.is_none() || previous.is_some_and(|m|m.is_foliage()) {
                    ownership.insert(cell,(TreePart::MajorBranch,Some(major_id)));
                }
            }
            let fork_angle = angle + (rand(4) - 0.5) * 1.6;
            let fork_reach=reach*(0.28+0.2*rand(5));
            let fork_rise=(((rules.secondary_pitch[0]+rules.secondary_pitch[1])*0.5)
                +0.18*(rand(21)-0.5)+0.20*self.phototropism-
                0.24*self.gravity)
                .clamp(rules.secondary_pitch[0],rules.secondary_pitch[1]);
            let fork=[
                mid[0]+fork_reach*fork_angle.cos(),
                mid[1]+fork_reach*fork_angle.sin(),
                (mid[2]+fork_reach*fork_rise).max(s*0.5),
            ];
            branches.push(TreeBranch {id:fork_id,parent:Some(major_id),
                root:grid_key(shape,s,mid),tip:grid_key(shape,s,fork),
                part:TreePart::SmallBranch});
            spans.push(BranchSpan::new(fork_id,mid,fork,fork_radius,
                (fork_radius*0.50).max(0.035)));
            let mut fork_cells=BTreeMap::new();
            paint_line(&mut fork_cells,shape,s,mid,fork,fork_radius,Material::Bark);
            for (cell,material) in fork_cells {
                let previous=cells.insert(cell,material);
                if previous.is_none() || previous.is_some_and(|m|m.is_foliage()) {
                    ownership.insert(cell,(TreePart::SmallBranch,Some(fork_id)));
                }
            }
            // An optional additional child makes twig topology vary with the
            // seed. It remains structurally attached to its own parent limb.
            let extra_fork=if rand(22)<rules.extra_twig_chance {
                let root=lerp3(mid,tip,0.34+0.28*rand(23));
                let a=angle+0.5+(rand(24)-0.5)*1.3;
                let length=reach*(0.16+0.18*rand(25));
                let pitch=rules.secondary_pitch[0]+
                    (rules.secondary_pitch[1]-rules.secondary_pitch[0])*rand(26);
                let end=[root[0]+length*a.cos(),root[1]+length*a.sin(),
                    (root[2]+length*pitch).max(s*0.5)];
                let id=major_id+2;
                let radius=(fork_radius*0.68).max(0.032);
                branches.push(TreeBranch{id,parent:Some(major_id),
                    root:grid_key(shape,s,root),tip:grid_key(shape,s,end),
                    part:TreePart::SmallBranch});
                spans.push(BranchSpan::new(id,root,end,radius,
                    (radius*0.52).max(0.026)));
                let mut extra_cells=BTreeMap::new();
                paint_line(&mut extra_cells,shape,s,root,end,radius,Material::Bark);
                for (cell,material) in extra_cells {
                    let previous=cells.insert(cell,material);
                    if previous.is_none() || previous.is_some_and(|m|m.is_foliage()) {
                        ownership.insert(cell,(TreePart::SmallBranch,Some(id)));
                    }
                }
                Some((end,id))
            } else {None};
            if self.leaf_density == 0.0 { continue; }
            // Canopy occupancy is a union of overlapping lobes, not two
            // isolated balls at the branch endpoints. Every new foliage cell
            // still inherits the source branch's harvestable identity.
            let broad_crown=matches!(self.preset,
                PlantPreset::Oak|PlantPreset::Walnut|PlantPreset::Ebony|
                PlantPreset::Maple|PlantPreset::CrimsonKingMaple|
                PlantPreset::Mango|PlantPreset::Lychee|
                PlantPreset::RedApple|PlantPreset::PinkApple|PlantPreset::YellowApple|
                PlantPreset::Cherry|PlantPreset::CherryBlossom|PlantPreset::Orange|
                PlantPreset::EasternCottonwood|PlantPreset::FremontCottonwood|
                PlantPreset::BlackCottonwood);
            let mut leaf_sites=vec![(tip,major_id),(fork,fork_id)];
            if broad_crown && i%3==0 {
                leaf_sites.push((lerp3(mid,tip,0.68),major_id));
                leaf_sites.push((lerp3(mid,fork,0.58),fork_id));
            }
            match rules.foliage {
                LeafPlacement::WeepingCurtain => {
                    for t in [0.20,0.38,0.56,0.74,0.90] {
                        leaf_sites.push((lerp3(mid,fork,t),fork_id));
                    }
                }
                LeafPlacement::SpurNeedles => {
                    for t in [0.26,0.48,0.70,0.86] {
                        leaf_sites.push((lerp3(base,tip,t),major_id));
                    }
                }
                LeafPlacement::FeatherSprays | LeafPlacement::NeedleSprays => {
                    if i%2==0 {
                        leaf_sites.push((lerp3(base,tip,0.50),major_id));
                        leaf_sites.push((lerp3(base,tip,0.74),major_id));
                    }
                }
                LeafPlacement::AlongTwigs => {
                    for t in [0.32,0.63] {
                        leaf_sites.push((lerp3(mid,fork,t),fork_id));
                    }
                }
                LeafPlacement::CulmLeaves => {
                    if height_fraction>0.38 {
                        leaf_sites.push((lerp3(mid,fork,0.72),fork_id));
                    }
                }
                LeafPlacement::OuterCrown | LeafPlacement::PinnateFronds |
                LeafPlacement::FanFronds => {}
            }
            if let Some((end,id))=extra_fork {
                leaf_sites.push((end,id));
            }
            // Keep bamboo foliage above the lower clear-culm region.
            if rules.foliage==LeafPlacement::CulmLeaves && height_fraction<0.38 {
                leaf_sites.clear();
            }
            for (cluster,(location,owner)) in leaf_sites.into_iter().enumerate() {
                let base_radius=self.leaf_scale*self.overall_scale*2.3*
                    rules.foliage_factor;
                let lobe_scale=if broad_crown && cluster>=2 {1.18}
                    else if rules.foliage==LeafPlacement::WeepingCurtain {0.78}
                    else {1.0};
                let r=(base_radius*lobe_scale*
                    (0.82+0.35*rand(6+cluster as u32))).max(s*0.82);
                let rz=match self.preset {
                    PlantPreset::Pine|PlantPreset::Larch|PlantPreset::Redwood|
                    PlantPreset::BaldCypress|PlantPreset::MediterraneanCypress=>r*1.75,
                    PlantPreset::Willow=>r*1.30,
                    PlantPreset::Birch|PlantPreset::Pear=>r*1.12,
                    PlantPreset::GreenBamboo|PlantPreset::BrownBamboo=>r*1.25,
                    PlantPreset::Shrub|PlantPreset::Pomegranate=>r*0.65,
                    _=>r*0.85,
                };
                let offset_z = match self.preset {
                    PlantPreset::Willow => -0.28 * rz,
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
                    self.seed,i*8+cluster as u32);
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
        if self.leaf_density>0.0 && spans.iter().any(|s|s.branch_id!=0)
            && matches!(self.preset,
            PlantPreset::Oak|PlantPreset::Walnut|PlantPreset::Ebony|
            PlantPreset::Maple|PlantPreset::CrimsonKingMaple|
            PlantPreset::Mango|PlantPreset::Lychee|
            PlantPreset::RedApple|PlantPreset::PinkApple|PlantPreset::YellowApple|
            PlantPreset::Cherry|PlantPreset::CherryBlossom|PlantPreset::Orange|
            PlantPreset::EasternCottonwood|PlantPreset::FremontCottonwood|
            PlantPreset::BlackCottonwood) {
            // Hide the otherwise exposed top of the full-height trunk within
            // a connected crown cap. No independent decorative leaf objects.
            let mut crown_cap=BTreeMap::new();
            let r=(self.leaf_scale*self.overall_scale*2.8).max(s*1.2);
            paint_leaf_cluster(&mut crown_cap,shape,s,trunk(0.90),r,r*0.78,
                self.leaf_density,self.leaf_variation,self.leaf_angle,
                self.seed,self.branch_count*8+1);
            for (cell,material) in crown_cap {
                if let std::collections::btree_map::Entry::Vacant(entry)=cells.entry(cell) {
                    entry.insert(material);
                    ownership.insert(cell,(TreePart::Foliage,Some(0)));
                }
            }
        }
        if !matches!(self.preset,
            PlantPreset::CoconutPalm|PlantPreset::DatePalm|PlantPreset::FanPalm|
            PlantPreset::GreenBamboo|PlantPreset::BrownBamboo) {
            apply_pipe_model(&mut spans,&branches,&mut cells,&mut ownership,
                trunk_radius,self.wood_pipe_exponent,self.wood_pipe_blend);
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
            spans,
        };
        let voxels:Vec<VoxelCell> = cells.iter().map(|(pos, material)| VoxelCell {
            x: pos[0], y: pos[1], z: pos[2], material: *material,
        }).collect();
        structure.validate(&voxels)?;
        let triangles = tree_surface_triangles(&cells,shape,s,self.leaf_render,
            Some(&structure));
        Ok(FoliageMesh { voxels, triangles, shape,
            voxel_size: s, height: h, leaf_render: self.leaf_render,
            structure:Some(structure) })
    }
}

/// A bottom-up pipe-model radius pass using the *actual occupied foliage
/// hexels* as support demand. Decorative card count never enters the model.
/// Existing species thickness is the authored baseline: blend and bounded
/// 0.65–1.35 scaling prevent this appearance pass from erasing silhouettes.
/// Any additional wood volume is claimed into the authoritative grid.
fn apply_pipe_model(
    spans:&mut [BranchSpan], branches:&[TreeBranch],
    cells:&mut BTreeMap<[i32;3],Material>,
    owners:&mut BTreeMap<[i32;3],(TreePart,Option<u32>)>,
    base_radius:f32, exponent:f32, blend:f32,
) {
    if blend<=0.0 || branches.len()<=1 {return;}
    let mut load:BTreeMap<u32,f32>=branches.iter().map(|b|(b.id,1.0)).collect();
    for (part,owner) in owners.values() {
        if *part==TreePart::Foliage {
            if let Some(id)=owner {
                if let Some(weight)=load.get_mut(id) {*weight+=1.0;}
            }
        }
    }
    // Parent IDs always precede children, including optional extra twigs.
    for branch in branches.iter().rev() {
        if let Some(parent)=branch.parent {
            let child=load.get(&branch.id).copied().unwrap_or(1.0);
            *load.entry(parent).or_default()+=child;
        }
    }
    let root_load=load.get(&0).copied().unwrap_or(1.0).max(1.0);
    let parts:BTreeMap<_,_>=branches.iter().map(|b|(b.id,b.part)).collect();
    for span in spans.iter_mut().filter(|s|s.branch_id!=0) {
        let support=load.get(&span.branch_id).copied().unwrap_or(1.0);
        let target=base_radius*0.85*(support/root_load).powf(1.0/exponent);
        let old=span.start_radius_mm as f32/1000.0;
        let wanted=old*(1.0-blend)+target*blend;
        let factor=(wanted/old).clamp(0.65,1.35);
        let start=(old*factor).clamp(0.026,0.5);
        let end=(span.end_radius_mm as f32/1000.0*factor)
            .clamp(0.020,start*0.96);
        let grew=start>old+0.001 || end>span.end_radius_mm as f32/1000.0+0.001;
        span.start_radius_mm=(start*1000.0).round() as u16;
        span.end_radius_mm=(end*1000.0).round() as u16;
        if grew {
            let a=span.from_mm.map(|v|v as f32/1000.0);
            let b=span.to_mm.map(|v|v as f32/1000.0);
            let mut new_wood=BTreeMap::new();
            paint_line(&mut new_wood,VoxelShape::HexPrism,WORLD_HEXEL_SIZE_METRES,
                a,b,start.max(end),Material::Bark);
            for (cell,material) in new_wood {
                let old=cells.insert(cell,material);
                if old.is_none() || old.is_some_and(Material::is_foliage) {
                    owners.insert(cell,(parts[&span.branch_id],Some(span.branch_id)));
                }
            }
        }
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
    // The endpoint of a thin twig can be farther from the center of its
    // owning 1m cell than the visible twig radius. Make its structural
    // anchor occupied even when the spherical brush misses that center.
    if material.is_wood() {
        cells.insert(grid_key(shape,s,a),material);
        cells.insert(grid_key(shape,s,b),material);
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
                // Preserve an almost solid canopy interior and concentrate
                // silhouette irregularity toward the outer surface. The
                // density slider still controls occupancy at low values.
                let edge = 0.95 + variation * (jitter - 0.5) * 0.34;
                let chance=(density + (1.0-dist).max(0.0)*0.35).min(1.0);
                if dist > edge.max(0.35) || jitter > chance { continue; }
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
/// Each logical 1 m leaf hexel hosts several separate textured sprays.
/// Their staggered placement increases canopy coverage without creating
/// more harvestable cells, extra entities or non-authoritative foliage.
fn add_leaf_cards(triangles:&mut Vec<Triangle>, pos:[i32;3],
                  material:Material, shape:VoxelShape, size:f32) {
    let c=grid_position(shape,size,pos);
    let id=(pos[0] as u32).wrapping_mul(0x9e3779b9)
        ^(pos[1] as u32).wrapping_mul(0x85ebca6b)
        ^(pos[2] as u32).wrapping_mul(0xc2b2ae35);
    let count=6; // 6 independently oriented, double-sided leaf sprays.
    for plane in 0..count {
        let seed=id ^ (plane as u32).wrapping_mul(0x27d4eb2d);
        let angle=TAU*hash01(seed,1,3);
        let width=size*(0.34+0.09*hash01(seed,2,5));
        let dx=size*(hash01(seed,3,9)-0.5)*0.42;
        let dy=size*(hash01(seed,4,11)-0.5)*0.42;
        let z=size*(hash01(seed,5,13)-0.5)*0.30;
        let drift=size*(hash01(seed,6,17)-0.5)*0.18;
        let d=[angle.cos()*width,angle.sin()*width];
        let z0=c[2]+z-size*0.39;
        let z1=c[2]+z+size*0.39;
        let quad=[
            [c[0]+dx-d[0],c[1]+dy-d[1],z0],
            [c[0]+dx+d[0],c[1]+dy+d[1],z0],
            [c[0]+dx+d[0]+drift,c[1]+dy+d[1]-drift,z1],
            [c[0]+dx-d[0]+drift,c[1]+dy-d[1]-drift,z1],
        ];
        let uv=[[0.0,1.0],[1.0,1.0],[1.0,0.0],[0.0,0.0]];
        let weights=[0.0,0.0,1.0,1.0];
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

/// Unit vector for the wood-cylinder frame.
fn normalize3(v:[f32;3])->[f32;3] {
    let n=(v[0]*v[0]+v[1]*v[1]+v[2]*v[2]).sqrt().max(1e-8);
    [v[0]/n,v[1]/n,v[2]/n]
}
fn cross3(a:[f32;3],b:[f32;3])->[f32;3] {
    [a[1]*b[2]-a[2]*b[1],
     a[2]*b[0]-a[0]*b[2],
     a[0]*b[1]-a[1]*b[0]]
}

/// Choose the adjacent axial or vertical cell in the normal's direction.
/// The editing tools use this for their Add action on curved branch surfaces.
fn branch_outward(normal:[f32;3])->[i32;3] {
    const NEIGHBORS:[[i32;3];8]=[
        [0,1,0],[-1,1,0],[-1,0,0],[0,-1,0],
        [1,-1,0],[1,0,0],[0,0,1],[0,0,-1]
    ];
    let mut best=[0,0,1];
    let mut score=f32::NEG_INFINITY;
    for d in NEIGHBORS {
        let world=[d[0] as f32 + 0.5*d[1] as f32,
            0.8660254*d[1] as f32,d[2] as f32];
        let dot=world[0]*normal[0]+world[1]*normal[1]+world[2]*normal[2];
        if dot>score {score=dot;best=d;}
    }
    best
}

/// For the thinnest twigs use two crossed, tapered bark-colored ribbons
/// rather than a full 1m prism. These are geometry inside a real wood cell;
/// they do not become separate, unharvestable decorative entities.
fn add_twig_ribbons(out:&mut Vec<Triangle>,cell:[i32;3],
                    a:[f32;3],b:[f32;3],r0:f32,r1:f32,
                    u:[f32;3],v:[f32;3]) {
    for axis in [u,v] {
        let quad=[
            add3(a,mul3(axis,-r0)),add3(a,mul3(axis,r0)),
            add3(b,mul3(axis,r1)),add3(b,mul3(axis,-r1)),
        ];
        let dir=sub3(b,a);
        let normal=normalize3(cross3(dir,axis));
        let outward=branch_outward(normal);
        let reverse=branch_outward(mul3(normal,-1.0));
        let uv=[[0.0,1.0],[1.0,1.0],[1.0,0.0],[0.0,0.0]];
        for (idx,face_out) in [
            ([0,1,2],outward),([0,2,3],outward),
            ([2,1,0],reverse),([3,2,0],reverse)
        ] {
            push_triangle(out,quad[idx[0]],quad[idx[1]],quad[idx[2]],
                Material::Bark,cell,face_out,idx.map(|i|uv[i]));
        }
    }
}

/// Six-sided tapered wood section restricted to exactly one occupied branch
/// cell. Chopping that cell suppresses its corresponding triangles.
fn add_tapered_branch_section(out:&mut Vec<Triangle>,cell:[i32;3],
                              a:[f32;3],b:[f32;3],r0:f32,r1:f32) {
    let dir=sub3(b,a);
    let len=(dir[0]*dir[0]+dir[1]*dir[1]+dir[2]*dir[2]).sqrt();
    if len<=0.0001 {return;}
    let tangent=normalize3(dir);
    let axis=if tangent[2].abs()<0.9 {[0.0,0.0,1.0]} else {[0.0,1.0,0.0]};
    let u=normalize3(cross3(tangent,axis));
    let v=cross3(tangent,u);
    if r0.max(r1) <= 0.085 {
        add_twig_ribbons(out,cell,a,b,r0,r1,u,v);
        return;
    }
    const SIDES:usize=6;
    let ring=|center:[f32;3],radius:f32| {
        std::array::from_fn::<_,SIDES,_>(|i| {
            let angle=TAU*i as f32/SIDES as f32;
            add3(center,add3(mul3(u,angle.cos()*radius),
                             mul3(v,angle.sin()*radius)))
        })
    };
    let start=ring(a,r0);
    let end=ring(b,r1);
    let sides=SIDES as f32;
    for i in 0..SIDES {
        let next=(i+1)%SIDES;
        let uv0=[i as f32/sides,0.0];
        let uv1=[(i+1) as f32/sides,0.0];
        let uv2=[(i+1) as f32/sides,len];
        let uv3=[i as f32/sides,len];
        let normal=normalize3(add3(sub3(start[i],a),sub3(start[next],a)));
        let outward=branch_outward(normal);
        push_triangle(out,start[i],start[next],end[next],Material::Bark,cell,
            outward,[uv0,uv1,uv2]);
        push_triangle(out,start[i],end[next],end[i],Material::Bark,cell,
            outward,[uv0,uv2,uv3]);
    }
    let start_out=branch_outward(mul3(tangent,-1.0));
    let end_out=branch_outward(tangent);
    for i in 1..SIDES-1 {
        push_triangle(out,start[0],start[i+1],start[i],Material::Bark,cell,
            start_out,[[0.5,0.5],[0.0,0.0],[1.0,0.0]]);
        push_triangle(out,end[0],end[i],end[i+1],Material::Bark,cell,
            end_out,[[0.5,0.5],[0.0,0.0],[1.0,0.0]]);
    }
}

/// Clip a visible wood fragment to every intersected *occupied* hexel.
/// The clipped polygon inherits the world-cell owner for deletion, picking,
/// rendering and OBJ export. No mesh fragment can survive an erased wood cell.
fn emit_owned_wood(
    out:&mut Vec<Triangle>, raw:Triangle,
    cells:&BTreeMap<[i32;3],Material>,
    roles:&BTreeMap<[i32;3],TreePart>,
    size:f32, visualized:&mut BTreeSet<[i32;3]>,
) {
    use crate::hexel_clip::{Vertex, triangle_candidates, clip_to_hex_prism};
    let source=std::array::from_fn::<_,3,_>(|i| Vertex {
        position:raw.points[i],uv:raw.uv[i],
    });
    // Convexity gives us an inexpensive exact fast path: all three corners
    // inside one hexel implies the entire triangle lies inside it.
    let nearest=grid_key(VoxelShape::HexPrism,size,raw.points[0]);
    if cells.get(&nearest)==Some(&Material::Bark)
        && matches!(roles.get(&nearest),
            Some(TreePart::Trunk|TreePart::MajorBranch|TreePart::SmallBranch))
        && raw.points.iter().all(|p|
            crate::hexel_clip::contains(*p,nearest,size)) {
        visualized.insert(nearest);
        out.push(Triangle{cell:nearest,..raw});
        return;
    }
    for cell in triangle_candidates(source,size) {
        // Artist-painted blocks are separate solids, never replaced by an
        // old generated path. Other generated limbs may share the same cell.
        if cells.get(&cell)!=Some(&Material::Bark)
            || !matches!(roles.get(&cell),
                Some(TreePart::Trunk|TreePart::MajorBranch|TreePart::SmallBranch)) {
            continue;
        }
        for piece in clip_to_hex_prism(source,cell,size) {
            visualized.insert(cell);
            out.push(Triangle {
                points:piece.map(|v|v.position),
                uv:piece.map(|v|v.uv),
                cell, ..raw.clone()
            });
        }
    }
}

/// The same rings are reused on both sides of each curve section, with a
/// transported local frame rather than choosing a new axis at every joint.
/// Bark UVs grow with distance and do not reset at branch knees.
fn emit_continuous_span_chain(
    out:&mut Vec<Triangle>, segments:&[&BranchSpan],
    cells:&BTreeMap<[i32;3],Material>,
    roles:&BTreeMap<[i32;3],TreePart>,
    size:f32, visualized:&mut BTreeSet<[i32;3]>,
) {
    if segments.is_empty(){return;}
    let mut positions:Vec<[f32;3]>=Vec::new();
    let mut radii:Vec<f32>=Vec::new();
    // A restrained branch collar around the parent attachment.
    // This is render-only, and can never extend into an unowned cell.
    let collar=if segments[0].branch_id==0 {0.0} else {0.07};
    let mut travelled=0.0_f32;
    for span in segments {
        let a=span.from_mm.map(|x|x as f32/1000.0);
        let b=span.to_mm.map(|x|x as f32/1000.0);
        let d=sub3(b,a);
        let distance=d.iter().map(|v|v*v).sum::<f32>().sqrt();
        if distance<0.001 {continue;}
        // Subdivide long spans before clipping. Shorter quads avoid crossing
        // many hex boundaries and keep the candidate-cell search bounded.
        let steps=(distance/0.70).ceil().max(1.0) as usize;
        for i in 0..steps {
            let t=i as f32/steps as f32;
            positions.push(lerp3(a,b,t));
            let r=((1.0-t)*span.start_radius_mm as f32+
                t*span.end_radius_mm as f32)/1000.0;
            let swelling=1.0+collar*(1.0-(travelled+t*distance)/0.75).max(0.0);
            radii.push(r*swelling);
        }
        travelled+=distance;
    }
    let Some(last)=segments.last() else {return;};
    positions.push(last.to_mm.map(|x|x as f32/1000.0));
    radii.push(last.end_radius_mm as f32/1000.0);
    if positions.len()<2{return;}
    let mut tangents=Vec::with_capacity(positions.len());
    let mut normals=Vec::with_capacity(positions.len());
    let mut binormals=Vec::with_capacity(positions.len());
    let mut previous_normal:Option<[f32;3]>=None;
    for i in 0..positions.len() {
        let d=if i==0 {sub3(positions[1],positions[0])}
        else if i+1==positions.len() {sub3(positions[i],positions[i-1])}
        else {sub3(positions[i+1],positions[i-1])};
        let tangent=normalize3(d);
        let reference=previous_normal.unwrap_or_else(||{
            if tangent[2].abs()<0.90 {[0.0,0.0,1.0]} else {[0.0,1.0,0.0]}
        });
        // Project the previous frame into the new tangent plane. For
        // near-parallel tangents this is a minimal-twist transport.
        let dot=reference.iter().zip(tangent).map(|(a,b)|a*b).sum::<f32>();
        let projected=sub3(reference,mul3(tangent,dot));
        let norm=projected.iter().map(|v|v*v).sum::<f32>().sqrt();
        let u=if norm<0.0001 {
            let axis=if tangent[2].abs()<0.90 {[0.0,0.0,1.0]}
                else {[0.0,1.0,0.0]};
            normalize3(cross3(tangent,axis))
        } else {mul3(projected,1.0/norm)};
        let v=normalize3(cross3(tangent,u));
        tangents.push(tangent);
        normals.push(u);
        binormals.push(v);
        previous_normal=Some(u);
    }
    let mut uv_v=0.0;
    const SIDES:usize=6;
    for i in 0..positions.len()-1 {
        let a=positions[i];
        let b=positions[i+1];
        let len=sub3(b,a).iter().map(|v|v*v).sum::<f32>().sqrt();
        if len<0.00001 {continue;}
        let mut raw=Vec::new();
        if radii[i].max(radii[i+1])<=0.085 {
            // Fine twigs remain crossed bark ribbons, still clipped to their
            // owning wood hexels instead of creating non-harvestable geometry.
            add_twig_ribbons(&mut raw,[0,0,0],a,b,
                radii[i],radii[i+1],normals[i],binormals[i]);
        } else {
            let ring=|j:usize,center:[f32;3],radius:f32| {
                let angle=TAU*j as f32/SIDES as f32;
                add3(center,add3(mul3(normals[i],angle.cos()*radius),
                    mul3(binormals[i],angle.sin()*radius)))
            };
            let next_ring=|j:usize,center:[f32;3],radius:f32| {
                let angle=TAU*j as f32/SIDES as f32;
                add3(center,add3(mul3(normals[i+1],angle.cos()*radius),
                    mul3(binormals[i+1],angle.sin()*radius)))
            };
            for j in 0..SIDES {
                let k=(j+1)%SIDES;
                let q0=ring(j,a,radii[i]);
                let q1=ring(k,a,radii[i]);
                let q2=next_ring(k,b,radii[i+1]);
                let q3=next_ring(j,b,radii[i+1]);
                let outward=branch_outward(normalize3(sub3(q0,a)));
                let u0=j as f32/SIDES as f32;
                let u1=(j+1) as f32/SIDES as f32;
                push_triangle(&mut raw,q0,q1,q2,Material::Bark,[0,0,0],
                    outward,[[u0,uv_v],[u1,uv_v],[u1,uv_v+len]]);
                push_triangle(&mut raw,q0,q2,q3,Material::Bark,[0,0,0],
                    outward,[[u0,uv_v],[u1,uv_v+len],[u0,uv_v+len]]);
            }
            // Cap *true* path tips only, not every artificial subdivision.
            // A later world harvesting system may add exposed cut scars.
            if i==0 {
                for j in 1..SIDES-1 {
                    push_triangle(&mut raw,ring(0,a,radii[i]),
                        ring(j+1,a,radii[i]),ring(j,a,radii[i]),
                        Material::Bark,[0,0,0],branch_outward(mul3(tangents[i],-1.0)),
                        [[0.0,0.0],[1.0,0.0],[0.5,0.5]]);
                }
            }
            if i+2==positions.len() {
                for j in 1..SIDES-1 {
                    push_triangle(&mut raw,next_ring(0,b,radii[i+1]),
                        next_ring(j,b,radii[i+1]),
                        next_ring(j+1,b,radii[i+1]),Material::Bark,[0,0,0],
                        branch_outward(tangents[i+1]),
                        [[0.0,0.0],[0.5,0.5],[1.0,0.0]]);
                }
            }
        }
        for tri in raw {emit_owned_wood(out,tri,cells,roles,size,visualized);}
        uv_v+=len;
    }
}

/// Common organic render surface for Foliage, editable Assets, GPU preview and
/// OBJ export. The actual 1m occupied hexels are never regenerated here.
/// Legacy saves without a trunk spine keep their original solid trunk.
fn tree_surface_triangles(
    cells:&BTreeMap<[i32;3],Material>,shape:VoxelShape,size:f32,
    render:LeafRenderSettings,structure:Option<&TreeStructure>,
)->Vec<Triangle> {
    let Some(tree)=structure.filter(|tree| !tree.spans.is_empty()
        && shape==VoxelShape::HexPrism) else {
        return surface_triangles(cells,shape,size,render);
    };
    let roles:BTreeMap<_,_>=tree.cells.iter().map(|part|(part.cell,part.part)).collect();
    let smooth_trunk=tree.spans.iter().any(|span|span.branch_id==0);
    let mut solids=cells.clone();
    for (cell,material) in cells {
        if material.is_wood() && matches!(roles.get(cell),
            Some(TreePart::MajorBranch|TreePart::SmallBranch)) ||
            (material.is_wood() && smooth_trunk &&
                roles.get(cell)==Some(&TreePart::Trunk)) {
            solids.remove(cell);
        }
    }
    let mut triangles=surface_triangles(&solids,shape,size,render);
    let mut visualized=BTreeSet::new();
    // Source span order follows parent-before-child generation and is stable
    // across source JSON, Tree Designer, Asset conversion and OBJ export.
    let mut paths:BTreeMap<u32,Vec<&BranchSpan>>=BTreeMap::new();
    for span in &tree.spans {
        paths.entry(span.branch_id).or_default().push(span);
    }
    for path in paths.values() {
        emit_continuous_span_chain(&mut triangles,path,cells,&roles,size,
            &mut visualized);
    }
    // Diagonal wood-hexel bridging may introduce occupied cells outside the
    // source tube's sampled surface. Give only these residual owned branch
    // hexels a thin *clipped* connector, never invisible physical material.
    for part in &tree.cells {
        if !matches!(part.part,TreePart::MajorBranch|TreePart::SmallBranch)
            || visualized.contains(&part.cell)
            || cells.get(&part.cell)!=Some(&Material::Bark) {continue;}
        let Some(owner)=part.branch_id else {continue;};
        let center=grid_position(shape,size,part.cell);
        let mut nearest=None;
        let mut best=f32::INFINITY;
        for span in tree.spans.iter().filter(|span|span.branch_id==owner) {
            let a=span.from_mm.map(|x|x as f32/1000.0);
            let b=span.to_mm.map(|x|x as f32/1000.0);
            let dir=sub3(b,a);
            let len2=dir.iter().map(|x|x*x).sum::<f32>().max(1e-8);
            let off=sub3(center,a);
            let t=(off.iter().zip(dir.iter()).map(|(a,b)|a*b).sum::<f32>()
                /len2).clamp(0.0,1.0);
            let point=lerp3(a,b,t);
            let delta=sub3(center,point);
            let distance=delta.iter().map(|x|x*x).sum::<f32>();
            if distance<best {
                best=distance;
                nearest=Some((point,normalize3(dir)));
            }
        }
        if let Some((point,dir))=nearest {
            let (a,b)=if best<0.005 {
                (add3(center,mul3(dir,-0.30)),add3(center,mul3(dir,0.30)))
            } else {(point,center)};
            let mut raw=Vec::new();
            add_tapered_branch_section(&mut raw,[0,0,0],a,b,0.07,0.045);
            for t in raw {
                // A bridge-cell fallback is never allowed to cross the
                // specified ownership boundary.
                use crate::hexel_clip::{Vertex,clip_to_hex_prism};
                let input=std::array::from_fn(|i|Vertex{
                    position:t.points[i],uv:t.uv[i]});
                for piece in clip_to_hex_prism(input,part.cell,size) {
                    triangles.push(Triangle{
                        points:piece.map(|v|v.position),
                        uv:piece.map(|v|v.uv),
                        cell:part.cell,..t.clone()
                    });
                }
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

    /// Rebuild visible triangles after changing a tree's occupied cells.
    /// Preserved branch spans cannot resurrect deleted or repainted hexels.
    pub fn with_tree_structure(mut self,structure:Option<TreeStructure>)->Self {
        if let Some(tree)=&structure {
            let cells:BTreeMap<_,_>=self.voxels.iter()
                .map(|v|([v.x,v.y,v.z],v.material)).collect();
            self.triangles=tree_surface_triangles(&cells,self.shape,
                self.voxel_size,self.leaf_render,Some(tree));
        }
        self.structure=structure;
        self
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
    fn pipe_model_uses_foliage_support_without_breaking_legacy_projects() {
        let mut p=FoliageProject::from_preset(PlantPreset::Oak);
        p.wood_pipe_blend=0.0;
        let original=p.generate().unwrap();
        p.wood_pipe_blend=1.0;
        p.wood_pipe_exponent=2.0;
        let adapted=p.generate().unwrap();
        assert_ne!(original.structure.as_ref().unwrap().spans,
            adapted.structure.as_ref().unwrap().spans);
        assert_eq!(original.structure.as_ref().unwrap()
            .spans.iter().filter(|s|s.branch_id==0).collect::<Vec<_>>(),
            adapted.structure.as_ref().unwrap()
            .spans.iter().filter(|s|s.branch_id==0).collect::<Vec<_>>(),
            "Pipe model must not change the authored trunk");
        adapted.structure.as_ref().unwrap().validate(&adapted.voxels).unwrap();
        let json=serde_json::to_string(&p).unwrap();
        let reloaded: FoliageProject=serde_json::from_str(&json).unwrap();
        assert_eq!(reloaded.wood_pipe_blend,1.0);
        assert_eq!(reloaded.wood_pipe_exponent,2.0);
        let mut old:serde_json::Value=serde_json::from_str(&json).unwrap();
        old.as_object_mut().unwrap().remove("wood_pipe_blend");
        old.as_object_mut().unwrap().remove("wood_pipe_exponent");
        let old: FoliageProject=serde_json::from_value(old).unwrap();
        old.validate().unwrap();
        assert_eq!(old.wood_pipe_blend,0.35);
        assert_eq!(old.wood_pipe_exponent,2.3);
    }

    #[test]
    fn organic_wood_triangles_never_leave_authoritative_hexels() {
        for preset in [PlantPreset::Oak,PlantPreset::Pine,
                       PlantPreset::FremontCottonwood,PlantPreset::CaturraCoffee,
                       PlantPreset::CoconutPalm,PlantPreset::GreenBamboo] {
            let mesh=FoliageProject::from_preset(preset).generate().unwrap();
            let structure=mesh.structure.as_ref().unwrap();
            assert!(structure.spans.iter().any(|s|s.branch_id==0),
                "{preset:?}: missing smooth trunk source");
            let wood:BTreeSet<_>=mesh.voxels.iter().filter(|v|v.material.is_wood())
                .map(|v|[v.x,v.y,v.z]).collect();
            let mut count=0;
            for tri in mesh.triangles.iter().filter(|t|t.material.is_wood()) {
                assert!(wood.contains(&tri.cell),"{preset:?} visual ghost");
                for position in tri.points {
                    assert!(crate::hexel_clip::contains(position,tri.cell,mesh.voxel_size),
                        "{preset:?}: triangle escapes {:?} at {position:?}",tri.cell);
                }
                count+=1;
            }
            assert!(count>50,"{preset:?}: no organic wood");
        }
    }

    #[test]
    fn authored_hexel_deletion_and_painting_never_resurrect_branch_surfaces() {
        use crate::asset::VoxelAsset;
        let project=FoliageProject::from_preset(PlantPreset::Oak);
        let mesh=project.generate().unwrap();
        let mut asset=VoxelAsset::from_tree(&project,&mesh);
        let remove=mesh.triangles.iter().find(|t|
            t.material.is_wood() && mesh.structure.as_ref().unwrap().cells
            .iter().any(|c|c.cell==t.cell&&c.part==TreePart::MajorBranch))
            .expect("test Oak has visible branch");
        let position=remove.cell;
        assert_eq!(asset.harvest_cell(position),Some(Material::Bark));
        let trimmed=asset.mesh().unwrap();
        assert!(!trimmed.triangles.iter()
            .any(|t|t.cell==position&&t.material.is_wood()));
        // Painting a different owned branch cell as leaf must not retain the
        // old branch geometry, even when its historical span still exists.
        let another=mesh.triangles.iter().find(|t|
            t.material.is_wood() && t.cell!=position &&
            mesh.structure.as_ref().unwrap().cells.iter()
                .any(|c|c.cell==t.cell&&c.part==TreePart::MajorBranch))
            .unwrap().cell;
        assert!(asset.paint(another,Material::Leaf));
        assert!(!asset.mesh().unwrap().triangles.iter()
            .any(|t|t.cell==another&&t.material.is_wood()));
        let loaded=VoxelAsset::from_json(&asset.to_json().unwrap()).unwrap();
        assert_eq!(loaded.mesh().unwrap().obj(),asset.mesh().unwrap().obj());
        assert!(!loaded.contains(position));
    }

    #[test]
    fn older_tree_structure_without_trunk_spine_retains_original_trunk_blocks() {
        let project=FoliageProject::from_preset(PlantPreset::Birch);
        let mesh=project.generate().unwrap();
        let mut legacy=mesh.structure.clone().unwrap();
        legacy.spans.retain(|span|span.branch_id!=0);
        legacy.validate(&mesh.voxels).unwrap();
        let rebuild=FoliageMesh::from_voxels_with_render(
            mesh.shape,mesh.voxel_size,mesh.voxels.clone(),mesh.leaf_render)
            .unwrap().with_tree_structure(Some(legacy));
        let trunk=mesh.structure.as_ref().unwrap().cells.iter()
            .find(|p|p.part==TreePart::Trunk).unwrap().cell;
        assert!(rebuild.triangles.iter().any(|t|
            t.cell==trunk&&t.material.is_wood()));
        assert_eq!(rebuild.voxels,mesh.voxels);
    }


    #[test]
    fn tree_growth_presets_are_saved_loaded_and_change_the_generated_tree() {
        for preset in PlantPreset::TREE_CATALOG {
            let mut tree=FoliageProject::from_preset(preset);
            let default_structure=tree.generate().unwrap().structure.unwrap();
            let original=serde_json::to_value(&tree).unwrap();
            assert!(original.get("growth_rules_override").is_none());
            assert!(original.get("trunk_rules_override").is_none());

            let mut branch=tree.effective_growth_rules();
            branch.reach_factor*=1.10;
            tree.set_growth_rules(branch).unwrap();
            let mut trunk=tree.effective_trunk_rules();
            trunk.taper=(trunk.taper+0.06).min(0.85);
            tree.set_trunk_rules(trunk).unwrap();
            assert_ne!(tree.generate().unwrap().structure.unwrap().spans,
                default_structure.spans, "{preset:?}: editable growth controls had no effect");

            let json=serde_json::to_string(&tree).unwrap();
            let roundtrip: FoliageProject=serde_json::from_str(&json).unwrap();
            roundtrip.validate().unwrap();
            assert_eq!(roundtrip.effective_growth_rules(),branch);
            assert_eq!(roundtrip.effective_trunk_rules(),trunk);
            assert_eq!(roundtrip.generate().unwrap().voxels,
                tree.generate().unwrap().voxels);

            tree.reset_growth_rules();
            assert!(tree.growth_rules_override.is_none());
            assert!(tree.trunk_rules_override.is_none());
            assert_eq!(tree.effective_growth_rules(),preset.growth_rules());
            assert_eq!(tree.effective_trunk_rules(),preset.trunk_growth_rules());
        }
    }

    #[test]
    fn invalid_growth_overrides_rejected_and_old_projects_still_load() {
        let mut tree=FoliageProject::from_preset(PlantPreset::Willow);
        let old_json=serde_json::to_string(&tree).unwrap();
        let restored: FoliageProject=serde_json::from_str(&old_json).unwrap();
        restored.validate().unwrap();
        let mut rules=tree.effective_growth_rules();
        rules.reach_factor=1.5;
        assert!(tree.set_growth_rules(rules).is_err());
        let mut rules=tree.effective_growth_rules();
        rules.primary_pitch[1]=1.0;
        tree.growth_rules_override=Some(rules);
        assert!(tree.validate().is_err());
        let mut trunk=tree.preset.trunk_growth_rules();
        trunk.taper=f32::NAN;
        assert!(tree.set_trunk_rules(trunk).is_err());
    }

    #[test]
    fn tree_catalog_contains_42_entries_including_coffee_and_cottonwoods() {
        let expected=[
            "Acacia","Birch","Ebony","Kapok","Maple","Crimson King Maple",
            "Oak","Purpleheart","Walnut","Willow","Cherry Blossom",
            "Kentucky Coffeetree","Eastern Cottonwood","Fremont Cottonwood",
            "Black Cottonwood",
            "Bald Cypress","Mediterranean Cypress","Larch","Pine","Redwood",
            "Red Apple","Pink Apple","Yellow Apple","Pear","Olive","Peach",
            "Cherry","Mango","Orange","Lychee","Breadfruit","Pomegranate",
            "Arabica Coffee","Caturra Coffee","Maragogipe Coffee",
            "Castillo Coffee","Tabi Coffee",
            "Green Bamboo","Brown Bamboo",
            "Coconut Palm","Date Palm","Fan Palm",
        ];
        let actual:Vec<_>=PlantPreset::TREE_CATALOG.iter()
            .map(|p|p.name()).collect();
        assert_eq!(actual,expected);
        assert_eq!(PlantPreset::ALL.len(),43); // Shrub is not a tree entry
        assert_eq!(PlantPreset::TREE_CATALOG.iter()
            .filter(|p|p.group()=="Hardwood & broadleaf").count(),15);
        assert_eq!(PlantPreset::TREE_CATALOG.iter()
            .filter(|p|p.group()=="Conifer & evergreen").count(),5);
        assert_eq!(PlantPreset::TREE_CATALOG.iter()
            .filter(|p|p.group()=="Fruit trees").count(),12);
        assert_eq!(PlantPreset::TREE_CATALOG.iter()
            .filter(|p|p.group()=="Bamboo").count(),2);
        assert_eq!(PlantPreset::TREE_CATALOG.iter()
            .filter(|p|p.group()=="Palms").count(),3);
        assert_eq!(PlantPreset::TREE_CATALOG.iter()
            .filter(|p|p.group()=="Coffee plants").count(),5);
        for preset in PlantPreset::ALL {
            let json=serde_json::to_string(&preset).unwrap();
            assert_eq!(serde_json::from_str::<PlantPreset>(&json).unwrap(),preset);
        }
        assert_eq!(PlantPreset::RedApple.fruit_color(),Some([193,42,38]));
        assert_eq!(PlantPreset::PinkApple.fruit_color(),Some([244,123,151]));
        assert_eq!(PlantPreset::YellowApple.fruit_color(),Some([237,198,45]));
        assert_eq!(PlantPreset::CherryBlossom.fruit_color(),None);
        assert!(PlantPreset::ArabicaCoffee.is_coffee());
        assert!(!PlantPreset::KentuckyCoffeetree.is_coffee());
        assert_eq!(PlantPreset::KentuckyCoffeetree.fruit_color(),None);
        assert!(PlantPreset::FremontCottonwood.is_cottonwood());
    }

    #[test]
    fn coffee_cultivars_and_cottonwood_are_distinct_seeded_trees() {
        let presets=[
            PlantPreset::ArabicaCoffee,PlantPreset::CaturraCoffee,
            PlantPreset::MaragogipeCoffee,PlantPreset::CastilloCoffee,
            PlantPreset::TabiCoffee,PlantPreset::KentuckyCoffeetree,
            PlantPreset::EasternCottonwood,PlantPreset::FremontCottonwood,
            PlantPreset::BlackCottonwood,
        ];
        for preset in presets {
            let project=FoliageProject::from_preset(preset);
            let mesh=project.generate().unwrap();
            let structure=mesh.structure.as_ref().unwrap();
            structure.validate(&mesh.voxels).unwrap();
            assert!(mesh.voxels.iter().any(|v|v.material.is_wood()));
            assert!(mesh.voxels.iter().any(|v|v.material.is_foliage()));
            assert_eq!(mesh.voxels,project.generate().unwrap().voxels);
            let json=serde_json::to_string(&project).unwrap();
            let loaded: FoliageProject=serde_json::from_str(&json).unwrap();
            assert_eq!(loaded.generate().unwrap().voxels,mesh.voxels);
        }
        let h=|p|FoliageProject::from_preset(p).height;
        assert!(h(PlantPreset::CaturraCoffee)<h(PlantPreset::ArabicaCoffee));
        assert!(h(PlantPreset::CastilloCoffee)<h(PlantPreset::TabiCoffee));
        assert!(h(PlantPreset::MaragogipeCoffee)>h(PlantPreset::ArabicaCoffee));
        let cutout=|p|leaf_cutout_for_species(p).chunks_exact(4)
            .map(|px|px[3]).collect::<Vec<_>>();
        assert_ne!(cutout(PlantPreset::MaragogipeCoffee),
            cutout(PlantPreset::CaturraCoffee));
        assert_ne!(cutout(PlantPreset::KentuckyCoffeetree),
            cutout(PlantPreset::Walnut));
        assert_ne!(cutout(PlantPreset::EasternCottonwood),
            cutout(PlantPreset::BlackCottonwood));
        assert_ne!(cutout(PlantPreset::EasternCottonwood),
            cutout(PlantPreset::FremontCottonwood));
        assert!(FoliageProject::from_preset(PlantPreset::EasternCottonwood)
            .leaf_render.wind_strength>
                FoliageProject::from_preset(PlantPreset::Oak)
                    .leaf_render.wind_strength);
    }

    #[test]
    fn palms_have_unbranched_trunks_and_distinct_frond_crowns() {
        for species in [PlantPreset::CoconutPalm,PlantPreset::DatePalm,
                        PlantPreset::FanPalm] {
            let project=FoliageProject::from_preset(species);
            let mesh=project.generate().unwrap();
            let tree=mesh.structure.as_ref().unwrap();
            assert_eq!(tree.branches.len(),project.branch_count as usize+1);
            assert!(tree.branches.iter().skip(1).all(|b|
                b.parent==Some(0)&&b.part==TreePart::MajorBranch));
            assert!(!tree.branches.iter().any(|b|b.part==TreePart::SmallBranch));
            assert!(tree.cells.iter().any(|c|c.part==TreePart::Foliage));
            assert!(tree.spans.len()>=project.branch_count as usize);
            let high=tree.branches.iter().skip(1)
                .filter(|b|b.root[2] as f32>mesh.height*0.85).count();
            assert_eq!(high,project.branch_count as usize,
                "{species:?}: palm petioles must grow at crown");
            assert_eq!(mesh.voxels,project.generate().unwrap().voxels);
            let mut alternate=project.clone();
            alternate.seed+=1;
            assert_ne!(tree.spans,
                alternate.generate().unwrap().structure.unwrap().spans);
            tree.validate(&mesh.voxels).unwrap();
        }
        let alpha=|p|leaf_cutout_for_species(p)
            .chunks_exact(4).map(|px|px[3]).collect::<Vec<_>>();
        assert_ne!(alpha(PlantPreset::CoconutPalm),alpha(PlantPreset::FanPalm));
        assert_ne!(alpha(PlantPreset::DatePalm),alpha(PlantPreset::FanPalm));
    }

    #[test]
    fn all_species_have_stable_seeded_branch_and_leaf_placement() {
        for species in PlantPreset::TREE_CATALOG {
            let original=FoliageProject::from_preset(species);
            let a=original.generate().unwrap();
            let b=original.generate().unwrap();
            assert_eq!(a.structure.as_ref().unwrap().branches,
                b.structure.as_ref().unwrap().branches, "{species:?}");
            assert_eq!(a.structure.as_ref().unwrap().spans,
                b.structure.as_ref().unwrap().spans, "{species:?}");
            assert_eq!(a.voxels,b.voxels, "{species:?}");

            let mut different=original.clone();
            different.seed=different.seed.wrapping_add(1);
            let c=different.generate().unwrap();
            assert_ne!(a.structure.as_ref().unwrap().spans,
                c.structure.as_ref().unwrap().spans,
                "{species:?}: different seeds must change individual structure");
            assert_eq!(a.shape,c.shape);
            assert_eq!(a.voxel_size,c.voxel_size);
        }
    }

    #[test]
    fn apple_and_bamboo_color_variants_share_seeded_geometry() {
        let geometry=|preset| {
            let tree=FoliageProject::from_preset(preset);
            let mesh=tree.generate().unwrap();
            (mesh.structure.unwrap(),mesh.voxels)
        };
        assert_eq!(geometry(PlantPreset::RedApple),
            geometry(PlantPreset::PinkApple));
        assert_eq!(geometry(PlantPreset::PinkApple),
            geometry(PlantPreset::YellowApple));
        assert_eq!(geometry(PlantPreset::GreenBamboo),
            geometry(PlantPreset::BrownBamboo));
    }

    #[test]
    fn botanically_distinct_leaf_architectures_have_distinct_cutouts() {
        let alpha=|species| leaf_cutout_for_species(species)
            .chunks_exact(4).map(|pixel|pixel[3]).collect::<Vec<_>>();
        assert_ne!(alpha(PlantPreset::Purpleheart),alpha(PlantPreset::Walnut));
        assert_ne!(alpha(PlantPreset::Kapok),alpha(PlantPreset::Breadfruit));
        assert_ne!(alpha(PlantPreset::Larch),alpha(PlantPreset::BaldCypress));
        assert_ne!(alpha(PlantPreset::Larch),
            alpha(PlantPreset::MediterraneanCypress));
    }

    #[test]
    fn seeded_height_radius_and_base_flare_stay_bounded() {
        for species in [PlantPreset::Kapok,PlantPreset::Redwood,
            PlantPreset::Oak,PlantPreset::GreenBamboo] {
            let project=FoliageProject::from_preset(species);
            let nominal=project.height*project.overall_scale;
            let tree=project.generate().unwrap();
            assert!((tree.height-nominal).abs()<=nominal*0.056);
            let next=FoliageProject {seed:99,..project.clone()}
                .generate().unwrap();
            assert_ne!(tree.height,next.height,"{species:?}");
            assert_eq!(tree.structure.as_ref().unwrap().origin,[0,0,0]);
            assert!(tree.structure.as_ref().unwrap().root_footprint.len()>0);
        }
    }

    #[test]
    fn secondary_twig_growth_is_seeded_and_respects_species_limits() {
        for species in [PlantPreset::Oak,PlantPreset::Willow,PlantPreset::Pine,
            PlantPreset::Larch,PlantPreset::RedApple,PlantPreset::GreenBamboo] {
            let mesh=FoliageProject::from_preset(species).generate().unwrap();
            let tree=mesh.structure.as_ref().unwrap();
            let majors=tree.branches.iter().filter(|b|
                b.part==TreePart::MajorBranch).count();
            let twigs=tree.branches.iter().filter(|b|
                b.part==TreePart::SmallBranch).count();
            assert!(twigs>=majors && twigs<=majors*2,
                "{species:?}: {twigs} twigs for {majors} limbs");
            assert!(tree.spans.iter().filter(|span|span.branch_id!=0)
                .all(|span|span.start_radius_mm>span.end_radius_mm));
            tree.validate(&mesh.voxels).unwrap();
        }
    }

    #[test]
    fn tapered_branch_mesh_keeps_solid_trunks_and_textured_leaves() {
        for preset in [PlantPreset::Oak,PlantPreset::Pine,PlantPreset::Willow] {
            let mesh=FoliageProject::from_preset(preset).generate().unwrap();
            let tree=mesh.structure.as_ref().unwrap();
            assert!(tree.spans.len() >= 3, "{preset:?}");
            let branch_cells:BTreeSet<_>=tree.cells.iter()
                .filter(|c|matches!(c.part,TreePart::MajorBranch|TreePart::SmallBranch))
                .map(|c|c.cell).collect();
            let trunk_cells:BTreeSet<_>=tree.cells.iter()
                .filter(|c|c.part==TreePart::Trunk).map(|c|c.cell).collect();
            let visible_branch_cells:BTreeSet<_>=mesh.triangles.iter()
                .filter(|t|t.material.is_wood()).map(|t|t.cell).collect();
            assert!(branch_cells.iter().all(|c|visible_branch_cells.contains(c)),
                "{preset:?}: a harvestable branch cell is invisible");
            assert!(mesh.triangles.iter().any(|t|
                t.material.is_wood() && branch_cells.contains(&t.cell)),
                "{preset:?}: branch tubes missing");
            assert!(mesh.triangles.iter().any(|t|
                t.material.is_wood() && trunk_cells.contains(&t.cell)),
                "{preset:?}: trunk surface missing");
            assert!(mesh.triangles.iter().any(|t|t.cutout &&
                t.material.is_foliage()),"{preset:?}: leaf cards missing");
            assert!(tree.spans.iter().filter(|s|s.branch_id!=0).all(|s|
                s.start_radius_mm<=500 && s.end_radius_mm<=500 &&
                s.start_radius_mm>s.end_radius_mm),
                "{preset:?}: generated branch radii must be tapered and bounded");
            assert!(tree.spans.iter().any(|s|s.branch_id==0),
                "{preset:?}: smooth trunk must have an authored spine");
            let again=FoliageProject::from_preset(preset).generate().unwrap();
            assert_eq!(mesh.triangles.len(),again.triangles.len());
            assert_eq!(mesh.triangles[0].points,again.triangles[0].points);
        }
    }

    #[test]
    fn invalid_tapered_branch_path_is_rejected() {
        let mesh=FoliageProject::from_preset(PlantPreset::Oak).generate().unwrap();
        let mut structure=mesh.structure.unwrap();
        structure.spans[0].branch_id=999_999;
        assert!(structure.validate(&mesh.voxels).is_err());
    }

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
            let wood:BTreeSet<_>=mesh.voxels.iter()
                .filter(|cell|cell.material.is_wood())
                .map(|cell|[cell.x,cell.y,cell.z]).collect();
            for face in mesh.triangles.iter().filter(|face|face.material.is_wood()) {
                assert!(wood.contains(&face.cell),"{preset:?}: unowned wood");
                assert!(face.points.iter().all(|point|
                    crate::hexel_clip::contains(*point,face.cell,mesh.voxel_size)),
                    "{preset:?}: wood escaped owning hexel {:?}",face.cell);
            }
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
            assert_eq!(faces.len(), expected+24, "leaf uses six double-sided sprays");
            assert_eq!(faces.iter().filter(|t|t.cutout).count(),24);
            let solid=LeafRenderSettings {
                geometry:LeafGeometry::SolidVoxel,..Default::default() };
            let legacy = surface_triangles(&cells,shape,1.0,solid);
            assert_eq!(legacy.len(),expected*2-if shape==VoxelShape::Cube {4} else {8});

        }
    }

    #[test]
    fn leaf_sprays_are_deterministic_and_owned_by_one_harvestable_cell() {
        let mut cells=BTreeMap::new();
        cells.insert([2,-1,4],Material::Leaf);
        let a=surface_triangles(&cells,VoxelShape::HexPrism,1.0,
            LeafRenderSettings::default());
        let b=surface_triangles(&cells,VoxelShape::HexPrism,1.0,
            LeafRenderSettings::default());
        assert_eq!(a.len(),24);
        assert!(a.iter().all(|t|t.cell==[2,-1,4] && t.cutout));
        assert_eq!(a.iter().map(|t|t.points).collect::<Vec<_>>(),
            b.iter().map(|t|t.points).collect::<Vec<_>>());
    }

    #[test]
    fn interior_of_a_dense_crown_lobe_is_not_perforated_randomly() {
        let mut cells=BTreeMap::new();
        paint_leaf_cluster(&mut cells,VoxelShape::HexPrism,1.0,
            [0.0,0.0,8.0],4.0,3.2,0.82,0.32,0.12,42,7);
        let mut tested=0usize;
        let mut filled=0usize;
        for z in 5..=11 {
            for q in -5..=5 {
                for r in -5..=5 {
                    let key=[q,r,z];
                    let p=grid_position(VoxelShape::HexPrism,1.0,key);
                    let d=((p[0]/4.0).powi(2)+(p[1]/4.0).powi(2)
                        +((p[2]-8.0)/3.2).powi(2)).sqrt();
                    if d<0.60 {
                        tested+=1;
                        if cells.contains_key(&key){filled+=1;}
                    }
                }
            }
        }
        assert!(tested>10);
        assert!(filled as f32/tested as f32>0.94,
            "dense lobe has too many hollow interior cells: {filled}/{tested}");
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
            assert_eq!(model.triangle_count(),24);
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
            assert_eq!(center, if surroundings==Material::Bark {0} else {24});
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
            if !preset.is_palm() {
                assert!(tree.branches.iter().any(|b|b.part==TreePart::SmallBranch));
            }
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
