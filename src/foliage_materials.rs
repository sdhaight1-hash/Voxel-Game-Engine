//! Material-role-aware procedural texture assignments for voxel foliage.
//! Both slots use the existing editable graph engine and bundled natural presets.

use crate::foliage::{leaf_cutout_for_species, PlantPreset};
use crate::leaf_vector::{LeafSilhouetteSource,vector_oak_cutout_rgba,
    vector_willow_cutout_rgba,vector_cottonwood_cutout_rgba,
    vector_subpixel_cutout_rgba};
use crate::presets::{builtin_presets, Preset};
use crate::{Graph, VoxelSettings};
use crate::material_functions::{MaterialFunctionInstance,MaterialFunctionLibrary};
use serde::{Deserialize, Serialize};

pub const MATERIAL_SIZE: u32 = 128;

/// Subpixel coverage is authored and cached only for vector base silhouettes.
/// Binary remains the default for all legacy projects.
#[derive(Clone,Copy,Debug,PartialEq,Eq,Serialize,Deserialize,Default)]
pub enum LeafAlphaSampling {
    #[default]
    Binary,
    Coverage2x2,
}

fn default_threshold() -> f32 { 0.5 }
fn default_bark_tint_strength() -> f32 { 0.0 }
fn default_leaf_species() -> PlantPreset { PlantPreset::Oak }

