//! Approximate biome-placement envelopes for the 43 foliage presets.
//!
//! Climate is defined by the location, not by a tree's RGB/seasonal material.
//! These are *preferred site envelopes* for game-world procedural distribution,
//! not proven universal survival limits or plant-hardiness certification.
//! Source/rationale and every row are documented in
//! docs/vegetation-climate-profiles.md.

use crate::foliage::PlantPreset;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SoilMoisture { Dry, Moderate, Moist, Wet }

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LightExposure { Sun, PartialShade, Shade }

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ClimateProfile {
    /// Preferred mean annual air temperature, in degrees Celsius.
    pub mean_annual_temp_c: [f32; 2],
    /// Preferred annual *atmospheric* rainfall, millimetres per year.
    pub annual_precip_mm: [u16; 2],
    /// Indicative lower temperature of the coldest brief annual event (°C).
    /// Art-directed biome gate; not a guarantee of plant survival.
    pub extreme_min_c: f32,
    /// Preferred count of months with less than 30 mm precipitation.
    pub dry_months_per_year: [u8; 2],
    /// Preferred local root-zone moisture, separate from annual rainfall.
    pub soil_moisture: SoilMoisture,
    pub light: LightExposure,
    /// Riparian cottonwoods/willows and desert date palms need dependable
    /// stream/alluvial/groundwater (or irrigation) even if rainfall is low.
    pub requires_groundwater: bool,
}

