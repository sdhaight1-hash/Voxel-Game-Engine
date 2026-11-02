//! Shared flora prefab contract for every built-in tree, bamboo, coffee plant
//! and shrub. A prefab is reusable *source data*, not a placed world entity.
//!
//! FoliageProject remains the single authority for generation, geometry,
//! appearance, leaf-card shape and authored seasonal palettes. ClimateProfile
//! supplies the independent habitat envelope. No second tree generator exists.

use crate::foliage::{FoliageMesh, FoliageProject, PlantPreset};
use crate::plant_climate::{ClimateProfile, SiteClimate};
use serde::{Deserialize, Serialize};

pub const FLORA_PREFAB_VERSION: u32 = 1;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FloraPrefab {
    pub format_version: u32,
    /// Stable library ID. Built-ins use the serialized PlantPreset enum name;
    /// user-authored variants must choose their own distinct ID when published.
    pub asset_id: String,
    pub species: PlantPreset,
    /// Seeded geometry, wood/leaf hexels, materials, and seasonal palettes.
    pub source: FoliageProject,
    /// Preferred natural site envelope; can be authored independently.
    pub habitat: ClimateProfile,
}

impl FloraPrefab {
    pub fn builtin_id(species: PlantPreset) -> String {
        format!("flora/{species:?}")
    }

    pub fn from_preset(species: PlantPreset) -> Self {
        Self {
            format_version: FLORA_PREFAB_VERSION,
            asset_id: Self::builtin_id(species),
            species,
            source: FoliageProject::from_preset(species),
            habitat: species.climate_profile(),
        }
    }

    /// Wrap an existing edited Tree Designer project without regenerating it
    /// or discarding its seeds, morphology overrides, palettes or materials.
    pub fn from_project(
        asset_id: impl Into<String>, source: FoliageProject,
    ) -> Result<Self, String> {
        let species=source.preset;
        let prefab=Self {
            format_version: FLORA_PREFAB_VERSION,
            asset_id: asset_id.into(),
            species,
            source,
            habitat:species.climate_profile(),
        };
        prefab.validate()?;
        Ok(prefab)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.format_version!=FLORA_PREFAB_VERSION {
            return Err("Unsupported Flora prefab format version".into());
        }
        if self.asset_id.is_empty()
            || !self.asset_id.bytes().all(|c|
                c.is_ascii_alphanumeric() || matches!(c,b'/'|b'-'|b'_'))
            || self.asset_id.starts_with('/')
            || self.asset_id.ends_with('/') {
            return Err("Flora prefab ID must use nonempty ASCII letters, digits, /, - or _".into());
        }
        if self.species!=self.source.preset {
            return Err("Flora prefab species must match its FoliageProject source".into());
        }
        if !self.habitat.is_valid() {
            return Err("Flora prefab contains invalid climate preferences".into());
        }
        self.source.validate()
    }

    /// Build a local-space, editable hexel tree for one individual.
    /// Does not mutate the shared source or place it into a world chunk.
    pub fn generate_with_seed(&self, seed: u32) -> Result<FoliageMesh, String> {
        self.validate()?;
        let mut individual=self.source.clone();
        individual.seed=seed;
        individual.generate()
    }

    pub fn suitability(&self, site: SiteClimate) -> f32 {
        self.habitat.suitability(site)
    }

    pub fn to_pretty_json(&self) -> Result<String, String> {
        self.validate()?;
        serde_json::to_string_pretty(self).map_err(|e|e.to_string())
    }

    pub fn from_json(text: &str) -> Result<Self, String> {
        let prefab: Self=serde_json::from_str(text).map_err(|e|e.to_string())?;
        prefab.validate()?;
        Ok(prefab)
    }
}

/// Minimal declaration of a placed individual (q/r horizontal, y vertical).
/// This is a reference only; terrain placement, loaded chunks, player edits,
/// growth, harvesting, persistence and coordinate conversion are world systems.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FloraPlacement {
    pub prefab_id: String,
    pub seed: u32,
    pub q: i32,
    pub r: i32,
    pub y: i32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FloraCatalog {
    pub schema: String,
    pub prefabs: Vec<FloraPrefab>,
}

