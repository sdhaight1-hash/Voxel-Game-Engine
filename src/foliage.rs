//! Native, deterministic procedural vegetation geometry.
//! Inspired by the workflow in UE5.7 Procedural Vegetation, but independent
//! of Unreal assets, Nanite, GPU shaders or skeletal animation.
//!
//! Stages: preset -> carve -> gravity / phototropism -> slope -> scale ->
//! remove branches -> mesh builder -> foliage palette -> distributor -> output.

use serde::{Deserialize, Serialize};
use std::f32::consts::TAU;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlantPreset { Oak, Pine, Birch, Willow, Maple, Shrub }

impl PlantPreset {
    pub const ALL: [Self; 6] = [Self::Oak,Self::Pine,Self::Birch,Self::Willow,Self::Maple,Self::Shrub];
    pub fn name(self) -> &'static str { match self {
        Self::Oak => "Oak",Self::Pine => "Pine",Self::Birch => "Birch",
        Self::Willow => "Willow",Self::Maple => "Maple",Self::Shrub => "Shrub"
    }}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FoliageStage {
    PresetLoader, Carve, Gravity, Slope, Scale, RemoveBranches,
    MeshBuilder, FoliagePalette, FoliageDistributor, Output,
}
impl FoliageStage {
    pub const ALL: [Self;10] = [
        Self::PresetLoader,Self::Carve,Self::Gravity,Self::Slope,Self::Scale,
        Self::RemoveBranches,Self::MeshBuilder,Self::FoliagePalette,
        Self::FoliageDistributor,Self::Output
    ];
    pub fn name(self) -> &'static str { match self {
        Self::PresetLoader=>"Preset Loader",
        Self::Carve=>"Carve",
        Self::Gravity=>"Gravity / Phototropism",
        Self::Slope=>"Slope",
        Self::Scale=>"Scale",
        Self::RemoveBranches=>"Remove Branches",
        Self::MeshBuilder=>"Mesh Builder",
        Self::FoliagePalette=>"Foliage Palette",
        Self::FoliageDistributor=>"Foliage Distributor",
        Self::Output=>"Output",
    }}
    pub fn group(self) -> &'static str { match self {
        Self::PresetLoader|Self::Carve|Self::Gravity|Self::Slope|Self::Scale|
        Self::RemoveBranches => "SKELETON",
        Self::MeshBuilder=>"GEOMETRY",Self::FoliagePalette|Self::FoliageDistributor=>"FOLIAGE",
        Self::Output=>"EXPORT"
    }}
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
    pub carve_bottom: f32,
    pub carve_top: f32,
    pub gravity: f32,
    pub phototropism: f32,
    pub slope: f32,
    pub overall_scale: f32,
    pub prune: f32,
    pub radial_segments: u32,
    pub leaf_density: f32,
    pub leaf_scale: f32,
    pub leaf_angle: f32,
    pub leaf_variation: f32,
    pub bark_color: [u8;3],
    pub leaf_color: [u8;3],
}

impl Default for FoliageProject {
    fn default() -> Self { Self::from_preset(PlantPreset::Oak) }
}

impl FoliageProject {
    pub fn from_preset(preset: PlantPreset) -> Self {
        let (height, radius, branches, branch_length, gravity, density, leaf_scale, bark, leaves) =
            match preset {
                PlantPreset::Oak => (7.0,0.35,26,0.65,0.12,0.75,0.42,[91,67,45],[67,135,52]),
                PlantPreset::Pine => (9.0,0.27,35,0.54,0.07,0.8,0.33,[89,71,52],[40,94,58]),
                PlantPreset::Birch => (8.0,0.22,23,0.44,0.07,0.64,0.32,[198,193,172],[96,158,54]),
                PlantPreset::Willow => (7.2,0.28,28,0.75,0.7,0.94,0.49,[91,86,61],[91,156,65]),
                PlantPreset::Maple => (7.0,0.32,29,0.66,0.1,0.8,0.46,[97,70,51],[112,163,52]),
                PlantPreset::Shrub => (2.8,0.17,32,0.73,0.18,0.95,0.26,[91,70,48],[67,142,51]),
            };
        Self {
            preset,seed:42,height,trunk_radius:radius,branch_count:branches,
            branch_length,carve_bottom:0.15,carve_top:0.02,
            gravity,phototropism:0.15,slope:0.0,overall_scale:1.0,prune:0.0,
            radial_segments:6,leaf_density:density,leaf_scale,
            leaf_angle:0.12,leaf_variation:0.32,bark_color:bark,leaf_color:leaves
        }
    }

