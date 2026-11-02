//! Material-role-aware procedural texture assignments for voxel foliage.
//! Both slots use the existing editable graph engine and bundled natural presets.

use crate::foliage::{leaf_cutout_rgba, PlantPreset};
use crate::presets::{builtin_presets, Preset};
use crate::{Graph, VoxelSettings};
use serde::{Deserialize, Serialize};

pub const MATERIAL_SIZE: u32 = 128;

fn default_threshold() -> f32 { 0.5 }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MaterialRole { Bark, Leaf, Opacity }
impl MaterialRole {
    pub fn name(self)-> &'static str {match self {
        Self::Bark=>"Bark",Self::Leaf=>"Leaf Albedo",Self::Opacity=>"Leaf Cutout",
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
    pub bark_preset: Option<String>,
    pub leaf_preset: Option<String>,
    #[serde(default = "default_threshold")]
    pub cutout_threshold: f32,
}

impl Default for FoliageMaterials {
    fn default() -> Self {
        Self { bark: None, leaf: None, leaf_opacity: None,
            bark_preset: None, leaf_preset: None, cutout_threshold: 0.5 }
    }
}

impl FoliageMaterials {
    pub fn validate(&self) -> Result<(), String> {
        if !self.cutout_threshold.is_finite()
            || !(0.0..=1.0).contains(&self.cutout_threshold) {
            return Err("Foliage cutout threshold must be 0..1".into());
        }
        for graph in [&self.bark, &self.leaf, &self.leaf_opacity].into_iter().flatten() {
            graph.validate()?;
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
            }
            MaterialRole::Leaf => {
                self.leaf=Some(graph);self.leaf_preset=Some(preset.title());
            }
            MaterialRole::Opacity => self.leaf_opacity=Some(graph),
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
            MaterialRole::Bark=>{self.bark=Some(graph);self.bark_preset=None;}
            MaterialRole::Leaf=>{self.leaf=Some(graph);self.leaf_preset=None;}
            MaterialRole::Opacity=>self.leaf_opacity=Some(graph),
        }
    }

    /// Map each plant family to *existing* built-in bark/leaf appearance variants.
    /// No geometry or species art is claimed to be scanned or game-derived.
    pub fn for_species(species: PlantPreset) -> Self {
        let name=match species {
            PlantPreset::Oak=>"Oak",
            PlantPreset::Pine=>"Pine",
            PlantPreset::Birch=>"Birch",
            PlantPreset::Willow=>"Willow",
            PlantPreset::Maple=>"Maple",
            PlantPreset::Shrub=>"Oak",
        };
        let mut out=Self::default();
        if let Ok(presets)=builtin_presets() {
            for (is_bark, variants) in [
                (true, if species==PlantPreset::Willow {
                    vec!["Willow bark","Bark / trunk sides"]
                } else {vec!["Bark / trunk sides","Bark"]}),
                (false, if species==PlantPreset::Willow {
                    vec!["Willow hanging leaf clusters","Willow leaves"]
                } else if species==PlantPreset::Pine {
                    vec!["Leaf-only canopy","Mature foliage"]
                } else {vec!["Leaf-only canopy","Mature foliage","Leaves"]}),
            ] {
                let chosen=presets.iter().find(|p|
                    p.material==name && variants.iter().any(|variant|p.variant==*variant));
                if let Some(preset)=chosen {
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

    /// Turn the original cutout silhouette into tinted, fully procedural RGBA
    /// foliage. RGB comes from the leaf graph; an optional opacity graph can
    /// punch extra holes but never fill transparent background.
    pub fn bake_leaf(&self, size:u32, leaf_tint:[u8;3]) -> Result<Vec<u8>,String> {
        self.validate()?;
        if size==0 || size>1024 {return Err("Foliage bake must be 1..1024 px".into());}
        let base=leaf_cutout_rgba();
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
                if let Some(color)=&graph {
                    for ch in 0..3 {
                        // The source leaf design retains its finely varied
                        // brightness, and the preset supplies the RGB palette.
                        rgba[dst+ch]=((color[dst+ch] as f32)*
                            (0.72+0.28*base[src] as f32/255.0)).round() as u8;
                    }
                } else {
                    for ch in 0..3 {
                        rgba[dst+ch]=((leaf_tint[ch] as f32)*
                            (0.72+0.28*base[src] as f32/255.0)).round() as u8;
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
        if let Some(graph)=&self.bark {graph.render_rgba(size,size)}
        else {Ok(bark_tint.iter().copied().chain(std::iter::once(255))
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
            mats.validate().unwrap();
        }
    }

    #[test]
    fn bake_preserves_alpha_gaps_and_uses_preset_palette() {
        let mat=FoliageMaterials::for_species(PlantPreset::Oak);
        let rgba=mat.bake_leaf(32,[0,255,0]).unwrap();
        assert_eq!(rgba.len(),32*32*4);
        assert!(rgba.chunks_exact(4).any(|p|p[3]==0));
        assert!(rgba.chunks_exact(4).any(|p|p[3]==255));
        assert!(rgba.chunks_exact(4).any(|p|p[3]==255 && p[0]!=0));
        let mask=mat.bake_leaf_opacity(32,[0,255,0]).unwrap();
        assert!(mask.chunks_exact(4).all(|p|p[0]==p[1]&&p[1]==p[2]&&p[3]==255));
        assert_eq!(mat.bake_bark(16,[0;3]).unwrap().len(),16*16*4);
        assert!(!mat.bake_pbr(8).unwrap().is_empty());
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
    fn legacy_materials_and_invalid_thresholds() {
        let old:FoliageMaterials=serde_json::from_str("{}").unwrap();
        assert!(old.bark.is_none());
        assert_eq!(old.cutout_threshold,0.5);
        let mut m=old;
        m.cutout_threshold=f32::NAN;
        assert!(m.validate().is_err());
        let _=LeafGeometry::CrossedCutout;
    }
}