impl FloraCatalog {
    pub fn built_in() -> Self {
        Self {
            schema:"hexel-flora-prefab-catalog-v1".into(),
            prefabs:PlantPreset::ALL.iter().copied()
                .map(FloraPrefab::from_preset).collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::foliage_materials::SeasonalPalettes;

    #[test]
    fn every_existing_preset_derives_from_same_flora_definition() {
        let catalog=FloraCatalog::built_in();
        assert_eq!(catalog.prefabs.len(),43);
        assert_eq!(catalog.schema,"hexel-flora-prefab-catalog-v1");
        let ids:std::collections::HashSet<_>=catalog.prefabs.iter()
            .map(|p|p.asset_id.as_str()).collect();
        assert_eq!(ids.len(),43);
        for (species,prefab) in PlantPreset::ALL.iter().zip(&catalog.prefabs) {
            assert_eq!(prefab.species,*species);
            assert_eq!(prefab.source.preset,*species);
            assert_eq!(prefab.asset_id,FloraPrefab::builtin_id(*species));
            assert!(prefab.validate().is_ok(),"{species:?}");
        }
    }

    #[test]
    fn derived_prefabs_preserve_custom_tree_materials_and_growth() {
        let mut source=FoliageProject::from_preset(PlantPreset::Oak);
        source.seed=187;
        source.leaf_color=[65,121,55];
        let mut palette=SeasonalPalettes::for_species(source.preset,source.leaf_color);
        palette.seasons[2][3]=[240,72,21];
        source.materials.seasonal_palettes=Some(palette);
        let mut rules=source.effective_growth_rules();
        rules.reach_factor*=1.05;
        source.set_growth_rules(rules).unwrap();
        let prefab=FloraPrefab::from_project("flora/custom-oak",source.clone()).unwrap();
        let json=prefab.to_pretty_json().unwrap();
        let restored=FloraPrefab::from_json(&json).unwrap();
        assert_eq!(restored.asset_id,"flora/custom-oak");
        assert_eq!(
            serde_json::to_value(&restored.source).unwrap(),
            serde_json::to_value(&source).unwrap()
        );
        assert_eq!(restored.habitat,PlantPreset::Oak.climate_profile());
        assert_eq!(restored.generate_with_seed(187).unwrap().voxels,
            source.generate().unwrap().voxels);
    }

    #[test]
    fn per_individual_seed_does_not_modify_parent_prefab() {
        for preset in [PlantPreset::CaturraCoffee,PlantPreset::FanPalm,
                       PlantPreset::FremontCottonwood,PlantPreset::Shrub] {
            let prefab=FloraPrefab::from_preset(preset);
            let a=prefab.generate_with_seed(42).unwrap();
            let b=prefab.generate_with_seed(42).unwrap();
            let c=prefab.generate_with_seed(43).unwrap();
            assert_eq!(a.voxels,b.voxels);
            assert_ne!(a.structure.as_ref().unwrap().spans,
                c.structure.as_ref().unwrap().spans);
            assert_eq!(prefab.source.seed,42);
        }
    }

    #[test]
    fn rejects_invalid_prefab_identity_species_and_habitat() {
        let mut prefab=FloraPrefab::from_preset(PlantPreset::Oak);
        prefab.species=PlantPreset::Pine;
        assert!(prefab.validate().is_err());
        prefab.species=PlantPreset::Oak;
        prefab.format_version=99;
        assert!(prefab.validate().is_err());
        prefab.format_version=FLORA_PREFAB_VERSION;
        prefab.asset_id="../unknown".into();
        assert!(prefab.validate().is_err());
        prefab.asset_id="flora/oak".into();
        prefab.habitat.mean_annual_temp_c=[19.0,2.0];
        assert!(prefab.validate().is_err());
    }

    #[test]
    fn placement_references_prefab_without_embedding_geometry() {
        let placement=FloraPlacement {
            prefab_id:FloraPrefab::builtin_id(PlantPreset::DatePalm),
            seed:9001,q:-12,r:8,y:214,
        };
        let restored:FloraPlacement=serde_json::from_str(
            &serde_json::to_string(&placement).unwrap()).unwrap();
        assert_eq!(placement,restored);
    }
}