/// Four seasons × four gradient stops (shadow, base, midtone, highlight).
/// Stored as RGB8 for compact JSON and native egui color editing; expanded
/// to 16 aligned vec4 values once when a wgpu scene is uploaded.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeasonalPalettes {
    pub seasons: [[[u8;3];4];4],
}
impl SeasonalPalettes {
    pub const NAMES: [&'static str;4] = ["Spring","Summer","Autumn","Winter"];
    pub const STOPS: [&'static str;4] = ["Shadow","Base","Midtone","Highlight"];

    fn blend(a:[u8;3],b:[u8;3],t:f32)->[u8;3] {
        std::array::from_fn(|i|
            ((a[i] as f32)*(1.0-t)+(b[i] as f32)*t).round() as u8)
    }
    fn gradient(color:[u8;3])->[[u8;3];4] {
        // Brightness progression, not four duplicate colors.
        let shade=|scale:f32|color.map(|c|(c as f32*scale).round().min(255.0) as u8);
        [shade(0.55),shade(0.83),color,shade(1.23)]
    }
    pub fn for_species(species:PlantPreset,summer:[u8;3])->Self {
        let evergreen=species.is_conifer() || species.is_bamboo() || species.is_palm()
            || species.is_coffee()
            || matches!(species,PlantPreset::Olive|PlantPreset::Orange|
                PlantPreset::Mango|PlantPreset::Lychee|
                PlantPreset::Breadfruit|PlantPreset::Kapok|
                PlantPreset::Ebony|PlantPreset::Purpleheart);
        let autumn=match species {
            PlantPreset::Maple=>[208,79,31],
            PlantPreset::CrimsonKingMaple=>[144,38,54],
            PlantPreset::Oak|PlantPreset::Walnut=>[176,104,40],
            PlantPreset::Birch=>[224,181,53],
            PlantPreset::KentuckyCoffeetree=>[210,188,81],
            PlantPreset::EasternCottonwood=>[216,180,53],
            PlantPreset::FremontCottonwood=>[238,192,54],
            PlantPreset::BlackCottonwood=>[202,173,70],
            PlantPreset::Willow=>[198,169,65],
            PlantPreset::Larch|PlantPreset::BaldCypress=>[187,128,43],
            PlantPreset::CherryBlossom=>[167,98,86],
            PlantPreset::Cherry|PlantPreset::RedApple|PlantPreset::PinkApple|
            PlantPreset::YellowApple=>[197,104,54],
            PlantPreset::Pear|PlantPreset::Peach=>[197,151,56],
            _=>[173,139,56],
        };
        let spring=if species==PlantPreset::CherryBlossom {
            // Light bronze-green emerging leaves beneath a separate, temporary
            // pink blossom overlay in the GPU spring preview.
            Self::blend(summer,[153,144,92],0.35)
        } else if evergreen { Self::blend(summer,[115,177,93],0.13) }
            else {Self::blend(summer,[164,202,81],0.32)};
        let autumn=if evergreen && !matches!(species,
            PlantPreset::Larch|PlantPreset::BaldCypress) {
                Self::blend(summer,autumn,0.13)
            } else {autumn};
        // Color-only winter. Deciduous leaf shedding belongs to ecology,
        // not the foliage palette shader.
        let winter=if evergreen {
            Self::blend(summer,[68,93,84],0.28)
        } else {
            Self::blend(autumn,[105,91,72],0.54)
        };
        Self {seasons:[Self::gradient(spring),Self::gradient(summer),
            Self::gradient(autumn),Self::gradient(winter)]}
    }
    /// WGSL uniform array order: Spring's 4 stops, Summer's 4, etc.
    pub fn gpu_stops(&self)->[[f32;4];16] {
        std::array::from_fn(|i|{
            let c=self.seasons[i/4][i%4];
            [c[0] as f32/255.0,c[1] as f32/255.0,c[2] as f32/255.0,1.0]
        })
    }
    /// CPU reference for tests and preview color swatches; the actual
    /// season blend is performed in the WGSL fragment shader.
    pub fn sample(&self,progress:f32,factor:f32)->[f32;3] {
        let p=progress.rem_euclid(4.0);
        let season=p.floor() as usize;
        let t=p.fract();
        let evaluate=|season:usize|{
            let x=factor.clamp(0.0,1.0)*3.0;
            let k=(x.floor() as usize).min(2);
            let f=x-k as f32;
            std::array::from_fn(|ch| {
                let a=self.seasons[season][k][ch] as f32/255.0;
                let b=self.seasons[season][k+1][ch] as f32/255.0;
                a+(b-a)*f
            })
        };
        let a:[f32;3]=evaluate(season);
        let b:[f32;3]=evaluate((season+1)%4);
        std::array::from_fn(|ch|a[ch]+(b[ch]-a[ch])*t)
    }
}
impl Default for SeasonalPalettes {
    fn default()->Self {
        Self::for_species(PlantPreset::Oak,[67,135,52])
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MaterialRole { Bark, Leaf, Opacity }
impl MaterialRole {
    pub fn name(self)-> &'static str {match self {
        Self::Bark=>"Bark",Self::Leaf=>"Leaf Pattern (palette-tinted)",Self::Opacity=>"Leaf Cutout",
    }}
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct FoliageMaterials {
    /// Independent PBR surface graphs. Legacy projects may omit these.
    pub bark: Option<Graph>,
    pub leaf: Option<Graph>,
    /// Optional height/greyscale-style graph applied as an additional opacity mask.
    /// The original leaf-card silhouette is always preserved.
    pub leaf_opacity: Option<Graph>,
    /// Authoring provenance only: baked graph snapshots above remain authoritative.
    #[serde(default)]
    pub bark_function: Option<MaterialFunctionInstance>,
    #[serde(default)]
    pub leaf_function: Option<MaterialFunctionInstance>,
    #[serde(default)]
    pub opacity_function: Option<MaterialFunctionInstance>,
    /// 0 = unchanged authored graph; 1 = graph luminance colored by bark_color.
    /// Defaults to 0 so existing tree materials remain pixel-identical.
    #[serde(default = "default_bark_tint_strength")]
    pub bark_tint_strength: f32,
    /// Optional physical bark tiling in meters per repeat. None preserves
    /// legacy UV sampling in procedural tree and editable Asset GPU previews.
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub bark_meters_per_repeat: Option<f32>,
    pub bark_preset: Option<String>,
    pub leaf_preset: Option<String>,
    /// The shape of the leaf-card cutout. Travels with the authored asset,
    /// including if its material node graphs are edited independently.
    #[serde(default = "default_leaf_species")]
    pub leaf_species: PlantPreset,
    /// Optional base leaf cutout replacement; legacy species art is default.
    #[serde(default)]
    pub leaf_silhouette: LeafSilhouetteSource,
    #[serde(default)]
    pub leaf_alpha_sampling: LeafAlphaSampling,
    /// Ripe fruit colour for a small visual indication on foliage cards.
    /// Real fruit items, harvesting and seasonal ripening are world systems.
    pub fruit_color: Option<[u8;3]>,
    /// Optional so existing saved assets can compute the correct fallback
    /// from their original leaf species and base tint.
    pub seasonal_palettes: Option<SeasonalPalettes>,
    #[serde(default = "default_threshold")]
    pub cutout_threshold: f32,
}

impl Default for FoliageMaterials {
    fn default() -> Self {
        Self { bark: None, leaf: None, leaf_opacity: None,
            bark_function:None,leaf_function:None,opacity_function:None,
            bark_tint_strength: 0.0,
            bark_meters_per_repeat: None,
            bark_preset: None, leaf_preset: None,
            leaf_species: PlantPreset::Oak,
            leaf_silhouette: LeafSilhouetteSource::LegacySpecies,
            leaf_alpha_sampling: LeafAlphaSampling::Binary,
            fruit_color: None,
            seasonal_palettes: None, cutout_threshold: 0.5 }
    }
}

impl FoliageMaterials {
    pub fn effective_palettes(&self,leaf_tint:[u8;3])->SeasonalPalettes {
        self.seasonal_palettes.clone().unwrap_or_else(||
            SeasonalPalettes::for_species(self.leaf_species,leaf_tint))
    }

    pub fn validate(&self) -> Result<(), String> {
        self.leaf_silhouette.validate(self.leaf_species)?;
        if self.leaf_alpha_sampling==LeafAlphaSampling::Coverage2x2 &&
            matches!(self.leaf_silhouette,LeafSilhouetteSource::LegacySpecies) {
            return Err("2x2 leaf-card coverage requires an authored vector silhouette".into());
        }
        if !self.bark_tint_strength.is_finite()
            || !(0.0..=1.0).contains(&self.bark_tint_strength) {
            return Err("Bark tint strength must be 0..1".into());
        }
        if let Some(meters)=self.bark_meters_per_repeat {
            if !meters.is_finite() || !(0.05..=8.0).contains(&meters) {
                return Err("Bark meters per repeat must be 0.05..8".into());
            }
        }
        if !self.cutout_threshold.is_finite()
            || !(0.0..=1.0).contains(&self.cutout_threshold) {
            return Err("Foliage cutout threshold must be 0..1".into());
        }
        for (graph,instance) in [
            (&self.bark,&self.bark_function),
            (&self.leaf,&self.leaf_function),
            (&self.leaf_opacity,&self.opacity_function)] {
            if let Some(graph)=graph {graph.validate()?;}
            if let Some(instance)=instance {
                instance.validate()?;
                if graph.is_none() {
                    return Err("Material function reference is missing its graph snapshot".into());
                }
            }
        }
        Ok(())
    }

    pub fn assign_preset(&mut self, slot: MaterialRole, preset: &Preset) {
        let graph=preset.build_graph(VoxelSettings {
            geometryless:true, ..Default::default()
        });
        match slot {
            MaterialRole::Bark => {
                self.bark=Some(graph);self.bark_preset=Some(preset.title());
                self.bark_function=None;
            }
            MaterialRole::Leaf => {
                self.leaf=Some(graph);self.leaf_preset=Some(preset.title());
                self.leaf_function=None;
            }
            MaterialRole::Opacity => {self.leaf_opacity=Some(graph);self.opacity_function=None;}
        }
    }

    pub fn graph(&self,slot:MaterialRole)->Option<&Graph> {
        match slot {
            MaterialRole::Bark=>self.bark.as_ref(),
            MaterialRole::Leaf=>self.leaf.as_ref(),
            MaterialRole::Opacity=>self.leaf_opacity.as_ref(),
        }
    }

    pub fn set_graph(&mut self,slot:MaterialRole,graph:Graph) {
        match slot {
            MaterialRole::Bark=>{self.bark=Some(graph);self.bark_preset=None;
                self.bark_function=None;}
            MaterialRole::Leaf=>{self.leaf=Some(graph);self.leaf_preset=None;
                self.leaf_function=None;}
            MaterialRole::Opacity=>{self.leaf_opacity=Some(graph);
                self.opacity_function=None;}
        }
    }

    /// Resolve a versioned authoring template into this material's existing
    /// role graph. The provenance is stored for editable authoring, but all
    /// bake/render paths keep reading the graph snapshot, even after the
    /// external function library is removed.
    pub fn apply_function(&mut self,role:MaterialRole,
        library:&MaterialFunctionLibrary,reference:MaterialFunctionInstance)
        ->Result<(),String> {
        let graph=library.resolve(&reference)?;
        self.set_graph(role,graph);
        match role {
            MaterialRole::Bark=>self.bark_function=Some(reference),
            MaterialRole::Leaf=>self.leaf_function=Some(reference),
            MaterialRole::Opacity=>self.opacity_function=Some(reference),
        }
        Ok(())
    }

    /// Map each plant to its own original, data-driven bark/leaf material.
    /// Old saved trees and assets retain their serialized graph snapshots.
    /// No geometry or species art is claimed to be scanned or game-derived.
    pub fn for_species(species: PlantPreset) -> Self {
        let mut out=Self::default();
        out.leaf_species=species;
        out.fruit_color=species.fruit_color();
        // Palette defaults are derived from the exact generated leaf tint
        // and selected species in the FoliageProject; a standalone material
        // without explicit palettes keeps the per-species fallback.
        out.seasonal_palettes=None;
        // Reuse exact names and variant types from the existing natural
        // material catalog. No extra external texture library is required.
        let name=species.material_name();
        let bark_variants:&[&str]=if species.is_bamboo() || species.is_palm() {
            &["Stalk sides"]
        } else if species==PlantPreset::Willow {
            &["Willow bark","Bark / trunk sides"]
        } else if species.group()=="Fruit trees" || species.is_coffee() {
            &["Bark"]
        } else {
            &["Bark / trunk sides","Bark"]
        };
        let leaf_variants:&[&str]=if species.is_bamboo() {
            &["Leaves"]
        } else if species.is_palm() {
            &["Frond canopy"]
        } else if species==PlantPreset::Willow {
            &["Willow hanging leaf clusters","Willow leaves"]
        } else if species.group()=="Fruit trees" || species.is_coffee() {
            &["Leaves"]
        } else {
            &["Leaf-only canopy","Mature foliage","Leaves"]
        };
        if let Ok(presets)=builtin_presets() {
            for (is_bark,variants) in [(true,bark_variants),(false,leaf_variants)] {
                if let Some(preset)=presets.iter().find(|p|
                    p.material==name && variants.contains(&p.variant.as_str())) {
                    let graph=preset.build_graph(VoxelSettings {
                        geometryless:true, ..Default::default()
                    });
                    if is_bark {
                        out.bark_preset=Some(preset.title());
                        out.bark=Some(graph);
                    } else {
                        out.leaf_preset=Some(preset.title());
                        out.leaf=Some(graph);
                    }
                }
            }
        }
        out
    }

    /// Bake cutout foliage with the authored species tint as the base color.
    /// Graphs contribute surface light/dark variation, not a replacement
    /// palette that can turn green Oak foliage into brown bark.
    pub fn bake_leaf(&self, size:u32, leaf_tint:[u8;3]) -> Result<Vec<u8>,String> {
        self.validate()?;
        if size==0 || size>1024 {return Err("Foliage bake must be 1..1024 px".into());}
        let base=if self.leaf_alpha_sampling==LeafAlphaSampling::Coverage2x2 {
            vector_subpixel_cutout_rgba(self.leaf_silhouette,self.leaf_species)?
        } else {match self.leaf_silhouette {
            LeafSilhouetteSource::LegacySpecies=>leaf_cutout_for_species(self.leaf_species),
            LeafSilhouetteSource::VectorOak{lobe_width}=>vector_oak_cutout_rgba(lobe_width),
            LeafSilhouetteSource::VectorWillow{blade_width}=>
                vector_willow_cutout_rgba(blade_width),
            LeafSilhouetteSource::VectorCottonwood{blade_width}=>
                vector_cottonwood_cutout_rgba(self.leaf_species,blade_width),
        }};
        let graph=self.leaf.as_ref().map(|g|g.render_rgba(size,size)).transpose()?;
        let opacity=self.leaf_opacity.as_ref()
            .map(|g|g.render_rgba(size,size)).transpose()?;
        let mut rgba=vec![0u8;size as usize*size as usize*4];
        for y in 0..size {
            for x in 0..size {
                let dst=((y*size+x)*4) as usize;
                let bx=(x as usize*64/size as usize).min(63);
                let by=(y as usize*64/size as usize).min(63);
                let src=(by*64+bx)*4;
                let silhouette=base[src+3];
                let alpha=match &opacity {
                    Some(mask) if mask[dst] as f32/255.0 < self.cutout_threshold => 0,
                    _=>silhouette,
                };
                // The species tint owns the base leaf color. The node graph
                // controls luminance/detail while keeping seasonal/species
                // green, red or purple foliage recognizable.
                let graph_light=graph.as_ref().map(|color| {
                    (0.25*color[dst] as f32 +
                     0.60*color[dst+1] as f32 +
                     0.15*color[dst+2] as f32)/255.0
                }).unwrap_or(1.0);
                let shade=(0.76+0.24*base[src] as f32/255.0)
                    *(0.72+0.28*graph_light);
                for ch in 0..3 {
                    rgba[dst+ch]=((leaf_tint[ch] as f32)*shade)
                        .round().clamp(0.0,255.0) as u8;
                }
                if alpha!=0 {
                    if let Some(fruit)=self.fruit_color {
                        // Small fruit-colored dots are *texture only*. They
                        // don't create physical or harvestable fruit objects.
                        let u=(x as f32+0.5)/size as f32;
                        let v=(y as f32+0.5)/size as f32;
                        for (index,p) in [[0.37,0.39],[0.63,0.52],
                            [0.37,0.65],[0.63,0.78]].iter().enumerate() {
                            if (u-p[0])*(u-p[0])+(v-p[1])*(v-p[1])<0.034*0.034 {
                                // Coffee cherries on one branch can be at
                                // different ripeness stages. Static card art,
                                // not simulated per-fruit growth or harvesting.
                                let color=if self.leaf_species.is_coffee() {
                                    match index {
                                        0=>[85,141,66], // unripe green
                                        1=>[205,170,45], // yellow
                                        2=>[220,112,43], // orange
                                        _=>fruit, // ripe red
                                    }
                                } else {fruit};
                                rgba[dst..dst+3].copy_from_slice(&color);
                                break;
                            }
                        }
                    }
                }
                rgba[dst+3]=alpha;
            }
        }
        Ok(rgba)
    }

    pub fn bake_bark(&self,size:u32,bark_tint:[u8;3])->Result<Vec<u8>,String> {
        self.validate()?;
        if size==0 || size>1024 {return Err("Bark bake must be 1..1024 px".into());}
        if let Some(graph)=&self.bark {
            let mut rgba=graph.render_rgba(size,size)?;
            let mix=self.bark_tint_strength;
            if mix>0.0 {
                for pixel in rgba.chunks_exact_mut(4) {
                    let luminance=(0.25*pixel[0] as f32 + 0.60*pixel[1] as f32
                        + 0.15*pixel[2] as f32)/255.0;
                    let shade=0.5+0.5*luminance;
                    for ch in 0..3 {
                        let tinted=bark_tint[ch] as f32*shade;
                        pixel[ch]=(pixel[ch] as f32*(1.0-mix)+tinted*mix)
                            .round().clamp(0.0,255.0) as u8;
                    }
                }
            }
            Ok(rgba)
        } else {Ok(bark_tint.iter().copied().chain(std::iter::once(255))
            .cycle().take(size as usize*size as usize*4).collect())}
    }

    pub fn bake_leaf_opacity(&self,size:u32,leaf_tint:[u8;3])
        ->Result<Vec<u8>,String> {
        let tex=self.bake_leaf(size,leaf_tint)?;
        Ok(tex.chunks_exact(4).flat_map(|p|[p[3],p[3],p[3],255])
            .collect())
    }

    pub fn bake_pbr(&self,size:u32)->Result<Vec<(String,Vec<u8>)>,String> {
        self.validate()?;
        let mut result=Vec::new();
        for (label,graph) in [("bark",&self.bark),("foliage-leaves",&self.leaf)] {
            if let Some(g)=graph {
                result.push((format!("{label}-roughness.png"),
                    g.render_output_channel_rgba(1,size,size)?));
                result.push((format!("{label}-normal.png"),
                    g.render_normal_rgba(size,size)?));
            }
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::foliage::LeafGeometry;

    #[test]
    fn species_use_real_bark_and_leaf_presets() {
        for species in PlantPreset::ALL {
            let mats=FoliageMaterials::for_species(species);
            assert!(mats.bark.is_some(),"{species:?} missing bark preset");
            assert!(mats.leaf.is_some(),"{species:?} missing leaf preset");
            assert!(mats.bark_preset.is_some());
            assert!(mats.leaf_preset.is_some());
            assert_eq!(mats.leaf_species,species);
            mats.validate().unwrap();
        }
    }

    #[test]
    fn vector_oak_replaces_its_base_silhouette_but_masks_cannot_fill_outside() {
        let mut materials=FoliageMaterials::for_species(PlantPreset::Oak);
        let legacy=materials.bake_leaf(64,[65,145,68]).unwrap();
        materials.leaf_silhouette=LeafSilhouetteSource::VectorOak{
            lobe_width:1.1,
        };
        let vector=materials.bake_leaf(64,[65,145,68]).unwrap();
        assert_ne!(vector,legacy);
        assert!(legacy.chunks_exact(4).zip(vector.chunks_exact(4))
            .any(|(a,b)|a[3]<b[3]),
            "the optional vector source must actually replace, not intersect with, legacy shape");
        let mut mask=Graph::default();
        mask.node_mut(1).unwrap().kind=crate::NodeKind::Color{rgb:[0,0,0]};
        materials.set_graph(MaterialRole::Opacity,mask);
        let empty=materials.bake_leaf(64,[65,145,68]).unwrap();
        assert!(empty.chunks_exact(4).all(|px|px[3]==0));
        materials.leaf_opacity=None;
        let restored=materials.bake_leaf(64,[65,145,68]).unwrap();
        assert_eq!(restored,vector);
        let saved=serde_json::to_string(&materials).unwrap();
        let loaded:FoliageMaterials=serde_json::from_str(&saved).unwrap();
        assert_eq!(loaded.bake_leaf(64,[65,145,68]).unwrap(),vector);
    }

    #[test]
    fn vector_subpixel_coverage_preserves_tint_and_subtractive_masks() {
        for (species,source) in [
            (PlantPreset::Oak,LeafSilhouetteSource::VectorOak{lobe_width:1.0}),
            (PlantPreset::Willow,LeafSilhouetteSource::VectorWillow{blade_width:1.0}),
            (PlantPreset::BlackCottonwood,
                LeafSilhouetteSource::VectorCottonwood{blade_width:1.0}),
        ] {
            let mut mat=FoliageMaterials::for_species(species);
            mat.leaf_silhouette=source;
            let binary=mat.bake_leaf(64,[62,146,73]).unwrap();
            mat.leaf_alpha_sampling=LeafAlphaSampling::Coverage2x2;
            let covered=mat.bake_leaf(64,[62,146,73]).unwrap();
            assert_ne!(binary,covered);
            assert!(covered.chunks_exact(4)
                .any(|px|px[3]>0 && px[3]<255));
            let mut black=Graph::default();
            black.node_mut(1).unwrap().kind=crate::NodeKind::Color{rgb:[0,0,0]};
            mat.set_graph(MaterialRole::Opacity,black);
            let empty=mat.bake_leaf(64,[62,146,73]).unwrap();
            assert!(empty.chunks_exact(4).all(|px|px[3]==0));
            mat.leaf_opacity=None;
            assert_eq!(mat.bake_leaf(64,[62,146,73]).unwrap(),covered);
            let restored:FoliageMaterials=serde_json::from_str(
                &serde_json::to_string(&mat).unwrap()).unwrap();
            assert_eq!(restored.bake_leaf(64,[62,146,73]).unwrap(),covered);
        }
        let legacy:FoliageMaterials=serde_json::from_str("{}").unwrap();
        assert_eq!(legacy.leaf_alpha_sampling,LeafAlphaSampling::Binary);
        let mut bad=FoliageMaterials::default();
        bad.leaf_alpha_sampling=LeafAlphaSampling::Coverage2x2;
        assert!(bad.validate().is_err());
    }

    #[test]
    fn willow_and_cottonwood_vector_cutouts_preserve_silhouette_owner_and_saved_json() {
        for (species,source) in [
            (PlantPreset::Willow,LeafSilhouetteSource::VectorWillow{blade_width:1.1}),
            (PlantPreset::EasternCottonwood,
                LeafSilhouetteSource::VectorCottonwood{blade_width:1.0}),
            (PlantPreset::FremontCottonwood,
                LeafSilhouetteSource::VectorCottonwood{blade_width:1.1}),
            (PlantPreset::BlackCottonwood,
                LeafSilhouetteSource::VectorCottonwood{blade_width:0.9}),
        ] {
            let mut materials=FoliageMaterials::for_species(species);
            let old=materials.bake_leaf(64,[76,141,63]).unwrap();
            materials.leaf_silhouette=source;
            let vector=materials.bake_leaf(64,[76,141,63]).unwrap();
            assert_ne!(old,vector,"{species:?} vector option must be effective");
            let mut blackout=Graph::default();
            blackout.node_mut(1).unwrap().kind=crate::NodeKind::Color{rgb:[0,0,0]};
            materials.set_graph(MaterialRole::Opacity,blackout);
            let cut=materials.bake_leaf(64,[76,141,63]).unwrap();
            assert!(cut.chunks_exact(4).all(|p|p[3]==0),
                "{species:?} opacity must remove all alpha");
            materials.leaf_opacity=None;
            assert_eq!(materials.bake_leaf(64,[76,141,63]).unwrap(),vector);
            let saved=serde_json::to_string(&materials).unwrap();
            let loaded:FoliageMaterials=serde_json::from_str(&saved).unwrap();
            loaded.validate().unwrap();
            assert_eq!(loaded.bake_leaf(64,[76,141,63]).unwrap(),vector);
            materials.leaf_silhouette=LeafSilhouetteSource::LegacySpecies;
            assert_eq!(materials.bake_leaf(64,[76,141,63]).unwrap(),old);
        }
    }

    #[test]
    fn old_materials_default_to_legacy_silhouette_and_non_oak_rejects_vector_oak() {
        let legacy:FoliageMaterials=serde_json::from_str(
            r#"{"leaf_species":"Oak"}"#).unwrap();
        assert_eq!(legacy.leaf_silhouette,LeafSilhouetteSource::LegacySpecies);
        assert_eq!(legacy.bake_leaf(64,[65,145,68]).unwrap(),
            FoliageMaterials::default().bake_leaf(64,[65,145,68]).unwrap());
        let mut willow=FoliageMaterials::for_species(PlantPreset::Willow);
        willow.leaf_silhouette=LeafSilhouetteSource::VectorOak{lobe_width:1.0};
        assert!(willow.validate().is_err());
    }

    #[test]
    fn bake_preserves_alpha_gaps_and_uses_preset_palette() {
        let mat=FoliageMaterials::for_species(PlantPreset::Oak);
        let rgba=mat.bake_leaf(32,[0,255,0]).unwrap();
        assert_eq!(rgba.len(),32*32*4);
        assert!(rgba.chunks_exact(4).any(|p|p[3]==0));
        assert!(rgba.chunks_exact(4).any(|p|p[3]==255));
        assert!(rgba.chunks_exact(4).any(|p|p[3]==255 && p[0]==0 && p[1]>0));
        let mask=mat.bake_leaf_opacity(32,[0,255,0]).unwrap();
        assert!(mask.chunks_exact(4).all(|p|p[0]==p[1]&&p[1]==p[2]&&p[3]==255));
        assert_eq!(mat.bake_bark(16,[0;3]).unwrap().len(),16*16*4);
        assert!(!mat.bake_pbr(8).unwrap().is_empty());
    }

    #[test]
    fn leaf_material_graph_cannot_replace_oak_green_with_bark_brown() {
        let project=crate::foliage::FoliageProject::from_preset(PlantPreset::Oak);
        let baked=project.materials.bake_leaf(64,project.leaf_color).unwrap();
        let samples:Vec<_>=baked.chunks_exact(4).filter(|p|p[3]==255).collect();
        assert!(!samples.is_empty());
        assert!(samples.iter().all(|p|p[1]>p[0] && p[1]>p[2]),
            "Oak leaf graph overrode the green species palette");
    }

    #[test]
    fn all_catalog_materials_bake_and_keep_species_and_fruit_data() {
        assert_eq!(PlantPreset::TREE_CATALOG.len(),42);
        assert_eq!(PlantPreset::ALL.len(),43);
        for species in PlantPreset::ALL {
            let mats=FoliageMaterials::for_species(species);
            let baked=mats.bake_leaf(64,[120,155,90]).unwrap();
            assert_eq!(baked.len(),64*64*4);
            assert!(baked.chunks_exact(4).any(|p|p[3]==0));
            assert!(baked.chunks_exact(4).any(|p|p[3]==255));
            let opacity=mats.bake_leaf_opacity(64,[120,155,90]).unwrap();
            for (px,rgba) in opacity.chunks_exact(4).zip(baked.chunks_exact(4)) {
                assert_eq!(*px,[rgba[3],rgba[3],rgba[3],255]);
            }
            let restored:FoliageMaterials=serde_json::from_str(
                &serde_json::to_string(&mats).unwrap()).unwrap();
            assert_eq!(restored.leaf_species,species);
            assert_eq!(restored.fruit_color,species.fruit_color());
            assert_eq!(restored.bake_leaf(64,[120,155,90]).unwrap(),baked);
        }
        for pair in [PlantPreset::RedApple,PlantPreset::PinkApple,
                     PlantPreset::YellowApple] {
            assert!(pair.fruit_color().is_some());
        }
        assert_ne!(FoliageMaterials::for_species(PlantPreset::RedApple)
            .bake_leaf(64,[120,155,90]).unwrap(),
            FoliageMaterials::for_species(PlantPreset::YellowApple)
            .bake_leaf(64,[120,155,90]).unwrap());
    }

    #[test]
    fn ornamental_cherry_has_distinct_fruit_and_foliage_seasons() {
        let ornamental=crate::foliage::FoliageProject::from_preset(PlantPreset::CherryBlossom);
        let orchard=crate::foliage::FoliageProject::from_preset(PlantPreset::Cherry);
        assert_eq!(ornamental.materials.fruit_color,None);
        assert!(orchard.materials.fruit_color.is_some());
        let palette=ornamental.materials.effective_palettes(ornamental.leaf_color);
        let summer=palette.sample(1.0,0.66);
        let autumn=palette.sample(2.0,0.66);
        assert!(summer[1]>summer[0] && summer[1]>summer[2]);
        assert!(autumn[0]>autumn[1]);
        assert_eq!(palette.sample(0.0,0.5),palette.sample(4.0,0.5));
    }

    #[test]
    fn palm_materials_remain_evergreen_without_fake_harvestable_fruit() {
        for species in [PlantPreset::CoconutPalm,PlantPreset::DatePalm,
                        PlantPreset::FanPalm] {
            let project=crate::foliage::FoliageProject::from_preset(species);
            let materials=&project.materials;
            assert!(materials.bark.is_some(), "{species:?}");
            assert!(materials.leaf.is_some(), "{species:?}");
            assert_eq!(materials.fruit_color,None);
            let pal=materials.effective_palettes(project.leaf_color);
            assert!(pal.sample(2.0,0.5)[1]>pal.sample(2.0,0.5)[0]*0.9);
            assert!(pal.sample(3.0,0.5)[1]>pal.sample(3.0,0.5)[0]*0.9);
        }
    }

    #[test]
    fn coffee_is_evergreen_and_cottonwoods_turn_yellow() {
        for species in [PlantPreset::ArabicaCoffee,PlantPreset::CaturraCoffee,
            PlantPreset::MaragogipeCoffee,PlantPreset::CastilloCoffee,
            PlantPreset::TabiCoffee] {
            let tree=crate::foliage::FoliageProject::from_preset(species);
            let palette=tree.materials.effective_palettes(tree.leaf_color);
            let autumn=palette.sample(2.0,0.5);
            assert!(autumn[1]>autumn[0],"{species:?}");
            assert!(tree.materials.fruit_color.is_some());
            assert!(tree.materials.leaf.is_some() && tree.materials.bark.is_some());
        }
        for species in [PlantPreset::EasternCottonwood,PlantPreset::FremontCottonwood,
            PlantPreset::BlackCottonwood] {
            let tree=crate::foliage::FoliageProject::from_preset(species);
            let autumn=tree.materials.effective_palettes(tree.leaf_color)
                .sample(2.0,0.5);
            assert!(autumn[0]>autumn[2]*1.8,"{species:?}");
            assert!(autumn[1]>autumn[2]*1.4,"{species:?}");
            assert_eq!(tree.materials.fruit_color,None);
        }
        let kentucky=crate::foliage::FoliageProject::from_preset(
            PlantPreset::KentuckyCoffeetree);
        assert_eq!(kentucky.materials.fruit_color,None);
    }

    #[test]
    fn seasonal_palettes_are_editable_serializable_and_wrap_smoothly() {
        for species in PlantPreset::ALL {
            let palettes=SeasonalPalettes::for_species(species,[75,135,56]);
            assert_eq!(palettes.gpu_stops().len(),16);
            assert_eq!(palettes.sample(0.0,0.5),palettes.sample(4.0,0.5));
            assert_eq!(palettes.sample(1.0,0.5),palettes.sample(5.0,0.5));
            for a in 0..4 {
                let middle=palettes.sample(a as f32+0.5,0.4);
                let before=palettes.sample(a as f32,0.4);
                let after=palettes.sample(((a+1)%4) as f32,0.4);
                for i in 0..3 {
                    assert!((middle[i]-(before[i]+after[i])*0.5).abs()<0.00001);
                }
            }
            let decoded:SeasonalPalettes=serde_json::from_str(
                &serde_json::to_string(&palettes).unwrap()).unwrap();
            assert_eq!(palettes,decoded);
        }
        let mut mat=FoliageMaterials::for_species(PlantPreset::Maple);
        let mut palette=mat.effective_palettes([112,163,52]);
        palette.seasons[2][3]=[232,39,26];
        mat.seasonal_palettes=Some(palette.clone());
        let loaded:FoliageMaterials=serde_json::from_str(
            &serde_json::to_string(&mat).unwrap()).unwrap();
        assert_eq!(loaded.effective_palettes([112,163,52]),palette);
        let old:FoliageMaterials=serde_json::from_str(
            r#"{"leaf_species":"Willow"}"#).unwrap();
        assert_eq!(old.effective_palettes([91,156,65]),
            SeasonalPalettes::for_species(PlantPreset::Willow,[91,156,65]));
    }

    #[test]
    fn an_opacity_graph_can_punch_holes_without_filling_outside_silhouette() {
        let mut mat=FoliageMaterials::default();
        let original=mat.bake_leaf(32,[60,170,80]).unwrap();
        let mut black=Graph::default();
        black.nodes[0].kind=crate::NodeKind::Color{rgb:[0,0,0]};
        mat.set_graph(MaterialRole::Opacity,black);
        let cut=mat.bake_leaf(32,[60,170,80]).unwrap();
        assert!(cut.chunks_exact(4).all(|p|p[3]==0));
        mat.cutout_threshold=0.0;
        let restored=mat.bake_leaf(32,[60,170,80]).unwrap();
        assert_eq!(restored,original);
        let json=serde_json::to_string(&mat).unwrap();
        let again:FoliageMaterials=serde_json::from_str(&json).unwrap();
        assert!(again.leaf_opacity.is_some());
    }

    #[test]
    fn bark_tint_is_explicit_and_old_graph_colors_remain_unchanged() {
        let mut mat=FoliageMaterials::default();
        let mut graph=Graph::default();
        graph.nodes[0].kind=crate::NodeKind::Color{rgb:[100,120,140]};
        mat.bark=Some(graph.clone());
        let old=graph.render_rgba(8,8).unwrap();
        assert_eq!(mat.bake_bark(8,[200,70,20]).unwrap(),old);
        mat.bark_tint_strength=1.0;
        let tinted=mat.bake_bark(8,[200,70,20]).unwrap();
        assert_ne!(tinted,old);
        assert!(tinted.chunks_exact(4).all(|p|p[0]>p[1] && p[1]>p[2]));
        let restored:FoliageMaterials=serde_json::from_str(
            &serde_json::to_string(&mat).unwrap()).unwrap();
        assert_eq!(restored.bark_tint_strength,1.0);
        assert_eq!(restored.bake_bark(8,[200,70,20]).unwrap(),tinted);
        mat.bark_tint_strength=f32::NAN;
        assert!(mat.validate().is_err());
    }

    #[test]
    fn legacy_materials_and_invalid_thresholds() {
        let old:FoliageMaterials=serde_json::from_str("{}").unwrap();
        assert!(old.bark.is_none());
        assert_eq!(old.cutout_threshold,0.5);
        assert_eq!(old.bark_tint_strength,0.0);
        let mut m=old;
        m.cutout_threshold=f32::NAN;
        assert!(m.validate().is_err());
        let _=LeafGeometry::CrossedCutout;
    }
}
