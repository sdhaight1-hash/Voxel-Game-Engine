//! CPU-executable node collection inspired by the official Material Maker library.
//! These are original implementations, not imported Material Maker node code.
//! All operations sample the input graph lazily, preserving existing JSON projects.

use serde::{Deserialize, Serialize};
use std::f32::consts::{PI, TAU};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LibraryNode {
    LinearGradient,
    RadialGradient,
    Circle,
    RoundedRectangle,
    Polygon,
    SineWave,
    WhiteNoise,
    ColorNoise,
    ValueNoise,
    FractalNoise,
    Clouds,
    Crystal,
    Dirt,
    Beehive,
    Weave,
    DiagonalWeave,
    Truchet,
    Fibers,
    Scratches,
    Spots,
    Triangles,
    Grayscale,
    BrightnessContrast,
    Gamma,
    Threshold,
    Smoothstep,
    Posterize,
    Saturation,
    HueShift,
    ColorTint,
    Tones,
    Solarize,
    Clamp,
    Abs,
    Sin,
    Cos,
    Power,
    Normalize,
    ColorChannels,
    Quantize,
    Vignette,
    EdgeDetect,
    Blur,
    Sharpen,
    Emboss,
    Dilate,
    Erode,
    Pixelize,
    NormalMap,
    InvertNormals,
    DirectionalBlur,
    Add,
    Subtract,
    Multiply,
    Divide,
    Min,
    Max,
    Difference,
    Screen,
    Overlay,
    SoftLight,
    HardLight,
    Mask,
    Lerp,
    Switch,
    Translate,
    Rotate,
    Scale,
    Mirror,
    Kaleidoscope,
    Warp,
    Swirl,
    Shear,
    Tile,
    WaveDistort,
    Spherize,
}

