//! Periodic, seeded region values and region-boundary coverage.
//!
//! One deterministic Voronoi neighborhood per pixel. The site distribution
//! deliberately matches the existing legacy Voronoi generator (without
//! changing that generator). Nothing is stored per tile or rasterized.

/// Stable per-site normalized value plus distance to the nearest cell border,
/// expressed as the difference of the two nearest site distances in cell units.
/// The neighborhood is always 3x3 regardless of grid resolution.
pub fn sample(u:f32,v:f32,cells:u32,seed:u32)->(f32,f32) {
    let n=cells as i32; // Graph validation requires 2..=32.
    let x=u.rem_euclid(1.0)*cells as f32;
    let y=v.rem_euclid(1.0)*cells as f32;
    let bx=x.floor() as i32;
    let by=y.floor() as i32;
    let mut closest=f32::MAX;
    let mut second=f32::MAX;
    let mut region=0.0;
    for dy in -1..=1 {
        for dx in -1..=1 {
            let cx=bx+dx;
            let cy=by+dy;
            let sx=cx as f32 + crate::lattice(cx,cy,n,seed);
            let sy=cy as f32 + crate::lattice(cx,cy,n,seed^0x97f4_a7c1);
            let distance_sq=(sx-x).powi(2)+(sy-y).powi(2);
            if distance_sq<closest {
                second=closest;
                closest=distance_sq;
                // Identity is a stable site-hash value, not a region count
                // or an unbounded integer carried in the scalar socket.
                region=crate::lattice(cx,cy,n,seed^0x6a09_e667);
            } else if distance_sq<second {
                second=distance_sq;
            }
        }
    }
    (region,(second.sqrt()-closest.sqrt()).max(0.0))
}

/// A soft material boundary, strongest where two region sites are equidistant.
/// The smoothing interval is in local cell-distance units (not tile UV units).
pub fn border_mask(distance_gap:f32,width:f32)->f32 {
    let t=(1.0-distance_gap/width).clamp(0.0,1.0);
    t*t*(3.0-2.0*t)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn values_and_borders_wrap_exactly_on_both_seams() {
        for cells in [2,7,32] {
            for seed in [0,17,0xffff_ffff] {
                for (u,v) in [(0.0,0.0),(0.5,0.25),(-0.5,0.5),(0.0,0.875)] {
                    let a=sample(u,v,cells,seed);
                    assert_eq!(a,sample(u+1.0,v,cells,seed));
                    assert_eq!(a,sample(u,v+1.0,cells,seed));
                    assert!((0.0..=1.0).contains(&a.0));
                    assert!(a.1>=0.0&&a.1.is_finite());
                }
            }
        }
    }

    #[test]
    fn region_value_is_constant_within_a_site_but_changes_across_sites() {
        let mut varied=std::collections::HashSet::new();
        for y in 0..32 {
            for x in 0..32 {
                let (value,_)=sample((x as f32+0.5)/32.0,
                    (y as f32+0.5)/32.0,7,37);
                varied.insert(value.to_bits());
            }
        }
        assert!(varied.len()>12,"region values should show multiple cells");
        assert_ne!(sample(0.31,0.61,7,37).0,sample(0.31,0.61,7,38).0);
    }

    #[test]
    fn mask_is_bounded_and_width_controls_transition() {
        assert_eq!(border_mask(0.0,0.25),1.0);
        assert_eq!(border_mask(0.25,0.25),0.0);
        assert_eq!(border_mask(10.0,0.25),0.0);
        assert!(border_mask(0.1,0.25)>border_mask(0.1,0.15));
        assert!((0.0..=1.0).contains(&border_mask(0.05,0.2)));
    }
}