const fn profile(
    temp: [f32; 2], rain: [u16; 2], minimum: f32, dry: [u8; 2],
    soil: SoilMoisture, light: LightExposure, groundwater: bool,
) -> ClimateProfile {
    ClimateProfile {
        mean_annual_temp_c:temp, annual_precip_mm:rain,
        extreme_min_c:minimum, dry_months_per_year:dry,
        soil_moisture:soil, light, requires_groundwater:groundwater,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct SiteClimate {
    pub mean_annual_temp_c: f32,
    pub annual_precip_mm: f32,
    /// Lowest typical annual short-lived temperature (NOT monthly average).
    pub extreme_min_c: f32,
    /// Count of months with precipitation < 30 mm, 0..=12.
    pub dry_months_per_year: u8,
    pub soil_moisture: SoilMoisture,
    pub light: LightExposure,
    pub groundwater_access: bool,
}

impl ClimateProfile {
    /// Simple, deterministic *placement weight* in 0..=1.
    /// Not a physical growth, productivity, health or survival simulator.
    /// World runtime can combine this with slope, soil, competition and density.
    pub fn suitability(self, site: SiteClimate) -> f32 {
        if !site.mean_annual_temp_c.is_finite() || !site.annual_precip_mm.is_finite()
            || !site.extreme_min_c.is_finite() || site.annual_precip_mm < 0.0
            || site.dry_months_per_year > 12
            || site.extreme_min_c < self.extreme_min_c
            || (self.requires_groundwater && !site.groundwater_access) {
            return 0.0;
        }
        let fit=|x:f32,low:f32,high:f32,falloff:f32| {
            if x<low {(1.0-(low-x)/falloff).clamp(0.0,1.0)}
            else if x>high {(1.0-(x-high)/falloff).clamp(0.0,1.0)}
            else {1.0_f32}
        };
        let temp=fit(site.mean_annual_temp_c,self.mean_annual_temp_c[0],
            self.mean_annual_temp_c[1],5.0);
        let rain=fit(site.annual_precip_mm,self.annual_precip_mm[0] as f32,
            self.annual_precip_mm[1] as f32,
            (self.annual_precip_mm[1]-self.annual_precip_mm[0]) as f32*0.4+250.0);
        let dry=fit(site.dry_months_per_year as f32,
            self.dry_months_per_year[0] as f32,
            self.dry_months_per_year[1] as f32,2.0);
        let moisture=match self.soil_moisture {
            SoilMoisture::Dry=>0_i32,SoilMoisture::Moderate=>1,
            SoilMoisture::Moist=>2,SoilMoisture::Wet=>3,
        };
        let site_moisture=match site.soil_moisture {
            SoilMoisture::Dry=>0_i32,SoilMoisture::Moderate=>1,
            SoilMoisture::Moist=>2,SoilMoisture::Wet=>3,
        };
        let wetness=match (moisture-site_moisture).abs() {
            0=>1.0,1=>0.65,2=>0.25,_=>0.0,
        };
        let light=if self.light==site.light {1.0} else if
            self.light==LightExposure::PartialShade ||
            site.light==LightExposure::PartialShade {0.75}
        else {0.35};
        temp*rain*dry*wetness*light
    }

    pub fn is_valid(self)->bool {
        self.mean_annual_temp_c.iter().all(|v|v.is_finite())
            && self.mean_annual_temp_c[0]<=self.mean_annual_temp_c[1]
            && self.extreme_min_c.is_finite()
            && self.extreme_min_c<=self.mean_annual_temp_c[0]
            && self.annual_precip_mm[0]<=self.annual_precip_mm[1]
            && self.dry_months_per_year[0]<=self.dry_months_per_year[1]
            && self.dry_months_per_year[1]<=12
    }
}

impl PlantPreset {
    pub fn climate_profile(self)->ClimateProfile {
        use PlantPreset::*;
        use SoilMoisture::*;
        use LightExposure::*;
        match self {
            Acacia => profile([19.0, 30.0], [250, 1100], -3.0, [3, 7], Dry, Sun, false),
            Birch => profile([0.0, 12.0], [450, 1500], -40.0, [0, 4], Moist, Sun, false),
            Ebony => profile([22.0, 29.0], [1200, 3000], 8.0, [0, 3], Moist, PartialShade, false),
            Kapok => profile([23.0, 29.0], [1200, 3000], 10.0, [1, 4], Moist, Sun, false),
            Maple => profile([7.0, 17.0], [650, 1600], -30.0, [0, 3], Moist, PartialShade, false),
            CrimsonKingMaple => profile([7.0, 17.0], [650, 1600], -30.0, [0, 3], Moist, Sun, false),
            Oak => profile([7.0, 19.0], [500, 1400], -25.0, [1, 4], Moderate, Sun, false),
            Purpleheart => profile([24.0, 29.0], [1700, 3500], 13.0, [0, 2], Wet, PartialShade, false),
            Walnut => profile([8.0, 17.0], [500, 1400], -28.0, [1, 3], Moderate, Sun, false),
            Willow => profile([5.0, 20.0], [600, 1800], -30.0, [0, 4], Wet, Sun, true),
            CherryBlossom => profile([9.0, 17.0], [750, 1700], -20.0, [0, 2], Moist, Sun, false),
            KentuckyCoffeetree => profile([8.0, 18.0], [600, 1300], -32.0, [1, 3], Moderate, Sun, false),
            EasternCottonwood => profile([5.0, 19.0], [300, 1500], -38.0, [0, 5], Moist, Sun, true),
            FremontCottonwood => profile([12.0, 24.0], [100, 800], -18.0, [2, 7], Moist, Sun, true),
            BlackCottonwood => profile([4.0, 17.0], [200, 2500], -32.0, [0, 4], Moist, Sun, true),
            BaldCypress => profile([13.0, 24.0], [900, 1800], -25.0, [0, 4], Wet, Sun, false),
            MediterraneanCypress => profile([12.0, 23.0], [300, 1000], -18.0, [2, 6], Dry, Sun, false),
            Larch => profile([-2.0, 12.0], [400, 1500], -45.0, [0, 4], Moderate, Sun, false),
            Pine => profile([1.0, 20.0], [350, 1700], -35.0, [1, 5], Moderate, Sun, false),
            Redwood => profile([10.0, 17.0], [850, 2600], -10.0, [1, 4], Moist, PartialShade, false),
            RedApple => profile([6.0, 15.0], [600, 1350], -30.0, [1, 3], Moderate, Sun, false),
            PinkApple => profile([6.0, 15.0], [600, 1350], -30.0, [1, 3], Moderate, Sun, false),
            YellowApple => profile([6.0, 15.0], [600, 1350], -30.0, [1, 3], Moderate, Sun, false),
            Pear => profile([7.0, 17.0], [600, 1300], -27.0, [1, 3], Moderate, Sun, false),
            Olive => profile([13.0, 23.0], [300, 900], -8.0, [3, 6], Dry, Sun, false),
            Peach => profile([10.0, 19.0], [500, 1400], -22.0, [1, 4], Moderate, Sun, false),
            Cherry => profile([7.0, 16.0], [600, 1300], -28.0, [1, 3], Moderate, Sun, false),
            Mango => profile([22.0, 30.0], [750, 2400], 5.0, [2, 5], Moderate, Sun, false),
            Orange => profile([16.0, 27.0], [700, 1800], -4.0, [0, 3], Moderate, Sun, false),
            Lychee => profile([18.0, 28.0], [1000, 2200], 0.0, [0, 3], Moist, Sun, false),
            Breadfruit => profile([23.0, 31.0], [1500, 3000], 10.0, [0, 2], Moist, Sun, false),
            Pomegranate => profile([14.0, 28.0], [300, 1000], -12.0, [3, 7], Dry, Sun, false),
            ArabicaCoffee => profile([18.0, 23.0], [1200, 2200], 2.0, [1, 3], Moist, PartialShade, false),
            CaturraCoffee => profile([18.0, 23.0], [1200, 2200], 2.0, [1, 3], Moist, PartialShade, false),
            MaragogipeCoffee => profile([18.0, 23.0], [1200, 2200], 2.0, [1, 3], Moist, PartialShade, false),
            CastilloCoffee => profile([18.0, 24.0], [1200, 2200], 2.0, [1, 3], Moist, PartialShade, false),
            TabiCoffee => profile([17.0, 23.0], [1200, 2200], 2.0, [1, 3], Moist, PartialShade, false),
            GreenBamboo => profile([12.0, 27.0], [900, 2300], -10.0, [0, 4], Moist, PartialShade, false),
            BrownBamboo => profile([12.0, 27.0], [900, 2300], -10.0, [0, 4], Moist, PartialShade, false),
            CoconutPalm => profile([24.0, 30.0], [1200, 2400], 10.0, [0, 3], Moist, Sun, false),
            DatePalm => profile([19.0, 31.0], [100, 400], -8.0, [6, 10], Moderate, Sun, true),
            FanPalm => profile([14.0, 28.0], [300, 1300], -8.0, [2, 5], Moderate, Sun, false),
            Shrub => profile([5.0, 27.0], [300, 2000], -25.0, [0, 6], Moderate, PartialShade, false),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlantClimateRecord {
    pub species: PlantPreset,
    pub climate: ClimateProfile,
}

/// Portable, versioned data for a future asset-library -> world-runtime handoff.
/// Does NOT itself plant foliage or mutate a world.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ClimateCatalog {
    pub schema: String,
    pub plants: Vec<PlantClimateRecord>,
}

impl ClimateCatalog {
    pub fn built_in()->Self {
        Self {
            schema:"hexel-vegetation-climate-v1".into(),
            plants:PlantPreset::ALL.iter().copied().map(|species|
                PlantClimateRecord {species,climate:species.climate_profile()}).collect(),
        }
    }
    pub fn to_pretty_json(&self)->Result<String,serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_43_profiles_are_valid_and_catalog_json_roundtrips() {
        let catalog=ClimateCatalog::built_in();
        assert_eq!(PlantPreset::TREE_CATALOG.len(),42);
        assert_eq!(catalog.plants.len(),43);
        for preset in PlantPreset::ALL {
            let profile=preset.climate_profile();
            assert!(profile.is_valid(),"{preset:?} invalid climate envelope");
            assert!(catalog.plants.iter().any(|r|r.species==preset));
        }
        let json=catalog.to_pretty_json().unwrap();
        let restored:ClimateCatalog=serde_json::from_str(&json).unwrap();
        assert_eq!(restored,catalog);
        assert_eq!(restored.schema,"hexel-vegetation-climate-v1");
    }

    #[test]
    fn coffee_is_tropical_and_frost_sensitive() {
        let coffee=PlantPreset::ArabicaCoffee.climate_profile();
        let good=SiteClimate {
            mean_annual_temp_c:20.0,annual_precip_mm:1600.0,
            extreme_min_c:6.0,dry_months_per_year:2,
            soil_moisture:SoilMoisture::Moist,light:LightExposure::PartialShade,
            groundwater_access:false,
        };
        assert_eq!(coffee.suitability(good),1.0);
        assert_eq!(coffee.suitability(SiteClimate{extreme_min_c:-1.0,..good}),0.0);
        assert_eq!(coffee.suitability(SiteClimate{
            mean_annual_temp_c:2.0,..good
        }),0.0);
        assert!(coffee.suitability(SiteClimate{
            annual_precip_mm:900.0,..good
        })<coffee.suitability(good));
    }

    #[test]
    fn riparian_and_oasis_species_need_water_access() {
        for species in [PlantPreset::FremontCottonwood,PlantPreset::BlackCottonwood,
                        PlantPreset::EasternCottonwood,PlantPreset::Willow,
                        PlantPreset::DatePalm] {
            assert!(species.climate_profile().requires_groundwater);
        }
        let site=SiteClimate {
            mean_annual_temp_c:22.0,annual_precip_mm:230.0,
            extreme_min_c:-2.0,dry_months_per_year:7,
            soil_moisture:SoilMoisture::Moist,light:LightExposure::Sun,
            groundwater_access:false,
        };
        let cottonwood=PlantPreset::FremontCottonwood.climate_profile();
        assert_eq!(cottonwood.suitability(site),0.0);
        assert!(cottonwood.suitability(SiteClimate{
            groundwater_access:true,..site
        })>0.0);
        let date=PlantPreset::DatePalm.climate_profile();
        assert_eq!(date.suitability(site),0.0);
        assert!(date.suitability(SiteClimate{
            groundwater_access:true,soil_moisture:SoilMoisture::Moderate,..site
        })>0.0);
    }

    #[test]
    fn soft_boundaries_do_not_claim_sharp_biological_cutoffs() {
        let oak=PlantPreset::Oak.climate_profile();
        let good=SiteClimate {
            mean_annual_temp_c:12.0,annual_precip_mm:900.0,
            extreme_min_c:-9.0,dry_months_per_year:3,
            soil_moisture:SoilMoisture::Moderate,light:LightExposure::Sun,
            groundwater_access:false,
        };
        assert_eq!(oak.suitability(good),1.0);
        let marginal=SiteClimate {mean_annual_temp_c:22.0,..good};
        assert!(oak.suitability(marginal)>0.0);
        assert!(oak.suitability(marginal)<1.0);
        assert_eq!(oak.suitability(SiteClimate{
            mean_annual_temp_c:25.0,..good
        }),0.0);
        assert_eq!(oak.suitability(SiteClimate{
            mean_annual_temp_c:f32::NAN,..good
        }),0.0);
    }
}
