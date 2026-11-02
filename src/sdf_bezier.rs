//! Bounded cubic Bézier material strokes. Pixel evaluation remains analytic
//! in source UV: no pre-rasterized intermediate images or hidden texture atlas.

pub const MIN_BEZIER_SEGMENTS:u8=8;
pub const MAX_BEZIER_SEGMENTS:u8=64;

fn cubic(points:[[f32;2];4],t:f32)->[f32;2] {
    let s=1.0-t;
    let basis=[s*s*s,3.0*s*s*t,3.0*s*t*t,t*t*t];
    std::array::from_fn(|i|
        (0..4).map(|j|basis[j]*points[j][i]).sum())
}

/// Tiled curves use the shortest wrapped displacement between adjacent
/// control points; unwrapped offsets may exceed 1.0 internally.
fn unwrap(points:[[f32;2];4])->[[f32;2];4] {
    let mut out=points;
    for i in 1..4 {
        for axis in 0..2 {
            let delta=points[i][axis]-points[i-1][axis];
            out[i][axis]=out[i-1][axis]+delta-delta.round();
        }
    }
    out
}

/// Curve centerline used by the editor's visual handles. Shares the exact
/// cubic and shortest-wrap convention with the signed-distance evaluator.
pub fn centerline(points:[[f32;2];4],t:f32,tiled:bool)->[f32;2] {
    let points=if tiled {unwrap(points)} else {points};
    cubic(points,t.clamp(0.0,1.0))
}

fn segment_distance(px:f32,py:f32,a:[f32;2],b:[f32;2],
    radius_start:f32,radius_end:f32)->f32 {
    let dx=b[0]-a[0];
    let dy=b[1]-a[1];
    let length_sq=dx*dx+dy*dy;
    let t=if length_sq>1e-12 {
        ((px-a[0])*dx+(py-a[1])*dy)/length_sq
    } else {0.0}.clamp(0.0,1.0);
    let cx=a[0]+dx*t;
    let cy=a[1]+dy*t;
    let radius=radius_start+(radius_end-radius_start)*t;
    ((px-cx).powi(2)+(py-cy).powi(2)).sqrt()-radius
}

/// Approximate signed distance to a cubic Bézier tapered stroke.
///
/// The centerline is sampled into 8..64 finite segments; signed distance
/// is estimated from nearest segment centerline minus interpolated radius.
/// The sign and interpolation are deterministic, but the off-contour
/// distance is approximate, especially where taper changes quickly.
pub fn stroke(u:f32,v:f32,points:[[f32;2];4],
    start_radius:f32,end_radius:f32,segments:u8,tiled:bool)->f32 {
    let control=if tiled {unwrap(points)} else {points};
    let count=segments as usize;
    let mut previous=control[0];
    let mut best=f32::MAX;
    for i in 1..=count {
        let t=i as f32/count as f32;
        let next=cubic(control,t);
        let r0=start_radius+(end_radius-start_radius)*(i-1) as f32/count as f32;
        let r1=start_radius+(end_radius-start_radius)*t;
        if tiled {
            // The unwrapped curve can travel beyond one tile even though
            // each saved control point lies in [0,1]. Center the searched
            // copies on this segment, not on an arbitrary origin tile.
            let tile_x=(u-previous[0]).round();
            let tile_y=(v-previous[1]).round();
            for y_shift in -1..=1 {
                for x_shift in -1..=1 {
                    best=best.min(segment_distance(
                        u-tile_x-x_shift as f32,
                        v-tile_y-y_shift as f32,previous,next,r0,r1));
                }
            }
        } else {
            best=best.min(segment_distance(u,v,previous,next,r0,r1));
        }
        previous=next;
    }
    best
}

pub fn valid_points(points:[[f32;2];4])->bool {
    points.iter().all(|p|crate::sdf::valid_uv(*p))
}

#[cfg(test)]
mod tests {
    use super::*;
    const LINE:[[f32;2];4]=[
        [0.2,0.5],[0.4,0.5],[0.6,0.5],[0.8,0.5],
    ];

    #[test]
    fn simple_cubic_stroke_has_correct_sign_and_taper() {
        let start=stroke(0.2,0.59,LINE,0.12,0.02,24,false);
        let end=stroke(0.8,0.59,LINE,0.12,0.02,24,false);
        assert!(start<0.0,"thick start should cover point");
        assert!(end>0.0,"thin end should not cover point");
        assert!(stroke(0.5,0.5,LINE,0.06,0.06,24,false)<0.0);
        assert!(stroke(0.5,0.9,LINE,0.06,0.06,24,false)>0.0);
    }

    #[test]
    fn crossing_the_seam_is_continuous_when_tiled() {
        let cross=[
            [0.88,0.5],[0.96,0.5],[0.04,0.5],[0.12,0.5],
        ];
        let a=stroke(0.0,0.5,cross,0.06,0.06,24,true);
        let b=stroke(1.0,0.5,cross,0.06,0.06,24,true);
        assert!(a<0.0,"wrapped curve should cover seam");
        assert!((a-b).abs()<1e-6,"opposing texture edges must match");
        let clamped=stroke(0.0,0.5,cross,0.06,0.06,24,false);
        assert!(clamped>a,"non-tiled mode must keep authored unwrapped path");
    }

    #[test]
    fn long_unwrapped_control_chain_still_tiles_at_seams() {
        let control=[
            [0.1,0.1],[0.55,0.55],[0.01,0.01],[0.46,0.46],
        ];
        let at_zero=stroke(0.0,0.0,control,0.05,0.05,16,true);
        let at_one=stroke(1.0,1.0,control,0.05,0.05,16,true);
        assert!((at_zero-at_one).abs()<1e-5);
    }

    #[test]
    fn straight_line_approximates_capsule_with_constant_radius() {
        // Capsule connects endpoints along the shortest periodic path.
        // Keep this Bézier's endpoint span under 0.5 UV to compare the
        // same actual centerline rather than two different wrapped routes.
        let short=[
            [0.30,0.5],[0.40,0.5],[0.50,0.5],[0.60,0.5],
        ];
        for x in [0.30,0.40,0.50,0.60] {
            let actual=stroke(x,0.56,short,0.07,0.07,24,true);
            let capsule=crate::sdf::capsule(x,0.56,short[0],short[3],0.07);
            assert!((actual-capsule).abs()<0.002);
        }
    }

    #[test]
    fn editor_centerline_uses_same_seam_unwrap_as_evaluator() {
        let points=[[0.9,0.5],[0.97,0.5],[0.03,0.5],[0.1,0.5]];
        let midpoint=centerline(points,0.5,true);
        assert!((midpoint[0]-1.0).abs()<1e-6);
        assert!((midpoint[1]-0.5).abs()<1e-6);
        let unwrapped=centerline(points,0.5,false);
        assert!((unwrapped[0]-0.5).abs()<1e-6);
        assert_eq!(centerline(points,-1.0,true),points[0]);
    }

    #[test]
    fn authored_coordinates_must_be_normalized() {
        assert!(valid_points(LINE));
        let mut wrong=LINE;
        wrong[2][0]=f32::NAN;
        assert!(!valid_points(wrong));
        wrong[2][0]=1.1;
        assert!(!valid_points(wrong));
    }
}