    pub fn validate(&self) -> Result<(),String> {
        for (name,x,min,max) in [
            ("height",self.height,0.5,30.0),
            ("trunk radius",self.trunk_radius,0.02,2.0),
            ("branch length",self.branch_length,0.1,1.5),
            ("carve bottom",self.carve_bottom,0.0,0.85),
            ("carve top",self.carve_top,0.0,0.85),
            ("gravity",self.gravity,-1.0,1.0),
            ("phototropism",self.phototropism,0.0,1.0),
            ("slope",self.slope,-0.8,0.8),
            ("overall scale",self.overall_scale,0.1,5.0),
            ("prune",self.prune,0.0,1.0),
            ("leaf density",self.leaf_density,0.0,1.0),
            ("leaf scale",self.leaf_scale,0.02,1.5),
            ("leaf angle",self.leaf_angle,-1.0,1.0),
            ("leaf variation",self.leaf_variation,0.0,1.0)
        ] {
            if !x.is_finite() || x<min || x>max {
                return Err(format!("Foliage {name} must be within {min}..{max}"));
            }
        }
        if self.carve_bottom+self.carve_top >= 0.95 {
            return Err("Carve bottom plus top must leave at least 5% of the tree".into());
        }
        if !(1..=90).contains(&self.branch_count) {
            return Err("Branch count must be 1..90".into());
        }
        if !(3..=12).contains(&self.radial_segments) {
            return Err("Mesh radial segments must be 3..12".into());
        }
        Ok(())
    }

    pub fn generate(&self) -> Result<FoliageMesh,String> {
        self.validate()?;
        let mut mesh=FoliageMesh { triangles:Vec::new(), height:self.height*self.overall_scale };
        let h=self.height;
        let trunk_z = |t:f32| -> [f32;3] {
            let tilt=t*t*self.slope*h*0.32;
            [tilt,0.06*h*t*t*(self.gravity-self.phototropism)*0.25,t*h]
        };
        let n=self.radial_segments;
        for i in 0..12 {
            let t0=i as f32/12.0;
            let t1=(i+1) as f32/12.0;
            add_tube(&mut mesh,trunk_z(t0),trunk_z(t1),
                self.trunk_radius*(1.0-t0*0.8).max(0.12),
                self.trunk_radius*(1.0-t1*0.8).max(0.1),
                n,Material::Bark);
        }
        // Deterministic golden-angle branching, independent of rendering frame.
        for i in 0..self.branch_count {
            let rand = |channel:u32| rng01(self.seed,i,channel);
            if rand(0) < self.prune { continue; }
            let t=self.carve_bottom+(1.0-self.carve_bottom-self.carve_top)
                *((i as f32+0.5+0.3*(rand(1)-0.5))/self.branch_count as f32);
            let base=trunk_z(t);
            let theta=TAU*(i as f32*0.6180339+rand(2)*0.18);
            let mut extent=self.branch_length*h*(0.48+0.5*rand(3));
            let vertical=match self.preset {
                PlantPreset::Pine => {extent *= (1.0-t).max(0.13);0.18},
                PlantPreset::Shrub => 0.30,
                PlantPreset::Willow => -0.05,
                PlantPreset::Birch => 0.22,
                _ => 0.22,
            };
            // Gravity pulls down, phototropism raises branch tips toward light.
            let rise=extent*(vertical+0.6*self.phototropism-0.85*self.gravity);
            let end=[base[0]+extent*theta.cos(),base[1]+extent*theta.sin(),
                (base[2]+rise).max(0.05*h)];
            let midpoint=lerp3(base,end,0.6);
            let width=self.trunk_radius*(0.11+0.16*(1.0-t));
            add_tube(&mut mesh,base,midpoint,width,width*0.6,n,Material::Bark);
            add_tube(&mut mesh,midpoint,end,width*0.6,width*0.1,n,Material::Bark);
            // Natural twig split, also follows gravity/phototropism.
            let tangle=theta+(rand(4)-0.5)*1.6;
            let twig_extent=extent*(0.28+0.19*rand(5));
            let tip=[midpoint[0]+twig_extent*tangle.cos(),
                midpoint[1]+twig_extent*tangle.sin(),
                midpoint[2]+twig_extent*(0.3+self.phototropism*0.25-self.gravity*0.7)];
            add_tube(&mut mesh,midpoint,tip,width*0.42,width*0.04,n,Material::Bark);
            // Distribution controls live leaf count, spacing, scale and orientation.
            // Leaves remain actual two-triangle polygons, not green noise pixels.
            let count=(self.leaf_density*12.0).round() as u32;
            for k in 0..count {
                let leaf_index=i*16+k;
                let leaf_rand=|c:u32| rng01(self.seed^0x9e3779b9,leaf_index,c);
                let anchor=if k%3==0 { tip } else {lerp3(midpoint,end,0.4+leaf_rand(0)*0.6)};
                let variation=1.0+self.leaf_variation*(leaf_rand(1)*2.0-1.0);
                let size=self.leaf_scale*variation;
                let a=TAU*leaf_rand(2)+self.leaf_angle*1.2;
                let up=if self.preset==PlantPreset::Willow {-0.4} else {0.28};
                let center=[
                    anchor[0]+size*0.34*a.cos(),
                    anchor[1]+size*0.34*a.sin(),
                    anchor[2]+size*up
                ];
                let v1=[size*a.cos(),size*a.sin(),size*up];
                let v2=[-size*0.38*a.sin(),size*0.38*a.cos(),
                    size*0.25*(leaf_rand(3)-0.5)];
                let p0=sub3(center,v1);
                let p1=add3(center,v2);
                let p2=add3(center,v1);
                let p3=sub3(center,v2);
                let material=match (leaf_rand(4)*3.0) as usize {
                    0=>Material::LeafDark,1=>Material::Leaf,_=>Material::LeafLight
                };
                // A diamond-style leaf, triangulated; pine uses narrower needles.
                let skinny=if self.preset==PlantPreset::Pine {0.35} else if
                    self.preset==PlantPreset::Willow {0.65} else {1.0};
                let p1=add3(center,mul3(sub3(p1,center),skinny));
                let p3=add3(center,mul3(sub3(p3,center),skinny));
                mesh.triangles.push(Triangle{points:[p0,p1,p2],material});
                mesh.triangles.push(Triangle{points:[p0,p2,p3],material});
            }
        }
        for face in &mut mesh.triangles {
            for point in &mut face.points {
                for v in point { *v *= self.overall_scale; }
            }
        }
        Ok(mesh)
    }
}

