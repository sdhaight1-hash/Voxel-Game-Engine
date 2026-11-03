//! Bounded, deterministic pattern splatter in normalized, periodic UV space.
//! A hash-addressed cell is the spatial index: each pixel visits only 3x3
//! nearby cells, independent of total stamp count (at most 32x32 per tile).
//! No baking, per-pixel allocations or second graph evaluator are introduced.

#[derive(Debug, Clone, Copy)]
pub struct ScatterSettings {
    pub cells: u32,
    /// Stamp square width in cell units, not tile UV or world meters.
    pub scale: f32,
    /// Random center offset in cell units (each axis, +/- jitter).
    pub jitter: f32,
    /// Fraction of a full random rotation, 0 = no rotation.
    pub rotation: f32,
    /// Maximum random multiplicative reduction in stamp mask brightness.
    pub brightness: f32,
    pub seed: u32,
}

impl ScatterSettings {
    pub fn validate(self) -> Result<(), String> {
        if !(2..=32).contains(&self.cells)
            || !self.scale.is_finite() || !(0.1..=0.85).contains(&self.scale)
            || !self.jitter.is_finite() || !(0.0..=0.25).contains(&self.jitter)
            || !self.rotation.is_finite() || !(0.0..=1.0).contains(&self.rotation)
            || !self.brightness.is_finite() || !(0.0..=1.0).contains(&self.brightness)
        {
            return Err("Shape Scatter requires 2..32 cells, scale 0.1..0.85, jitter 0..0.25, rotation 0..1 and brightness variation 0..1".into());
        }
        Ok(())
    }
}

fn hash(x: u32, y: u32, seed: u32) -> u32 {
    let mut h = seed ^ x.wrapping_mul(0x9e37_79b9) ^ y.wrapping_mul(0x85eb_ca6b);
    h ^= h >> 16;
    h = h.wrapping_mul(0x7feb_352d);
    h ^= h >> 15;
    h = h.wrapping_mul(0x846c_a68b);
    h ^ (h >> 16)
}

fn random(x: u32, y: u32, seed: u32) -> f32 {
    (hash(x,y,seed) >> 8) as f32 * (1.0 / 16_777_216.0)
}

/// Scatter a connected Mask input as a set of locally transformed stamps.
/// Density is sampled once at each candidate's wrapped center; if unconnected,
/// the graph supplies 1.0. Samples outside the local [0,1] stamp square are
/// skipped, and overlap uses max coverage, never unbounded additive gain.
pub fn sample(
    u: f32, v: f32, settings: ScatterSettings,
    mut pattern: impl FnMut(f32,f32)->f32,
    mut density: impl FnMut(f32,f32)->f32,
) -> f32 {
    let n = settings.cells as i32;
    let x = u.rem_euclid(1.0) * n as f32;
    let y = v.rem_euclid(1.0) * n as f32;
    let cell_x = x.floor() as i32;
    let cell_y = y.floor() as i32;
    let mut coverage: f32 = 0.0;
    // A stamp's largest rotated half-extent is 0.85/sqrt(2) cells.
    // With max 0.25-cell jitter, no cell farther than +/-1 can contribute.
    let max_r2 = (settings.scale * std::f32::consts::FRAC_1_SQRT_2).powi(2);
    for gy in (cell_y-1)..=(cell_y+1) {
        for gx in (cell_x-1)..=(cell_x+1) {
            let hx = gx.rem_euclid(n) as u32;
            let hy = gy.rem_euclid(n) as u32;
            let offset_x = (random(hx,hy,settings.seed ^ 0x243f_6a88)*2.0-1.0)*settings.jitter;
            let offset_y = (random(hx,hy,settings.seed ^ 0x85a3_08d3)*2.0-1.0)*settings.jitter;
            let cx = gx as f32 + 0.5 + offset_x;
            let cy = gy as f32 + 0.5 + offset_y;
            let dx = x-cx;
            let dy = y-cy;
            if dx*dx+dy*dy > max_r2 {continue;}
            let chance = random(hx,hy,settings.seed ^ 0x1319_8a2e);
            if density((cx/n as f32).rem_euclid(1.0),
                       (cy/n as f32).rem_euclid(1.0)).clamp(0.0,1.0) <= chance {
                continue;
            }
            let theta = (random(hx,hy,settings.seed ^ 0x0370_7344)-0.5)
                * std::f32::consts::TAU * settings.rotation;
            let (s,c) = theta.sin_cos();
            let pu = (dx*c+dy*s)/settings.scale + 0.5;
            let pv = (-dx*s+dy*c)/settings.scale + 0.5;
            if !(0.0..=1.0).contains(&pu) || !(0.0..=1.0).contains(&pv) {
                continue;
            }
            let shade = 1.0 - settings.brightness *
                random(hx,hy,settings.seed ^ 0xa409_3822);
            coverage = coverage.max(pattern(pu,pv).clamp(0.0,1.0)*shade);
            if coverage >= 1.0 {return 1.0;}
        }
    }
    coverage
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    fn defaults() -> ScatterSettings {
        ScatterSettings { cells:8,scale:0.7,jitter:0.2,
            rotation:1.0,brightness:0.2,seed:42 }
    }

    #[test]
    fn seamless_and_deterministic_for_negative_and_wrapped_uv() {
        let settings=defaults();
        for (u,v) in [(0.0,0.12),(0.34,0.0),(-0.2,0.41),(0.93,0.99)] {
            let f=|x,y| sample(x,y,settings,
                |px,py| if (px-0.5).hypot(py-0.5)<0.35 {1.0} else {0.0},
                |_,_|1.0);
            assert_eq!(f(u,v),f(u+1.0,v),"U tile seam");
            assert_eq!(f(u,v),f(u,v+1.0),"V tile seam");
            assert_eq!(f(u,v),f(u,v),"stable seeded output");
        }
    }

    #[test]
    fn density_zero_suppresses_all_stamps_and_seed_changes_positions() {
        let settings=defaults();
        assert_eq!(sample(0.2,0.3,settings,|_,_|1.0,|_,_|0.0),0.0);
        let pixels=|s:ScatterSettings| (0..32).flat_map(|y|(0..32).map(move |x|
            sample((x as f32+0.5)/32.0,(y as f32+0.5)/32.0,s,
                |_,_|1.0,|_,_|1.0).to_bits())).collect::<Vec<_>>();
        let a=pixels(settings);
        assert!(a.iter().any(|b|*b!=0));
        assert!(a.iter().any(|b|*b==0));
        assert_ne!(a,pixels(ScatterSettings{seed:99,..settings}));
    }

    #[test]
    fn neighbor_work_and_parameter_validation_are_bounded() {
        let calls=Cell::new(0);
        let settings=ScatterSettings{cells:32,..defaults()};
        let value=sample(0.325,0.725,settings,|_,_|{
            calls.set(calls.get()+1);1.0
        },|_,_|1.0);
        assert!((0.0..=1.0).contains(&value));
        assert!(calls.get()<=9,"only a fixed neighborhood is evaluated");
        assert!(settings.validate().is_ok());
        for invalid in [
            ScatterSettings{cells:33,..settings},
            ScatterSettings{scale:f32::NAN,..settings},
            ScatterSettings{jitter:0.4,..settings},
            ScatterSettings{rotation:f32::INFINITY,..settings},
            ScatterSettings{brightness:-0.01,..settings}
        ] { assert!(invalid.validate().is_err()); }
    }
}
