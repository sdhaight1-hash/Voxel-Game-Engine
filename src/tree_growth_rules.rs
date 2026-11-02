//! Bounded architectural growth rules for the 29 authored tree/bamboo types.
//! These are stylized visual rules at the editor's 1m hexel resolution, not
//! measured botanical growth simulations. Author-edited slider values still
//! apply, but the seed cannot turn one species into a different silhouette.
use crate::foliage::PlantPreset;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phyllotaxy {
    Spiral,       // staggered / alternate shoots
    Opposite,     // paired branches rotated between pairs
    Whorled,      // concentric tiers (conifers)
    CulmNodes,    // bamboo branches at discrete culm nodes
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LeafPlacement {
    OuterCrown,      // terminal and near-terminal broadleaf sprays
    AlongTwigs,      // compound leaves along fine lateral shoots
    WeepingCurtain,  // descending willow foliage
    NeedleSprays,    // conifer needle masses along branches
    SpurNeedles,     // larch clusters on short shoots
    FeatherSprays,   // bald cypress feathery branchlets
    CulmLeaves,      // bamboo leaves concentrated at nodes / twigs
}

#[derive(Clone, Copy, Debug)]
pub struct TreeGrowthRules {
    pub phyllotaxy: Phyllotaxy,
    pub foliage: LeafPlacement,
    /// Vertical change / horizontal branch reach; negative angles droop.
    pub primary_pitch: [f32; 2],
    pub secondary_pitch: [f32; 2],
    pub reach_factor: f32,
    /// Chance to form one extra secondary twig on a primary limb.
    pub extra_twig_chance: f32,
    /// Seeded trunk deflection as a proportion of height.
    pub lean_fraction: f32,
    /// Scale of foliage clusters relative to the existing artist leaf size.
    pub foliage_factor: f32,
}

impl TreeGrowthRules {
    pub fn check(self) -> bool {
        self.primary_pitch[0] <= self.primary_pitch[1]
            && self.secondary_pitch[0] <= self.secondary_pitch[1]
            && self.reach_factor > 0.0 && self.reach_factor <= 1.5
            && (0.0..=1.0).contains(&self.extra_twig_chance)
            && (0.0..=0.08).contains(&self.lean_fraction)
            && (0.25..=1.5).contains(&self.foliage_factor)
    }
}

impl PlantPreset {
    pub fn growth_rules(self) -> TreeGrowthRules {
        use LeafPlacement::*;
        use Phyllotaxy::*;
        // Every row: branching pattern, leaf sites, major/secondary pitch
        // limits, lateral reach, probability of an extra twig, seed trunk lean,
        // and cluster size. These are game-art bounds, not botanical measurements.
        let (phyllotaxy, foliage, primary_pitch, secondary_pitch,
            reach_factor, extra_twig_chance, lean_fraction, foliage_factor) =
            match self {
                Self::Acacia => (Spiral, AlongTwigs, [-0.12,0.25],[-0.18,0.35],1.18,0.70,0.035,0.83),
                Self::Birch => (Spiral, OuterCrown, [0.25,0.85],[0.20,0.80],0.81,0.42,0.035,0.82),
                Self::Ebony => (Spiral, OuterCrown, [0.02,0.46],[-0.10,0.48],0.95,0.78,0.023,1.00),
                Self::Kapok => (Whorled, OuterCrown, [-0.10,0.34],[-0.18,0.34],1.20,0.53,0.018,1.09),
                Self::Maple => (Opposite, OuterCrown, [0.04,0.52],[0.02,0.58],1.03,0.73,0.025,1.00),
                Self::CrimsonKingMaple => (Opposite, OuterCrown, [0.04,0.52],[0.02,0.58],1.03,0.73,0.025,1.00),
                Self::Oak => (Spiral, OuterCrown, [-0.06,0.42],[-0.14,0.50],1.13,0.85,0.036,1.04),
                Self::Purpleheart => (Spiral, AlongTwigs, [0.00,0.48],[-0.10,0.53],1.04,0.64,0.025,0.94),
                Self::Walnut => (Spiral, AlongTwigs, [-0.08,0.43],[-0.16,0.48],1.10,0.78,0.039,1.03),
                Self::Willow => (Spiral, WeepingCurtain, [-1.10,-0.10],[-1.30,-0.36],1.15,0.93,0.032,0.78),
                Self::BaldCypress => (Spiral, FeatherSprays, [0.00,0.40],[-0.09,0.31],0.94,0.55,0.019,0.79),
                Self::MediterraneanCypress => (Whorled, NeedleSprays, [0.25,0.92],[0.18,0.81],0.58,0.22,0.009,0.66),
                Self::Larch => (Whorled, SpurNeedles, [-0.03,0.33],[-0.11,0.26],1.02,0.66,0.020,0.74),
                Self::Pine => (Whorled, NeedleSprays, [-0.03,0.32],[-0.09,0.30],1.02,0.47,0.022,0.81),
                Self::Redwood => (Whorled, NeedleSprays, [0.00,0.38],[-0.08,0.24],0.86,0.33,0.011,0.81),
                Self::RedApple|Self::PinkApple|Self::YellowApple =>
                    (Spiral, OuterCrown, [-0.04,0.45],[-0.10,0.50],1.00,0.62,0.031,0.94),
                Self::Pear => (Spiral, OuterCrown, [0.17,0.67],[0.10,0.70],0.85,0.51,0.026,0.88),
                Self::Olive => (Opposite, AlongTwigs, [-0.16,0.31],[-0.18,0.42],1.13,0.91,0.062,0.75),
                Self::Peach => (Spiral, AlongTwigs, [-0.02,0.50],[-0.08,0.55],1.05,0.72,0.030,0.87),
                Self::Cherry => (Spiral, OuterCrown, [0.05,0.62],[-0.03,0.56],1.01,0.62,0.024,0.97),
                Self::Mango => (Spiral, OuterCrown, [-0.06,0.42],[-0.08,0.50],1.09,0.86,0.025,1.10),
                Self::Orange => (Spiral, OuterCrown, [0.00,0.47],[-0.06,0.46],0.98,0.62,0.029,0.99),
                Self::Lychee => (Spiral, OuterCrown, [-0.02,0.46],[-0.08,0.48],1.04,0.69,0.027,0.93),
                Self::Breadfruit => (Spiral, AlongTwigs, [-0.08,0.47],[-0.09,0.54],1.07,0.70,0.029,1.08),
                Self::Pomegranate => (Opposite, AlongTwigs, [-0.04,0.55],[-0.10,0.62],1.12,0.90,0.044,0.78),
                Self::GreenBamboo|Self::BrownBamboo =>
                    (CulmNodes, CulmLeaves, [0.12,0.82],[0.04,0.64],0.62,0.15,0.007,0.55),
                Self::Shrub => (Spiral, OuterCrown, [-0.15,0.45],[-0.20,0.55],1.10,0.75,0.030,0.78),
            };
        TreeGrowthRules { phyllotaxy, foliage, primary_pitch, secondary_pitch,
            reach_factor, extra_twig_chance, lean_fraction, foliage_factor }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_preset_has_valid_seed_limits() {
        for species in PlantPreset::ALL {
            assert!(species.growth_rules().check(), "{species:?}");
        }
    }

    #[test]
    fn color_variants_share_one_growth_architecture() {
        let a=PlantPreset::RedApple.growth_rules();
        let b=PlantPreset::PinkApple.growth_rules();
        let c=PlantPreset::YellowApple.growth_rules();
        assert_eq!(a.phyllotaxy,b.phyllotaxy);
        assert_eq!(b.primary_pitch,c.primary_pitch);
        assert_eq!(a.extra_twig_chance,c.extra_twig_chance);
        let a=PlantPreset::GreenBamboo.growth_rules();
        let b=PlantPreset::BrownBamboo.growth_rules();
        assert_eq!(a.phyllotaxy,b.phyllotaxy);
        assert_eq!(a.primary_pitch,b.primary_pitch);
    }

    #[test]
    fn key_architectures_cannot_blend_into_each_other() {
        assert_eq!(PlantPreset::Willow.growth_rules().foliage,
            LeafPlacement::WeepingCurtain);
        assert!(PlantPreset::Willow.growth_rules().primary_pitch[1] < 0.0);
        assert_eq!(PlantPreset::Larch.growth_rules().foliage,
            LeafPlacement::SpurNeedles);
        assert_eq!(PlantPreset::GreenBamboo.growth_rules().phyllotaxy,
            Phyllotaxy::CulmNodes);
        assert_eq!(PlantPreset::Maple.growth_rules().phyllotaxy,
            Phyllotaxy::Opposite);
    }
}
