//! Optional Oak leaf-card silhouette authored as signed-distance primitives.
//! Existing species artwork and hexel occupancy remain untouched.
use serde::{Deserialize,Serialize};
use crate::sdf::{self,SdfBoolean};

#[derive(Debug,Clone,Copy,PartialEq,Serialize,Deserialize)]
pub enum LeafSilhouetteSource {
    LegacySpecies,
    /// Four lobed broadleaf forms plus connecting twigs, in 0..1 card UV.
    VectorOak { lobe_width:f32 },
}
impl Default for LeafSilhouetteSource {
    fn default()->Self {Self::LegacySpecies}
}
impl LeafSilhouetteSource {
    pub fn validate(self,is_oak:bool)->Result<(),String> {
        match self {
            Self::LegacySpecies=>Ok(()),
            Self::VectorOak{lobe_width} if is_oak && lobe_width.is_finite()
                && (0.65..=1.45).contains(&lobe_width)=>Ok(()),
            Self::VectorOak{..}=>Err(
                "Vector Oak cutout requires Oak and lobe width 0.65..1.45".into()),
        }
    }
}

fn line_distance(u:f32,v:f32,start:[f32;2],end:[f32;2],radius:f32)->f32 {
    let dx=end[0]-start[0];
    let dy=end[1]-start[1];
    let t=(((u-start[0])*dx+(v-start[1])*dy)/(dx*dx+dy*dy))
        .clamp(0.0,1.0);
    ((u-start[0]-dx*t).powi(2)+(v-start[1]-dy*t).powi(2)).sqrt()-radius
}

fn oak_distance(u:f32,v:f32,lobe_width:f32)->(f32,f32) {
    let mut foliage=f32::MAX;
    for center in [[0.36,0.33],[0.65,0.46],[0.35,0.61],[0.64,0.76]] {
        let core=sdf::ellipse(u,v,center,[0.064*lobe_width,0.104]);
        let left=sdf::ellipse(u,v,[center[0]-0.052,center[1]-0.01],
            [0.057*lobe_width,0.054]);
        let right=sdf::ellipse(u,v,[center[0]+0.052,center[1]-0.01],
            [0.057*lobe_width,0.054]);
        let tip=sdf::ellipse(u,v,[center[0],center[1]-0.077],
            [0.046*lobe_width,0.055]);
        let leaf=sdf::combine(
            sdf::combine(core,left,SdfBoolean::Union,0.008),
            sdf::combine(right,tip,SdfBoolean::Union,0.008),
            SdfBoolean::Union,0.008);
        foliage=foliage.min(leaf);
    }
    let mut twig=line_distance(u,v,[0.50,0.90],[0.50,0.14],0.009);
    for (a,b) in [
        ([0.50,0.40],[0.36,0.33]),
        ([0.50,0.55],[0.65,0.51]),
        ([0.50,0.72],[0.35,0.70]),
    ] {
        twig=twig.min(line_distance(u,v,a,b,0.007));
    }
    (foliage,twig)
}

/// A new *base* silhouette, not a subtractive extra-hole mask. Keep the
/// card's existing binary alpha and shading contract for GPU cutout render.
pub fn vector_oak_cutout_rgba(lobe_width:f32)->Vec<u8> {
    const SIZE:usize=64;
    let mut pixels=Vec::with_capacity(SIZE*SIZE*4);
    for y in 0..SIZE {
        for x in 0..SIZE {
            let u=(x as f32+0.5)/SIZE as f32;
            let v=(y as f32+0.5)/SIZE as f32;
            let (leaf,twig)=oak_distance(u,v,lobe_width);
            let leaves=leaf<=0.0;
            let twig=twig<=0.0;
            let shade=if leaves {
                0.83+0.14*((x*11+y*7)%17) as f32/16.0
            } else {0.58};
            let value=(shade*255.0) as u8;
            pixels.extend_from_slice(&[value,value,value,
                if leaves||twig {255} else {0}]);
        }
    }
    pixels
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::foliage::{leaf_cutout_for_species,PlantPreset};

    #[test]
    fn vector_oak_has_real_leaf_and_empty_card_pixels() {
        let shape=vector_oak_cutout_rgba(1.0);
        let legacy=leaf_cutout_for_species(PlantPreset::Oak);
        assert_eq!(shape.len(),64*64*4);
        let mut added=0;
        for (a,b) in shape.chunks_exact(4).zip(legacy.chunks_exact(4)) {
            assert!(a[3]==0||a[3]==255);
            if a[3]>b[3] {added+=1;}
        }
        assert!(added>0,"vector base shape must add pixels outside the old silhouette");
        assert_ne!(shape,legacy,"opt-in vector silhouette must not copy old art");
    }

    #[test]
    fn changing_lobe_width_changes_geometry_without_affecting_twigs() {
        let narrow=vector_oak_cutout_rgba(0.65);
        let broad=vector_oak_cutout_rgba(1.45);
        assert_ne!(narrow,broad);
        assert!(narrow.chunks_exact(4).zip(broad.chunks_exact(4))
            .all(|(a,b)|a[3]<=b[3]),
            "wider lobes should only add coverage");
    }

    #[test]
    fn non_oak_and_invalid_lobe_params_are_rejected() {
        assert!(LeafSilhouetteSource::LegacySpecies.validate(false).is_ok());
        assert!(LeafSilhouetteSource::VectorOak{lobe_width:1.0}
            .validate(true).is_ok());
        assert!(LeafSilhouetteSource::VectorOak{lobe_width:1.0}
            .validate(false).is_err());
        assert!(LeafSilhouetteSource::VectorOak{lobe_width:f32::NAN}
            .validate(true).is_err());
        assert!(LeafSilhouetteSource::VectorOak{lobe_width:0.1}
            .validate(true).is_err());
    }
}
