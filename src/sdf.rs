//! Tileable UV-space signed-distance patterns. Distances stay as signed
//! normalized-UV floats until an explicit Distance -> Mask conversion.
use serde::{Deserialize,Serialize};

#[derive(Debug,Clone,Copy,PartialEq,Eq,Serialize,Deserialize)]
pub enum SdfBoolean { Union, Intersection, Subtract }

fn delta(u:f32,center:f32)->f32 {
    let d=u-center;
    d-d.round()
}

pub fn circle(u:f32,v:f32,center:[f32;2],radius:f32)->f32 {
    let dx=delta(u,center[0]);
    let dy=delta(v,center[1]);
    (dx*dx+dy*dy).sqrt()-radius
}

/// First-order ellipse signed distance: exact zero-isocontour and sign,
/// approximate Euclidean distance off the boundary. For edge coverage this
/// avoids the expensive iterative nearest-point solution per texture pixel.
pub fn ellipse(u:f32,v:f32,center:[f32;2],radii:[f32;2])->f32 {
    let x=delta(u,center[0]).abs();
    let y=delta(v,center[1]).abs();
    let rx=radii[0];
    let ry=radii[1];
    let k0=((x/rx).powi(2)+(y/ry).powi(2)).sqrt();
    let k1=((x/(rx*rx)).powi(2)+(y/(ry*ry)).powi(2)).sqrt();
    if k1==0.0 {-rx.min(ry)} else {k0*(k0-1.0)/k1}
}

/// A periodic capsule between the *shortest wrapped* endpoint positions.
/// Search neighboring copies so shapes crossing a tile boundary are continuous.
pub fn capsule(u:f32,v:f32,start:[f32;2],end:[f32;2],radius:f32)->f32 {
    let dx=delta(end[0],start[0]);
    let dy=delta(end[1],start[1]);
    let length_sq=dx*dx+dy*dy;
    let mut best=f32::MAX;
    for oy in -1..=1 {
        for ox in -1..=1 {
            let px=u-start[0]-ox as f32;
            let py=v-start[1]-oy as f32;
            let t=if length_sq>1e-12 {
                ((px*dx+py*dy)/length_sq).clamp(0.0,1.0)
            } else {0.0};
            let x=px-t*dx;
            let y=py-t*dy;
            best=best.min((x*x+y*y).sqrt()-radius);
        }
    }
    best
}

fn smooth_min(a:f32,b:f32,k:f32)->f32 {
    if k==0.0 {return a.min(b);}
    let h=(0.5+0.5*(b-a)/k).clamp(0.0,1.0);
    b*(1.0-h)+a*h-k*h*(1.0-h)
}

fn smooth_max(a:f32,b:f32,k:f32)->f32 {
    -smooth_min(-a,-b,k)
}

/// Smooth width is expressed in tile-normalized UV units, not pixels.
pub fn combine(a:f32,b:f32,operation:SdfBoolean,smoothing:f32)->f32 {
    match operation {
        SdfBoolean::Union=>smooth_min(a,b,smoothing),
        SdfBoolean::Intersection=>smooth_max(a,b,smoothing),
        SdfBoolean::Subtract=>smooth_max(a,-b,smoothing),
    }
}

/// Convert signed distance to 0..1 grayscale coverage. Edges are 50%
/// covered at distance zero when width > 0; width zero is a hard cutoff.
pub fn to_mask(distance:f32,width:f32)->f32 {
    if width==0.0 {
        if distance<=0.0 {1.0} else {0.0}
    } else {
        let t=(0.5-distance/width).clamp(0.0,1.0);
        t*t*(3.0-2.0*t)
    }
}

pub fn valid_uv(point:[f32;2])->bool {
    point.iter().all(|n|n.is_finite()&&(0.0..=1.0).contains(n))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn circle_has_signed_interior_exact_boundary_and_periodic_edges() {
        let center=[0.5,0.5];
        assert!((circle(0.5,0.5,center,0.2)+0.2).abs()<1e-6);
        assert!(circle(0.7,0.5,center,0.2).abs()<1e-6);
        assert!(circle(0.5,0.95,center,0.2)>0.0);
        let seam_center=[0.96,0.52];
        assert!((circle(0.0,0.4,seam_center,0.21)
            -circle(1.0,0.4,seam_center,0.21)).abs()<1e-6);
    }

    #[test]
    fn ellipse_is_signed_and_periodic_with_true_zero_boundary() {
        let radii=[0.3,0.1];
        assert!(ellipse(0.5,0.5,[0.5,0.5],radii)<0.0);
        assert!(ellipse(0.8,0.5,[0.5,0.5],radii).abs()<1e-5);
        assert!(ellipse(0.5,0.62,[0.5,0.5],radii)>0.0);
        assert!((ellipse(0.0,0.55,[0.95,0.5],radii)
            -ellipse(1.0,0.55,[0.95,0.5],radii)).abs()<1e-6);
    }

    #[test]
    fn capsule_wraps_segment_across_uv_boundary() {
        let start=[0.92,0.5];
        let end=[0.08,0.5];
        assert!(capsule(0.0,0.5,start,end,0.025)<0.0);
        assert!((capsule(0.0,0.61,start,end,0.025)
            -capsule(1.0,0.61,start,end,0.025)).abs()<1e-6);
    }

    #[test]
    fn booleans_preserve_distance_and_soft_union_is_continuous() {
        let (a,b)=(-0.1,0.2);
        assert_eq!(combine(a,b,SdfBoolean::Union,0.0),a);
        assert_eq!(combine(a,b,SdfBoolean::Intersection,0.0),b);
        assert_eq!(combine(a,b,SdfBoolean::Subtract,0.0),a.max(-b));
        assert!(combine(0.02,0.02,SdfBoolean::Union,0.1)<0.02);
        assert!(combine(0.02,0.02,SdfBoolean::Intersection,0.1)>0.02);
    }

    #[test]
    fn masks_require_explicit_conversion_and_clip_to_unit_range() {
        assert_eq!(to_mask(-1.0,0.0),1.0);
        assert_eq!(to_mask(0.1,0.0),0.0);
        assert_eq!(to_mask(0.0,0.02),0.5);
        assert_eq!(to_mask(-1.0,0.02),1.0);
        assert_eq!(to_mask(1.0,0.02),0.0);
    }
}
