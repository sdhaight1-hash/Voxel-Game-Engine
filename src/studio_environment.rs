//! Deterministic, linear-RGB studio environment for the optional bark IBL preview.
//! CPU preprocessing runs once when enabled, never per material edit or frame.
//! The 2D atlas stores five GGX-prefiltered lat-long bands with a 1px guard
//! row above/below each band. No HDR asset, shader loops or external crate.
use std::f32::consts::{PI,TAU};

pub const WIDTH:u32=48;
pub const HEIGHT:u32=24;
pub const LEVELS:u32=5;
pub const BAND_HEIGHT:u32=HEIGHT+2;
pub const ATLAS_HEIGHT:u32=BAND_HEIGHT*LEVELS;
const SAMPLES:u32=48;

#[derive(Clone)]
pub struct StudioEnvironment {
    pub specular:Vec<u8>,
    pub irradiance:Vec<u8>,
}

fn dot(a:[f32;3],b:[f32;3])->f32 {
    a[0]*b[0]+a[1]*b[1]+a[2]*b[2]
}
fn cross(a:[f32;3],b:[f32;3])->[f32;3] {
    [a[1]*b[2]-a[2]*b[1],a[2]*b[0]-a[0]*b[2],
     a[0]*b[1]-a[1]*b[0]]
}
fn norm(a:[f32;3])->[f32;3] {
    let length=dot(a,a).sqrt();
    if length<1e-7 {return [0.0,0.0,1.0];}
    [a[0]/length,a[1]/length,a[2]/length]
}
fn basis(n:[f32;3])->([f32;3],[f32;3]) {
    let reference=if n[2].abs()<0.9 {[0.0,0.0,1.0]}
        else {[0.0,1.0,0.0]};
    let t=norm(cross(reference,n));
    (t,cross(n,t))
}
fn world(n:[f32;3],t:[f32;3],b:[f32;3],v:[f32;3])->[f32;3] {
    norm(std::array::from_fn(|i|t[i]*v[0]+b[i]*v[1]+n[i]*v[2]))
}
pub fn latlong_dir(x:u32,y:u32)->[f32;3] {
    let phi=TAU*((x as f32+0.5)/WIDTH as f32-0.5);
    let theta=PI*(y as f32+0.5)/HEIGHT as f32;
    [theta.sin()*phi.cos(),theta.sin()*phi.sin(),theta.cos()]
}
/// A bounded *linear* sky/ground and two soft studio illumination panels.
/// Values are <= 1 for direct Rgba8Unorm data storage (not sRGB).
fn radiance(dir:[f32;3])->[f32;3] {
    let sky=dir[2].clamp(0.0,1.0);
    let ground=(-dir[2]).clamp(0.0,1.0);
    let key=dot(dir,norm([0.46,0.35,0.81])).max(0.0).powf(28.0);
    let fill=dot(dir,norm([-0.8,0.16,0.58])).max(0.0).powf(12.0);
    [
        0.11+0.10*sky-0.07*ground+0.70*key+0.17*fill,
        0.14+0.15*sky-0.09*ground+0.65*key+0.20*fill,
        0.19+0.21*sky-0.12*ground+0.55*key+0.25*fill,
    ].map(|v|v.clamp(0.0,1.0))
}
fn sample(i:u32,count:u32)->(f32,f32) {
    (i as f32/count as f32,
     i.reverse_bits() as f32 / 4294967296.0)
}
fn diffuse_irradiance(normal:[f32;3])->[f32;3] {
    let (t,b)=basis(normal);
    let mut sum=[0.0;3];
    // Cosine-weighted hemisphere importance sampling. The average is
    // irradiance divided by PI, ready for the Lambert diffuse BRDF.
    for i in 0..SAMPLES {
        let (u,v)=sample(i,SAMPLES);
        let phi=TAU*u;
        let radius=v.sqrt();
        let direction=world(normal,t,b,[
            radius*phi.cos(),radius*phi.sin(),(1.0-v).sqrt()]);
        let light=radiance(direction);
        for k in 0..3 {sum[k]+=light[k];}
    }
    sum.map(|v|v/SAMPLES as f32)
}
fn prefiltered_specular(reflection:[f32;3],roughness:f32)->[f32;3] {
    if roughness==0.0 {return radiance(reflection);}
    let (t,b)=basis(reflection);
    let alpha=(roughness*roughness).max(0.001);
    let alpha2=alpha*alpha;
    let mut sum=[0.0;3];
    let mut total=0.0;
    // GGX half-vector importance sample, V=N=reflection approximation.
    // Radiance is integrated once on CPU, not on every GPU fragment.
    for i in 0..SAMPLES {
        let (u,v)=sample(i,SAMPLES);
        let phi=TAU*u;
        let cos_theta=((1.0-v)/(1.0+(alpha2-1.0)*v)).sqrt();
        let sin_theta=(1.0-cos_theta*cos_theta).max(0.0).sqrt();
        let h=world(reflection,t,b,[
            sin_theta*phi.cos(),sin_theta*phi.sin(),cos_theta]);
        let vh=dot(reflection,h).max(0.0);
        let l=norm(std::array::from_fn(|k|2.0*vh*h[k]-reflection[k]));
        let weight=dot(reflection,l).max(0.0);
        if weight>0.0 {
            let color=radiance(l);
            for k in 0..3 {sum[k]+=color[k]*weight;}
            total+=weight;
        }
    }
    if total<=1e-7 {return radiance(reflection);}
    sum.map(|v|v/total)
}
fn pixel(color:[f32;3],out:&mut Vec<u8>) {
    for component in color {
        out.push((component.clamp(0.0,1.0)*255.0).round() as u8);
    }
    out.push(255);
}
pub fn generate()->StudioEnvironment {
    let mut specular=Vec::with_capacity((WIDTH*ATLAS_HEIGHT*4) as usize);
    let mut irradiance=Vec::with_capacity((WIDTH*HEIGHT*4) as usize);
    for level in 0..LEVELS {
        let roughness=level as f32/(LEVELS-1) as f32;
        for guarded_y in 0..BAND_HEIGHT {
            let y=guarded_y.saturating_sub(1).min(HEIGHT-1);
            for x in 0..WIDTH {
                pixel(prefiltered_specular(latlong_dir(x,y),roughness),
                      &mut specular);
            }
        }
    }
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            pixel(diffuse_irradiance(latlong_dir(x,y)),&mut irradiance);
        }
    }
    StudioEnvironment{specular,irradiance}
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn studio_ibl_maps_are_bounded_deterministic_and_guarded() {
        let a=generate();
        let b=generate();
        assert_eq!(a.specular,b.specular);
        assert_eq!(a.irradiance,b.irradiance);
        assert_eq!(a.specular.len(),(WIDTH*ATLAS_HEIGHT*4) as usize);
        assert_eq!(a.irradiance.len(),(WIDTH*HEIGHT*4) as usize);
        assert!(a.specular.chunks_exact(4).all(|c|c[3]==255));
        assert!(a.irradiance.chunks_exact(4).all(|c|c[3]==255));
        for level in 0..LEVELS {
            for x in 0..WIDTH {
                let stride=(WIDTH*4) as usize;
                let top=((level*BAND_HEIGHT)*WIDTH*4+x*4) as usize;
                let first=top+stride;
                let bottom=((level*BAND_HEIGHT+HEIGHT+1)*WIDTH*4+x*4)
                    as usize;
                let last=bottom-stride;
                assert_eq!(&a.specular[top..top+4],&a.specular[first..first+4]);
                assert_eq!(&a.specular[bottom..bottom+4],
                    &a.specular[last..last+4]);
            }
        }
    }
    #[test]
    fn roughness_filters_a_bright_reflection_and_diffuse_uses_sky() {
        let key=norm([0.46,0.35,0.81]);
        let smooth=prefiltered_specular(key,0.0);
        let rough=prefiltered_specular(key,1.0);
        assert!(smooth[0]>rough[0] && smooth[1]>rough[1]);
        assert!(rough.iter().all(|c|c.is_finite()&&*c>0.0&&*c<=1.0));
        let sky=diffuse_irradiance([0.0,0.0,1.0]);
        let ground=diffuse_irradiance([0.0,0.0,-1.0]);
        assert!(sky[2]>ground[2]);
    }
}