fn rng01(seed:u32,id:u32,channel:u32)->f32 {
    let mut x=seed ^ id.wrapping_mul(0x9e3779b9) ^ channel.wrapping_mul(0x85ebca6b);
    x^=x>>16;x=x.wrapping_mul(0x7feb352d);
    x^=x>>15;x=x.wrapping_mul(0x846ca68b);x^=x>>16;
    x as f32/u32::MAX as f32
}
fn add3(a:[f32;3],b:[f32;3])->[f32;3]{[a[0]+b[0],a[1]+b[1],a[2]+b[2]]}
fn sub3(a:[f32;3],b:[f32;3])->[f32;3]{[a[0]-b[0],a[1]-b[1],a[2]-b[2]]}
fn mul3(a:[f32;3],s:f32)->[f32;3]{[a[0]*s,a[1]*s,a[2]*s]}
fn lerp3(a:[f32;3],b:[f32;3],t:f32)->[f32;3]{add3(a,mul3(sub3(b,a),t))}
fn cross(a:[f32;3],b:[f32;3])->[f32;3] {
    [a[1]*b[2]-a[2]*b[1],a[2]*b[0]-a[0]*b[2],a[0]*b[1]-a[1]*b[0]]
}
fn unit(a:[f32;3])->[f32;3] {
    let len=(a[0]*a[0]+a[1]*a[1]+a[2]*a[2]).sqrt();
    if len<0.00001 {[0.0,0.0,1.0]} else {mul3(a,1.0/len)}
}
fn add_tube(mesh:&mut FoliageMesh,start:[f32;3],end:[f32;3],
            radius0:f32,radius1:f32,segments:u32,material:Material) {
    let axis=unit(sub3(end,start));
    let refv=if axis[2].abs()>0.85 {[0.0,1.0,0.0]} else {[0.0,0.0,1.0]};
    let tangent=unit(cross(axis,refv));
    let bitangent=unit(cross(axis,tangent));
    for i in 0..segments {
        let angle=TAU*i as f32/segments as f32;
        let next=TAU*(i+1) as f32/segments as f32;
        let ring=|center:[f32;3],radius:f32,a:f32| -> [f32;3] {
            add3(center,mul3(add3(mul3(tangent,a.cos()),mul3(bitangent,a.sin())),radius))
        };
        let a=ring(start,radius0,angle);let b=ring(start,radius0,next);
        let c=ring(end,radius1,next);let d=ring(end,radius1,angle);
        mesh.triangles.push(Triangle{points:[a,b,c],material});
        mesh.triangles.push(Triangle{points:[a,c,d],material});
    }
}

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum Material { Bark, LeafDark, Leaf, LeafLight }
#[derive(Clone,Debug)]
pub struct Triangle { pub points:[[f32;3];3],pub material:Material }
#[derive(Clone,Debug)]
pub struct FoliageMesh { pub triangles:Vec<Triangle>,pub height:f32 }

