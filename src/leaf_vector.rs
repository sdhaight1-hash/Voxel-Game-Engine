//! Opt-in tree leaf-card silhouettes authored as signed-distance primitives.
//! Existing species artwork and hexel occupancy remain untouched.
use serde::{Deserialize,Serialize};
use crate::sdf::{self,SdfBoolean};
use crate::foliage::PlantPreset;
use crate::sdf_bezier;

#[derive(Debug,Clone,Copy,PartialEq,Serialize,Deserialize)]
pub enum LeafSilhouetteSource {
    LegacySpecies,
    /// Four lobed broadleaf forms plus connecting twigs, in 0..1 card UV.
    VectorOak { lobe_width:f32 },
    /// Hanging curved, tapered blades in normalized card UV.
    VectorWillow { blade_width:f32 },
    /// Species-aware deltoid or ovate blade outlines in normalized card UV.
    VectorCottonwood { blade_width:f32 },
}
impl Default for LeafSilhouetteSource {
    fn default()->Self {Self::LegacySpecies}
}
impl LeafSilhouetteSource {
    pub fn validate(self,species:PlantPreset)->Result<(),String> {
        let valid_width=|value:f32|value.is_finite() &&
            (0.65..=1.45).contains(&value);
        match self {
            Self::LegacySpecies=>Ok(()),
            Self::VectorOak{lobe_width} if species==PlantPreset::Oak &&
                valid_width(lobe_width)=>Ok(()),
            Self::VectorWillow{blade_width} if species==PlantPreset::Willow &&
                valid_width(blade_width)=>Ok(()),
            Self::VectorCottonwood{blade_width}
                if matches!(species,PlantPreset::EasternCottonwood
                    |PlantPreset::FremontCottonwood|PlantPreset::BlackCottonwood)
                    && valid_width(blade_width)=>Ok(()),
            _=>Err("Vector silhouette must match its species and have width 0.65..1.45".into()),
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
    rasterize_leaf_card(|u,v|oak_distance(u,v,lobe_width),false)
}

/// Existing callers continue to use single-center binary coverage; the
/// opt-in path evaluates four subpixel points before RGBA8 quantization.
fn rasterize_leaf_card(mut distance:impl FnMut(f32,f32)->(f32,f32),
    coverage_2x2:bool)->Vec<u8> {
    const SIZE:usize=64;
    let mut pixels=Vec::with_capacity(SIZE*SIZE*4);
    for y in 0..SIZE {
        for x in 0..SIZE {
            let u=(x as f32+0.5)/SIZE as f32;
            let v=(y as f32+0.5)/SIZE as f32;
            let (leaf,twig)=distance(u,v);
            let leaves=leaf<=0.0;
            let twig=twig<=0.0;
            let shade=if leaves {
                0.83+0.14*((x*11+y*7)%17) as f32/16.0
            } else {0.58};
            let value=(shade*255.0) as u8;
            let alpha=if coverage_2x2 {
                let mut inside=0u32;
                for oy in [0.25,0.75] {
                    for ox in [0.25,0.75] {
                        let (l,t)=distance((x as f32+ox)/SIZE as f32,
                            (y as f32+oy)/SIZE as f32);
                        if l<=0.0||t<=0.0 {inside+=1;}
                    }
                }
                ((inside*255+2)/4) as u8
            } else if leaves||twig {255} else {0};
            pixels.extend_from_slice(&[value,value,value,alpha]);
        }
    }
    pixels
}

/// Willow's long hanging blades are tapered cubic strokes. Four control
/// points stay in ordinary card UV, with no automatic tile repetition.
fn willow_distance(u:f32,v:f32,blade_width:f32)->(f32,f32) {
    let mut leaves=f32::MAX;
    let mut twig=line_distance(u,v,[0.50,0.12],[0.50,0.90],0.008);
    for index in 0..5 {
        let y=0.19+index as f32*0.133;
        for side in [-1.0f32,1.0] {
            let root=[0.50,y];
            let start=[0.50+side*0.035,y+0.015];
            let points=[
                start,
                [0.50+side*0.065,y+0.060],
                [0.50+side*0.145,y+0.095],
                [0.50+side*0.205,y+0.155],
            ];
            leaves=leaves.min(sdf_bezier::stroke(u,v,points,
                0.027*blade_width,0.003*blade_width,16,false));
            twig=twig.min(line_distance(u,v,root,start,0.006));
        }
    }
    (leaves,twig)
}

/// This is a silhouette-only polygon SDF: distance to nearest straight edge
/// with sign from even-odd crossing. No tessellation or external atlas.
fn polygon_distance(u:f32,v:f32,center:[f32;2],radii:[f32;2],
    vertices:&[[f32;2]])->f32 {
    let mut nearest=f32::MAX;
    let mut inside=false;
    for i in 0..vertices.len() {
        let a=[center[0]+radii[0]*vertices[i][0],
            center[1]+radii[1]*vertices[i][1]];
        let next=vertices[(i+1)%vertices.len()];
        let b=[center[0]+radii[0]*next[0],
            center[1]+radii[1]*next[1]];
        nearest=nearest.min(line_distance(u,v,a,b,0.0));
        if (a[1]>v)!=(b[1]>v) &&
            u<(b[0]-a[0])*(v-a[1])/(b[1]-a[1])+a[0] {
            inside=!inside;
        }
    }
    if inside {-nearest} else {nearest}
}

const DELTOID:[[f32;2];12]=[
    [0.0,-1.0],[0.28,-0.73],[0.62,-0.43],[1.0,-0.11],
    [0.83,0.14],[0.43,0.54],[0.0,1.0],
    [-0.43,0.54],[-0.83,0.14],[-1.0,-0.11],
    [-0.62,-0.43],[-0.28,-0.73],
];
const OVATE:[[f32;2];12]=[
    [0.0,-1.0],[0.50,-0.83],[0.86,-0.47],[1.0,0.0],
    [0.85,0.46],[0.48,0.83],[0.0,1.0],
    [-0.48,0.83],[-0.85,0.46],[-1.0,0.0],
    [-0.86,-0.47],[-0.50,-0.83],
];

fn cottonwood_distance(u:f32,v:f32,species:PlantPreset,
    blade_width:f32)->(f32,f32) {
    let black=species==PlantPreset::BlackCottonwood;
    let fremont=species==PlantPreset::FremontCottonwood;
    let vertices=if black {&OVATE[..]} else {&DELTOID[..]};
    let rx=if black {0.078} else if fremont {0.108} else {0.098};
    let ry=if black {0.128} else {0.103};
    let mut leaves=f32::MAX;
    let mut twig=line_distance(u,v,[0.50,0.14],[0.50,0.92],0.008);
    for center in [[0.35,0.30],[0.67,0.47],[0.34,0.63],[0.66,0.77]] {
        leaves=leaves.min(polygon_distance(u,v,center,
            [rx*blade_width,ry],vertices));
        twig=twig.min(line_distance(u,v,[0.50,center[1]+0.08],
            [center[0],center[1]+0.065],0.006));
    }
    (leaves,twig)
}

pub fn vector_willow_cutout_rgba(blade_width:f32)->Vec<u8> {
    rasterize_leaf_card(|u,v|willow_distance(u,v,blade_width),false)
}

pub fn vector_cottonwood_cutout_rgba(species:PlantPreset,
    blade_width:f32)->Vec<u8> {
    rasterize_leaf_card(|u,v|cottonwood_distance(u,v,species,blade_width),false)
}

/// Four subpixel occupancy tests per 64px texel. Only supported for
/// authored vector silhouettes: historical species cards are untouched.
pub fn vector_subpixel_cutout_rgba(source:LeafSilhouetteSource,
    species:PlantPreset)->Result<Vec<u8>,String> {
    source.validate(species)?;
    match source {
        LeafSilhouetteSource::VectorOak{lobe_width}=>
            Ok(rasterize_leaf_card(|u,v|oak_distance(u,v,lobe_width),true)),
        LeafSilhouetteSource::VectorWillow{blade_width} =>
            Ok(rasterize_leaf_card(|u,v|willow_distance(u,v,blade_width),true)),
        LeafSilhouetteSource::VectorCottonwood{blade_width} =>
            Ok(rasterize_leaf_card(|u,v|
                cottonwood_distance(u,v,species,blade_width),true)),
        LeafSilhouetteSource::LegacySpecies =>
            Err("Subpixel silhouette coverage requires a vector source".into()),
    }
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
    fn subpixel_coverage_is_fractional_opt_in_and_deterministic() {
        for (species,source) in [
            (PlantPreset::Oak,LeafSilhouetteSource::VectorOak{lobe_width:1.0}),
            (PlantPreset::Willow,LeafSilhouetteSource::VectorWillow{blade_width:1.0}),
            (PlantPreset::EasternCottonwood,
                LeafSilhouetteSource::VectorCottonwood{blade_width:1.0}),
            (PlantPreset::FremontCottonwood,
                LeafSilhouetteSource::VectorCottonwood{blade_width:1.0}),
            (PlantPreset::BlackCottonwood,
                LeafSilhouetteSource::VectorCottonwood{blade_width:1.0}),
        ] {
            let covered=vector_subpixel_cutout_rgba(source,species).unwrap();
            let repeated=vector_subpixel_cutout_rgba(source,species).unwrap();
            assert_eq!(covered,repeated);
            assert_eq!(covered.len(),64*64*4);
            assert!(covered.chunks_exact(4)
                .any(|px|px[3]>0&&px[3]<255),
                "{species:?} needs intermediate edge coverage");
            assert!(covered.chunks_exact(4)
                .all(|p|[0,64,128,191,255].contains(&p[3])),
                "{species:?} needs four-sample quantization");
            assert!(covered.chunks_exact(4).any(|p|p[3]==0));
            assert!(covered.chunks_exact(4).any(|p|p[3]==255));
        }
        assert!(vector_subpixel_cutout_rgba(
            LeafSilhouetteSource::LegacySpecies,PlantPreset::Oak).is_err());
    }

    #[test]
    fn willow_vector_blades_really_replace_old_art_and_width_is_monotone() {
        let legacy=leaf_cutout_for_species(PlantPreset::Willow);
        let slim=vector_willow_cutout_rgba(0.65);
        let broad=vector_willow_cutout_rgba(1.45);
        assert_eq!(broad.len(),64*64*4);
        assert_ne!(broad,legacy);
        assert!(broad.chunks_exact(4).any(|p|p[3]==0));
        assert!(broad.chunks_exact(4).any(|p|p[3]==255));
        assert!(broad.chunks_exact(4).zip(legacy.chunks_exact(4))
            .any(|(a,b)|a[3]>b[3]),"vector Willow must replace the base outline");
        assert!(slim.chunks_exact(4).zip(broad.chunks_exact(4))
            .all(|(a,b)|a[3]<=b[3]));
    }

    #[test]
    fn cottonwood_vector_outlines_are_species_specific_and_width_monotone() {
        let mut images=Vec::new();
        for species in [
            PlantPreset::EasternCottonwood,
            PlantPreset::FremontCottonwood,
            PlantPreset::BlackCottonwood,
        ] {
            let old=leaf_cutout_for_species(species);
            let narrow=vector_cottonwood_cutout_rgba(species,0.65);
            let wide=vector_cottonwood_cutout_rgba(species,1.45);
            assert_ne!(old,wide,"{species:?} must have a real new outline");
            assert!(wide.chunks_exact(4).any(|p|p[3]==0));
            assert!(wide.chunks_exact(4).any(|p|p[3]==255));
            assert!(narrow.chunks_exact(4).zip(wide.chunks_exact(4))
                .all(|(a,b)|a[3]<=b[3]),"{species:?} widening must not remove leaf pixels");
            assert!(wide.chunks_exact(4).zip(old.chunks_exact(4))
                .any(|(a,b)|a[3]>b[3]),"{species:?} must be a replacement");
            images.push(vector_cottonwood_cutout_rgba(species,1.0));
        }
        assert_ne!(images[0],images[1]);
        assert_ne!(images[0],images[2]);
        assert_ne!(images[1],images[2]);
    }

    #[test]
    fn vector_species_validation_rejects_mismatches_and_invalid_widths() {
        let willow=LeafSilhouetteSource::VectorWillow{blade_width:1.0};
        assert!(willow.validate(PlantPreset::Willow).is_ok());
        assert!(willow.validate(PlantPreset::Oak).is_err());
        let cotton=LeafSilhouetteSource::VectorCottonwood{blade_width:1.0};
        for species in [PlantPreset::EasternCottonwood,
            PlantPreset::FremontCottonwood,PlantPreset::BlackCottonwood] {
            assert!(cotton.validate(species).is_ok());
        }
        assert!(cotton.validate(PlantPreset::Willow).is_err());
        for bad in [f32::NAN,f32::INFINITY,0.64,1.46] {
            assert!(LeafSilhouetteSource::VectorWillow{blade_width:bad}
                .validate(PlantPreset::Willow).is_err());
            assert!(LeafSilhouetteSource::VectorCottonwood{blade_width:bad}
                .validate(PlantPreset::BlackCottonwood).is_err());
        }
    }

    #[test]
    fn non_oak_and_invalid_lobe_params_are_rejected() {
        assert!(LeafSilhouetteSource::LegacySpecies.validate(PlantPreset::Willow).is_ok());
        assert!(LeafSilhouetteSource::VectorOak{lobe_width:1.0}
            .validate(PlantPreset::Oak).is_ok());
        assert!(LeafSilhouetteSource::VectorOak{lobe_width:1.0}
            .validate(PlantPreset::Willow).is_err());
        assert!(LeafSilhouetteSource::VectorOak{lobe_width:f32::NAN}
            .validate(PlantPreset::Oak).is_err());
        assert!(LeafSilhouetteSource::VectorOak{lobe_width:0.1}
            .validate(PlantPreset::Oak).is_err());
    }
}