impl LibraryNode {
    pub const ALL: &'static [Self] = &[
        Self::LinearGradient,
        Self::RadialGradient,
        Self::Circle,
        Self::RoundedRectangle,
        Self::Polygon,
        Self::SineWave,
        Self::WhiteNoise,
        Self::ColorNoise,
        Self::ValueNoise,
        Self::FractalNoise,
        Self::Clouds,
        Self::Crystal,
        Self::Dirt,
        Self::Beehive,
        Self::Weave,
        Self::DiagonalWeave,
        Self::Truchet,
        Self::Fibers,
        Self::Scratches,
        Self::Spots,
        Self::Triangles,
        Self::Grayscale,
        Self::BrightnessContrast,
        Self::Gamma,
        Self::Threshold,
        Self::Smoothstep,
        Self::Posterize,
        Self::Saturation,
        Self::HueShift,
        Self::ColorTint,
        Self::Tones,
        Self::Solarize,
        Self::Clamp,
        Self::Abs,
        Self::Sin,
        Self::Cos,
        Self::Power,
        Self::Normalize,
        Self::ColorChannels,
        Self::Quantize,
        Self::Vignette,
        Self::EdgeDetect,
        Self::Blur,
        Self::Sharpen,
        Self::Emboss,
        Self::Dilate,
        Self::Erode,
        Self::Pixelize,
        Self::NormalMap,
        Self::InvertNormals,
        Self::DirectionalBlur,
        Self::Add,
        Self::Subtract,
        Self::Multiply,
        Self::Divide,
        Self::Min,
        Self::Max,
        Self::Difference,
        Self::Screen,
        Self::Overlay,
        Self::SoftLight,
        Self::HardLight,
        Self::Mask,
        Self::Lerp,
        Self::Switch,
        Self::Translate,
        Self::Rotate,
        Self::Scale,
        Self::Mirror,
        Self::Kaleidoscope,
        Self::Warp,
        Self::Swirl,
        Self::Shear,
        Self::Tile,
        Self::WaveDistort,
        Self::Spherize,
    ];

    pub fn category(self) -> &'static str {
        match self {
            Self::LinearGradient | Self::RadialGradient | Self::Circle | Self::RoundedRectangle | Self::Polygon | Self::SineWave => "Shapes & Gradients",
            Self::WhiteNoise | Self::ColorNoise | Self::ValueNoise | Self::FractalNoise | Self::Clouds | Self::Crystal | Self::Dirt => "Noise",
            Self::Beehive | Self::Weave | Self::DiagonalWeave | Self::Truchet | Self::Fibers | Self::Scratches | Self::Spots | Self::Triangles => "Patterns",
            Self::Grayscale | Self::BrightnessContrast | Self::Gamma | Self::Threshold | Self::Smoothstep | Self::Posterize | Self::Saturation | Self::HueShift | Self::ColorTint | Self::Tones | Self::Solarize | Self::Clamp | Self::Abs | Self::Sin | Self::Cos | Self::Power | Self::Normalize | Self::ColorChannels | Self::Quantize | Self::Vignette => "Color & Tones",
            Self::EdgeDetect | Self::Blur | Self::Sharpen | Self::Emboss | Self::Dilate | Self::Erode | Self::Pixelize | Self::NormalMap | Self::InvertNormals | Self::DirectionalBlur => "Filters",
            Self::Add | Self::Subtract | Self::Multiply | Self::Divide | Self::Min | Self::Max | Self::Difference | Self::Screen | Self::Overlay | Self::SoftLight | Self::HardLight | Self::Mask | Self::Lerp | Self::Switch => "Math & Mix",
            Self::Translate | Self::Rotate | Self::Scale | Self::Mirror | Self::Kaleidoscope | Self::Warp | Self::Swirl | Self::Shear | Self::Tile | Self::WaveDistort | Self::Spherize => "Transforms",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::LinearGradient => "Linear Gradient",
            Self::RadialGradient => "Radial Gradient",
            Self::Circle => "Circle",
            Self::RoundedRectangle => "Rounded Rectangle",
            Self::Polygon => "Polygon",
            Self::SineWave => "Sine Wave",
            Self::WhiteNoise => "White Noise",
            Self::ColorNoise => "Color Noise",
            Self::ValueNoise => "Value Noise",
            Self::FractalNoise => "Fractal Noise",
            Self::Clouds => "Clouds",
            Self::Crystal => "Crystal",
            Self::Dirt => "Dirt",
            Self::Beehive => "Beehive",
            Self::Weave => "Weave",
            Self::DiagonalWeave => "Diagonal Weave",
            Self::Truchet => "Truchet",
            Self::Fibers => "Fibers",
            Self::Scratches => "Scratches",
            Self::Spots => "Spots",
            Self::Triangles => "Triangles",
            Self::Grayscale => "Grayscale",
            Self::BrightnessContrast => "Brightness Contrast",
            Self::Gamma => "Gamma",
            Self::Threshold => "Threshold",
            Self::Smoothstep => "Smoothstep",
            Self::Posterize => "Posterize",
            Self::Saturation => "Saturation",
            Self::HueShift => "Hue Shift",
            Self::ColorTint => "Color Tint",
            Self::Tones => "Tones",
            Self::Solarize => "Solarize",
            Self::Clamp => "Clamp",
            Self::Abs => "Abs",
            Self::Sin => "Sin",
            Self::Cos => "Cos",
            Self::Power => "Power",
            Self::Normalize => "Normalize",
            Self::ColorChannels => "Color Channels",
            Self::Quantize => "Quantize",
            Self::Vignette => "Vignette",
            Self::EdgeDetect => "Edge Detect",
            Self::Blur => "Blur",
            Self::Sharpen => "Sharpen",
            Self::Emboss => "Emboss",
            Self::Dilate => "Dilate",
            Self::Erode => "Erode",
            Self::Pixelize => "Pixelize",
            Self::NormalMap => "Normal Map",
            Self::InvertNormals => "Invert Normals",
            Self::DirectionalBlur => "Directional Blur",
            Self::Add => "Add",
            Self::Subtract => "Subtract",
            Self::Multiply => "Multiply",
            Self::Divide => "Divide",
            Self::Min => "Min",
            Self::Max => "Max",
            Self::Difference => "Difference",
            Self::Screen => "Screen",
            Self::Overlay => "Overlay",
            Self::SoftLight => "Soft Light",
            Self::HardLight => "Hard Light",
            Self::Mask => "Mask",
            Self::Lerp => "Lerp",
            Self::Switch => "Switch",
            Self::Translate => "Translate",
            Self::Rotate => "Rotate",
            Self::Scale => "Scale",
            Self::Mirror => "Mirror",
            Self::Kaleidoscope => "Kaleidoscope",
            Self::Warp => "Warp",
            Self::Swirl => "Swirl",
            Self::Shear => "Shear",
            Self::Tile => "Tile",
            Self::WaveDistort => "Wave Distort",
            Self::Spherize => "Spherize",
        }
    }

    pub fn input_count(self) -> usize {
        match self {
            Self::LinearGradient | Self::RadialGradient | Self::Circle | Self::RoundedRectangle | Self::Polygon | Self::SineWave | Self::WhiteNoise | Self::ColorNoise | Self::ValueNoise | Self::FractalNoise | Self::Clouds | Self::Crystal | Self::Dirt | Self::Beehive | Self::Weave | Self::DiagonalWeave | Self::Truchet | Self::Fibers | Self::Scratches | Self::Spots | Self::Triangles => 0,
            Self::Add | Self::Subtract | Self::Multiply | Self::Divide | Self::Min | Self::Max | Self::Difference | Self::Screen | Self::Overlay | Self::SoftLight | Self::HardLight | Self::Mask | Self::Lerp | Self::Switch => 2,
            _ => 1,
        }
    }

    pub fn input_name(self, input: usize) -> &'static str {
        if self.input_count() == 2 {
            if input == 0 { "A" } else { "B" }
        } else { "Image" }
    }

    /// Parameter "amount" is normalized 0..1 and seed is deterministic.
    /// Sampling coordinates wrap to tile seamlessly, where meaningful.
    pub fn evaluate(
        self, u: f32, v: f32, amount: f32, seed: u32,
        input: impl Fn(usize, f32, f32) -> [f32; 3],
    ) -> [f32; 3] {
        use LibraryNode::*;
        let amount = amount.clamp(0.0, 1.0);
        let freq = ((amount * 24.0) + 2.0).round() as u32;
        let gray = |v: f32| [v.clamp(0.0, 1.0); 3];
        let source = || input(0, u, v);
        let scalar = |u: f32, v: f32| luminance(input(0, u, v));
        match self {
            LinearGradient => gray(u.rem_euclid(1.0)),
            RadialGradient => {
                let x = u.rem_euclid(1.0) - 0.5;
                let y = v.rem_euclid(1.0) - 0.5;
                gray((1.0 - (x*x+y*y).sqrt() * 1.6).max(0.0))
            }
            Circle => {
                let x = u.rem_euclid(1.0)-0.5;
                let y = v.rem_euclid(1.0)-0.5;
                gray(if x*x+y*y <= (0.12+amount*0.32).powi(2) {1.0} else {0.0})
            }
            RoundedRectangle => {
                let x = (u.rem_euclid(1.0)-0.5).abs();
                let y = (v.rem_euclid(1.0)-0.5).abs();
                let r = 0.05+amount*0.2;
                let dx = (x-(0.35-r)).max(0.0);
                let dy = (y-(0.35-r)).max(0.0);
                gray(if dx*dx+dy*dy <= r*r && x <= 0.35 && y <= 0.35 {1.0} else {0.0})
            }
            Polygon => {
                let x=u.rem_euclid(1.0)-0.5;
                let y=v.rem_euclid(1.0)-0.5;
                let count=(3.0+amount*9.0).round();
                let a=y.atan2(x).rem_euclid(TAU);
                let sector=(a*count/TAU).round()*TAU/count;
                let radial=(x*x+y*y).sqrt() * (a-sector).cos();
                gray(if radial < 0.34*(PI/count).cos() {1.0} else {0.0})
            }
            SineWave => gray((TAU*(u*freq as f32+v)).sin()*0.5+0.5),
            WhiteNoise => gray(crate::lattice((u*freq as f32).floor() as i32,
                (v*freq as f32).floor() as i32,freq as i32,seed)),
            ColorNoise => [
                crate::periodic_noise(u,v,freq,seed),
                crate::periodic_noise(u,v,freq,seed.wrapping_add(0x5523)),
                crate::periodic_noise(u,v,freq,seed.wrapping_add(0x9bda))],
            ValueNoise => gray(crate::periodic_noise(u,v,freq,seed)),
            FractalNoise => {
                let mut sum=0.0; let mut weight=1.0; let mut total=0.0;
                for i in 0..5 {
                    sum += crate::periodic_noise(u,v,freq<<i,seed.wrapping_add(i)) * weight;
                    total += weight; weight *= 0.5;
                }
                gray(sum/total)
            }
            Clouds => {
                let n=crate::periodic_noise(u,v,freq,seed);
                let detail=crate::periodic_noise(u,v,freq*4,seed.wrapping_add(17));
                gray(((n*0.8+detail*0.2)-0.25)*1.7)
            }
            Crystal => {
                let d=cell_distance(u,v,freq as i32,seed);
                gray((1.0-d*1.6).max(0.0).powf(0.7))
            }
            Dirt => {
                let n=crate::periodic_noise(u,v,freq,seed);
                let spots=crate::lattice((u*freq as f32*3.0) as i32,
                    (v*freq as f32*3.0) as i32,(freq*3) as i32,seed+3);
                gray((n*0.7+spots*0.3).powf(2.0))
            }
            Beehive => {
                let n=freq as f32;
                let row=(v*n).floor();
                let x=(u*n+((row as i32 & 1) as f32)*0.5).fract()-0.5;
                let y=(v*n).fract()-0.5;
                let edge=(x.abs()*0.95+y.abs()*0.55).max(y.abs()*1.1);
                gray(if edge < 0.39 {1.0} else {0.0})
            }
            Weave | DiagonalWeave => {
                let (u,v)=if self==DiagonalWeave {(u+v,u-v)} else {(u,v)};
                let u=(u*freq as f32).rem_euclid(1.0);
                let v=(v*freq as f32).rem_euclid(1.0);
                let cx=u < 0.35 || u > 0.65;
                let cy=v < 0.35 || v > 0.65;
                gray(if cx == cy {0.82} else {0.17})
            }
            Truchet => {
                let x=u*freq as f32; let y=v*freq as f32;
                let ix=x.floor() as i32;let iy=y.floor() as i32;
                let px=x.fract();let py=y.fract();
                let twist=crate::lattice(ix,iy,freq as i32,seed)>0.5;
                let d=if twist {(px*px+py*py).sqrt()} else {
                    ((1.0-px).powi(2)+py*py).sqrt()
                };
                gray(if (d-0.5).abs() < 0.12 {1.0} else {0.0})
            }
            Fibers => {
                let wobble=crate::periodic_noise(u,v,5,seed)*0.15;
                gray((TAU*(u*freq as f32+wobble)).sin().abs().powf(2.0))
            }
            Scratches => {
                let n=crate::lattice((u*freq as f32*2.0).floor() as i32,
                    (v*freq as f32*2.0).floor() as i32,
                    (freq*2) as i32,seed);
                let line=(u*freq as f32*2.0).fract();
                gray(if n>0.66 && line<0.12 {1.0} else {0.0})
            }
            Spots => {
                let d=cell_distance(u,v,freq as i32,seed);
                gray(if d < 0.25 {1.0} else {0.0})
            }
            Triangles => {
                let x=(u*freq as f32).fract();
                let y=(v*freq as f32).fract();
                gray(if y<x {0.85} else {0.15})
            }

            Grayscale => gray(luminance(source())),
            BrightnessContrast => source().map(|x| ((x-0.5)*(amount*2.0+0.25)+0.5).clamp(0.0,1.0)),
            Gamma => source().map(|x| x.max(0.0).powf(0.25+amount*3.75)),
            Threshold => gray(if luminance(source()) >= amount {1.0} else {0.0}),
            Smoothstep => gray(smooth(0.25*amount,1.0-0.25*amount,luminance(source()))),
            Posterize | Quantize => source().map(|x| {
                let steps=(2.0+amount*14.0).round();
                (x*steps).round()/steps
            }),
            Saturation => {
                let c=source();let y=luminance(c);c.map(|x|y+(x-y)*amount*2.0)
            }
            HueShift => hue_shift(source(),amount),
            ColorTint => {
                let tint=[1.0,0.28+amount*0.65,0.25+amount*0.75];
                let c=source();[c[0]*tint[0],c[1]*tint[1],c[2]*tint[2]]
            }
            Tones => source().map(|x|smooth(0.1*amount,1.0-0.1*amount,x)),
            Solarize => source().map(|x| if x>amount {1.0-x} else {x}),
            Clamp => source().map(|x|x.clamp(0.0,amount.max(0.01))),
            Abs => source().map(|x|(x*2.0-1.0).abs()),
            Sin => source().map(|x|(TAU*(x*(1.0+amount*10.0))).sin()*0.5+0.5),
            Cos => source().map(|x|(TAU*(x*(1.0+amount*10.0))).cos()*0.5+0.5),
            Power => source().map(|x|x.max(0.0).powf(0.25+4.0*amount)),
            Normalize => {
                let c=source(); let max=c.iter().copied().fold(0.0_f32,f32::max).max(0.00001);
                c.map(|x|x/max)
            }
            ColorChannels => {
                let c=source();
                [c[1],c[2],c[0]]
            }
            Vignette => {
                let x=u.rem_euclid(1.0);let y=v.rem_euclid(1.0);
                let weight=(1.0-amount*(1.0-(TAU*x).cos())*0.25)
                    *(1.0-amount*(1.0-(TAU*y).cos())*0.25);
                source().map(|x|x*weight)
            }
            EdgeDetect => {
                let delta=0.003+amount*0.012;
                let gx=scalar(u+delta,v)-scalar(u-delta,v);
                let gy=scalar(u,v+delta)-scalar(u,v-delta);
                gray(((gx*gx+gy*gy).sqrt()*5.0).clamp(0.0,1.0))
            }
            Blur | DirectionalBlur => {
                let radius=0.002+amount*0.019;
                let mut c=[0.0;3];let mut count=0.0;
                for i in -2..=2 {
                    for j in -2..=2 {
                        if self==DirectionalBlur && j!=0 {continue;}
                        let p=input(0,u+i as f32*radius,v+j as f32*radius);
                        for k in 0..3 {c[k]+=p[k];}
                        count+=1.0;
                    }
                }
                c.map(|x|x/count)
            }
            Sharpen | Emboss => {
                let d=0.003+amount*0.008;
                let center=source();let left=input(0,u-d,v);
                let right=input(0,u+d,v);
                let upper=input(0,u,v-d);let lower=input(0,u,v+d);
                if self==Sharpen {std::array::from_fn(|i|
                    (center[i]*5.0-left[i]-right[i]-upper[i]-lower[i]).clamp(0.0,1.0))
                } else {std::array::from_fn(|i|
                    (0.5+right[i]-left[i]+lower[i]-upper[i]).clamp(0.0,1.0))}
            }
            Dilate | Erode => {
                let radius=0.002+amount*0.02;
                let mut c=source();
                for dx in -1..=1 {
                    for dy in -1..=1 {
                        let other=input(0,u+dx as f32*radius,v+dy as f32*radius);
                        for k in 0..3 {
                            c[k]=if self==Dilate {c[k].max(other[k])} else {c[k].min(other[k])};
                        }
                    }
                }
                c
            }
            Pixelize => {
                let cells=(3.0+amount*60.0).round();
                input(0,((u*cells).floor()+0.5)/cells,((v*cells).floor()+0.5)/cells)
            }
            NormalMap => {
                let d=0.003+amount*0.01;
                let dx=scalar(u-d,v)-scalar(u+d,v);
                let dy=scalar(u,v-d)-scalar(u,v+d);
                let scale=1.0+amount*7.0;
                let len=((dx*scale).powi(2)+(dy*scale).powi(2)+1.0).sqrt();
                [0.5+dx*scale/len*0.5,0.5+dy*scale/len*0.5,0.5+0.5/len]
            }
            InvertNormals => {
                let c=source();[1.0-c[0],1.0-c[1],c[2]]
            }
            Add | Subtract | Multiply | Divide | Min | Max | Difference | Screen |
            Overlay | SoftLight | HardLight | Mask | Lerp | Switch => {
                let a=source();let b=input(1,u,v);
                std::array::from_fn(|i| (match self {
                    Add => (a[i]+b[i]*amount).min(1.0),
                    Subtract => (a[i]-b[i]*amount).max(0.0),
                    Multiply => a[i]*b[i],
                    Divide => (a[i]/b[i].max(0.001)).min(1.0),
                    Min => a[i].min(b[i]),
                    Max => a[i].max(b[i]),
                    Difference => (a[i]-b[i]).abs(),
                    Screen => 1.0-(1.0-a[i])*(1.0-b[i]),
                    Overlay => if a[i]<0.5 {2.0*a[i]*b[i]} else {1.0-2.0*(1.0-a[i])*(1.0-b[i])},
                    SoftLight => (1.0-2.0*b[i])*a[i]*a[i]+2.0*b[i]*a[i],
                    HardLight => if b[i]<0.5 {2.0*a[i]*b[i]} else {1.0-2.0*(1.0-a[i])*(1.0-b[i])},
                    Mask => a[i]*luminance(b),
                    Lerp => a[i]*(1.0-amount)+b[i]*amount,
                    Switch => if amount<0.5 {a[i]} else {b[i]},
                    _ => unreachable!()
                }).clamp(0.0,1.0))
            }
            Translate => input(0,u+amount*0.5,v+amount*0.31),
            Rotate => {
                let a=amount*TAU;let c=a.cos();let s=a.sin();
                let x=u-0.5;let y=v-0.5;
                input(0,x*c-y*s+0.5,x*s+y*c+0.5)
            }
            Scale => {
                let f=0.5+amount*4.0;input(0,(u-0.5)*f+0.5,(v-0.5)*f+0.5)
            }
            Mirror => {
                let x=u.rem_euclid(1.0); let y=v.rem_euclid(1.0);
                input(0,if x<0.5 {2.0*x} else {2.0-2.0*x},
                    if amount>0.5 {if y<0.5 {2.0*y} else {2.0-2.0*y}} else {y})
            }
            Kaleidoscope => {
                let dx=u-0.5;let dy=v-0.5;
                let count=(2.0+amount*14.0).round();let unit=TAU/count;
                let mut a=dy.atan2(dx).rem_euclid(unit);
                if a>unit*0.5 {a=unit-a;}
                let r=(dx*dx+dy*dy).sqrt();
                input(0,0.5+r*a.cos(),0.5+r*a.sin())
            }
            Warp => {
                let dx=scalar(u+0.13,v+0.29)-0.5;
                let dy=scalar(u+0.39,v+0.09)-0.5;
                input(0,u+dx*amount*0.3,v+dy*amount*0.3)
            }
            Swirl => {
                let x=u-0.5;let y=v-0.5;let r=(x*x+y*y).sqrt();
                let a=(1.0-r*2.0).max(0.0)*amount*TAU*2.0;
                input(0,0.5+x*a.cos()-y*a.sin(),0.5+x*a.sin()+y*a.cos())
            }
            Shear => input(0,u+amount*(v-0.5),v),
            Tile => {
                let repeat=(1.0+amount*11.0).round();
                input(0,u*repeat,v*repeat)
            }
            WaveDistort => {
                let amount=amount*0.17;
                input(0,u+amount*(TAU*v*6.0).sin(),v+amount*(TAU*u*6.0).cos())
            }
            Spherize => {
                let x=(u-0.5)*2.0;let y=(v-0.5)*2.0;
                let r=(x*x+y*y).sqrt().min(1.0);
                let f=(1.0-amount)+amount*(1.0-r*r*0.8).sqrt();
                input(0,0.5+x*f*0.5,0.5+y*f*0.5)
            }
        }
    }
}