impl FoliageMesh {
    pub fn triangle_count(&self)->usize {self.triangles.len()}
    /// Export a real triangular surface mesh; the MTL defines independent bark
    /// and three foliage color variants. No external 3D renderer is required.
    pub fn obj(&self)->String {
        let mut output=String::from("# Texture Studio procedural foliage\nmtllib foliage.mtl\no foliage\n");
        for tri in &self.triangles {
            for p in &tri.points {
                output.push_str(&format!("v {:.6} {:.6} {:.6}\n",p[0],p[1],p[2]));
            }
        }
        let mut previous=None;
        for (i,tri) in self.triangles.iter().enumerate() {
            if previous!=Some(tri.material) {
                let mat=match tri.material {
                    Material::Bark=>"bark",Material::LeafDark=>"leaf_dark",
                    Material::Leaf=>"leaf",Material::LeafLight=>"leaf_light"
                };
                output.push_str(&format!("usemtl {mat}\n"));
                previous=Some(tri.material);
            }
            let p=i*3+1;
            output.push_str(&format!("f {p} {} {}\n",p+1,p+2));
        }
        output
    }
}

pub fn mtl(project:&FoliageProject)->String {
    let bark=project.bark_color;
    let leaf=project.leaf_color;
    let materials=[
        ("bark",bark),
        ("leaf_dark",leaf.map(|c|((c as f32*0.72).round()) as u8)),
        ("leaf",leaf),
        ("leaf_light",leaf.map(|c|((c as f32*1.22).min(255.0).round()) as u8)),
    ];
    let mut text=String::from("# Procedural foliage colors; assign textures in target engine as desired\n");
    for (name,c) in materials {
        text.push_str(&format!("newmtl {name}\nKd {:.4} {:.4} {:.4}\nKa 0.0 0.0 0.0\n\n",
            c[0] as f32/255.0,c[1] as f32/255.0,c[2] as f32/255.0));
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_species_generate_real_meshes() {
        for preset in PlantPreset::ALL {
            let project=FoliageProject::from_preset(preset);
            let mesh=project.generate().unwrap();
            assert!(mesh.triangle_count()>300,"{}",preset.name());
            assert!(mesh.triangles.iter().any(|tri|tri.material!=Material::Bark));
            assert!(mesh.triangles.iter().any(|tri|tri.material==Material::Bark));
            assert!(mesh.triangles.iter().flat_map(|t|t.points)
                .flat_map(|p|p.into_iter()).all(|x|x.is_finite()));
        }
    }
    #[test]
    fn pruning_and_density_control_visible_mesh_contents() {
        let base=FoliageProject::default();
        let full=base.generate().unwrap();
        let mut pruned=base.clone();
        pruned.prune=1.0;
        assert!(pruned.generate().unwrap().triangle_count()<full.triangle_count());
        let mut bare=base.clone();
        bare.leaf_density=0.0;
        assert!(bare.generate().unwrap().triangles.iter()
            .all(|tri|tri.material==Material::Bark));
    }
    #[test]
    fn geometry_is_deterministic_and_obj_export_has_all_triangles() {
        let p=FoliageProject::from_preset(PlantPreset::Willow);
        let a=p.generate().unwrap();
        let b=p.generate().unwrap();
        assert_eq!(a.obj(),b.obj());
        let obj=a.obj();
        assert_eq!(obj.lines().filter(|l|l.starts_with("f ")).count(),a.triangle_count());
        assert_eq!(obj.lines().filter(|l|l.starts_with("v ")).count(),a.triangle_count()*3);
        assert!(obj.contains("usemtl leaf"));
        assert!(mtl(&p).contains("newmtl bark"));
    }
    #[test]
    fn project_round_trips_and_bad_settings_rejected() {
        let mut p=FoliageProject::from_preset(PlantPreset::Pine);
        p.gravity=-0.3;p.seed=u32::MAX;
        let decoded:FoliageProject=serde_json::from_str(&serde_json::to_string(&p).unwrap()).unwrap();
        assert_eq!(decoded.seed,p.seed);
        decoded.generate().unwrap();
        p.carve_bottom=0.7;p.carve_top=0.7;
        assert!(p.validate().is_err());
        p.carve_top=0.0;p.leaf_density=f32::NAN;
        assert!(p.validate().is_err());
    }
}
