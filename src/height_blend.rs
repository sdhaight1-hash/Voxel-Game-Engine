//! One reusable height-conditioned layer weight for all material channels.
//! Legacy graphs are unchanged: only new graph nodes call this function.

/// Blend mask in [0,1]. Relative height biases the authored mask, but
/// equal-height layers and zero influence preserve it exactly. The bias
/// vanishes at both mask endpoints, preserving hard material boundaries.
///
/// The height fields must be normalized [0,1]. This is a controlled
/// artistic height transition, not geometric occlusion or displacement.
pub fn weight(height_a:f32,height_b:f32,mask:f32,influence:f32)->f32 {
    let m=mask.clamp(0.0,1.0);
    let delta=height_b.clamp(0.0,1.0)-height_a.clamp(0.0,1.0);
    let envelope=4.0*m*(1.0-m);
    (m+delta*influence*envelope).clamp(0.0,1.0)
}

/// Linear channel mix. The same weight must be connected to each PBR
/// channel to keep albedo/roughness/metallic/height/opacity aligned.
pub fn mix(a:[f32;3],b:[f32;3],mask:f32)->[f32;3] {
    let t=mask.clamp(0.0,1.0);
    std::array::from_fn(|i|a[i]*(1.0-t)+b[i]*t)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn endpoints_flat_heights_and_disabled_bias_preserve_mask() {
        for m in [0.0,0.1,0.25,0.5,0.75,0.9,1.0] {
            assert_eq!(weight(0.3,0.3,m,1.0),m);
            assert_eq!(weight(0.1,0.9,m,0.0),m);
        }
        assert_eq!(weight(0.0,1.0,0.0,1.0),0.0);
        assert_eq!(weight(1.0,0.0,1.0,1.0),1.0);
    }
    #[test]
    fn higher_material_height_wins_at_transition_not_endpoints() {
        let base=0.5;
        assert!(weight(0.1,0.9,base,0.4)>base);
        assert!(weight(0.9,0.1,base,0.4)<base);
        assert_eq!(weight(0.1,0.9,base,1.0),1.0);
        assert_eq!(weight(0.9,0.1,base,1.0),0.0);
        assert!((0.0..=1.0).contains(&weight(-0.4,2.0,0.75,1.0)));
    }
    #[test]
    fn channel_mix_is_exact_at_endpoints() {
        let a=[0.25,0.4,0.9];let b=[0.75,0.2,0.1];
        assert_eq!(mix(a,b,0.0),a);
        assert_eq!(mix(a,b,1.0),b);
        let middle=mix(a,b,0.5);
        for k in 0..3 { assert!((middle[k]-(a[k]+b[k])*0.5).abs()<1e-6); }
    }
}