fn luminance(c:[f32;3])->f32 {
    (c[0]*0.2126+c[1]*0.7152+c[2]*0.0722).clamp(0.0,1.0)
}

fn smooth(a:f32,b:f32,x:f32)->f32 {
    let t=((x-a)/(b-a).max(0.00001)).clamp(0.0,1.0);
    t*t*(3.0-2.0*t)
}

fn cell_distance(u:f32,v:f32,period:i32,seed:u32)->f32 {
    let x=u*period as f32;let y=v*period as f32;
    let ix=x.floor() as i32;let iy=y.floor() as i32;
    let mut best=f32::INFINITY;
    for dy in -1..=1 {
        for dx in -1..=1 {
            let cx=ix+dx;let cy=iy+dy;
            let rx=crate::lattice(cx,cy,period,seed);
            let ry=crate::lattice(cx,cy,period,seed^0x8bad_f00d);
            let px=cx as f32+rx-x;
            let py=cy as f32+ry-y;
            best=best.min((px*px+py*py).sqrt());
        }
    }
    best
}

fn hue_shift(c:[f32;3], amount:f32)->[f32;3] {
    let min=c.iter().copied().fold(f32::INFINITY,f32::min);
    let max=c.iter().copied().fold(f32::NEG_INFINITY,f32::max);
    let delta=max-min;
    if delta<0.00001 {return c;}
    let mut hue=if max==c[0] {(c[1]-c[2])/delta}
    else if max==c[1] {(c[2]-c[0])/delta+2.0}
    else {(c[0]-c[1])/delta+4.0};
    hue=(hue/6.0+amount).rem_euclid(1.0)*6.0;
    let saturation=delta/max.max(0.00001);
    let value=max;
    let chroma=value*saturation;
    let x=chroma*(1.0-((hue%2.0)-1.0).abs());
    let (r,g,b)=if hue<1.0 {(chroma,x,0.0)}
    else if hue<2.0 {(x,chroma,0.0)}
    else if hue<3.0 {(0.0,chroma,x)}
    else if hue<4.0 {(0.0,x,chroma)}
    else if hue<5.0 {(x,0.0,chroma)}
    else {(chroma,0.0,x)};
    let m=value-chroma;
    [r+m,g+m,b+m]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Graph,NodeKind};

    #[test]
    fn every_library_node_builds_and_renders() {
        for &operation in LibraryNode::ALL {
            let mut graph=Graph::default();
            let id=graph.add_node(NodeKind::Library {
                operation, amount:0.5, seed:17
            },[10.0,10.0]);
            graph.connect(1,id,0).ok();
            if operation.input_count()>1 {graph.connect(1,id,1).unwrap();}
            graph.connect(id,2,0).unwrap();
            graph.validate().unwrap();
            let pixels=graph.render_rgba(4,4).unwrap();
            assert_eq!(pixels.len(),64,"{}",operation.name());
        }
    }

    #[test]
    fn categories_are_present_and_distinct() {
        let cats:std::collections::HashSet<_>=LibraryNode::ALL.iter().map(|n|n.category()).collect();
        assert_eq!(cats.len(),7);
        assert_eq!(LibraryNode::ALL.len(),76);
    }

    #[test]
    fn binary_ops_compute_actual_results() {
        let mul=LibraryNode::Multiply.evaluate(0.2,0.4,0.5,3,
            |index,_,_|if index==0 {[0.5;3]} else {[0.25;3]});
        assert_eq!(mul,[0.125;3]);
        let added=LibraryNode::Add.evaluate(0.2,0.4,1.0,3,
            |index,_,_|if index==0 {[0.5;3]} else {[0.25;3]});
        assert_eq!(added,[0.75;3]);
    }

    #[test]
    fn transforms_operate_on_input_coordinates() {
        let moved=LibraryNode::Translate.evaluate(0.2,0.4,0.5,3,
            |_,u,v|[u,v,0.0]);
        assert!((moved[0]-0.45).abs()<0.001);
        assert!((moved[1]-0.555).abs()<0.001);
    }
}
