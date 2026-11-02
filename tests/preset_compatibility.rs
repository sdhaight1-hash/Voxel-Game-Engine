//! Contract tests tying *all* built-in material recipes to *all* Flora
//! presets, without replacing serialized, user-authored graph snapshots.
use texture_studio::{VoxelSettings,Graph};
use texture_studio::presets::builtin_presets;
use texture_studio::foliage::{FoliageProject,PlantPreset};
use texture_studio::foliage_materials::{
    FoliageMaterials,LeafAlphaSampling,MaterialRole,
};
use texture_studio::leaf_vector::LeafSilhouetteSource;
use texture_studio::flora_prefab::FloraPrefab;
use texture_studio::asset::VoxelAsset;

#[test]
fn all_43_tree_presets_have_species_owned_bark_and_leaf_material_graphs() {
    let catalog=builtin_presets().unwrap();
    assert_eq!(catalog.len(),1067);
    assert_eq!(PlantPreset::ALL.len(),43);
    for species in PlantPreset::ALL {
        let mats=FoliageMaterials::for_species(species);
        mats.validate().unwrap();
        assert_eq!(mats.leaf_species,species);
        assert_eq!(mats.leaf_silhouette,LeafSilhouetteSource::LegacySpecies);
        assert_eq!(mats.leaf_alpha_sampling,LeafAlphaSampling::Binary);
        for role in [MaterialRole::Bark,MaterialRole::Leaf] {
            let title=match role {
                MaterialRole::Bark=>mats.bark_preset.as_ref(),
                MaterialRole::Leaf=>mats.leaf_preset.as_ref(),
                MaterialRole::Opacity=>unreachable!(),
            }.unwrap_or_else(||panic!("{species:?} missing {role:?} catalog reference"));
            let preset=catalog.iter().find(|p|
                p.material==species.material_name() && p.title()==*title)
                .unwrap_or_else(||panic!(
                    "{species:?} {role:?} points outside its own catalog material: {title}"
                ));
            let graph=mats.graph(role).unwrap_or_else(||panic!(
                "{species:?} {role:?} has no authored graph"));
            graph.validate().unwrap();
            let expected=preset.build_graph(VoxelSettings{
                geometryless:true,..Default::default()
            });
            assert_eq!(serde_json::to_value(graph).unwrap(),
                serde_json::to_value(&expected).unwrap(),
                "{species:?} {role:?} must embed the selected stable material graph");
        }
        let tree=FoliageProject::from_preset(species);
        tree.validate().unwrap();
        assert_eq!(tree.materials.bark_preset,mats.bark_preset);
        assert_eq!(tree.materials.leaf_preset,mats.leaf_preset);
        let tree_json=serde_json::to_string(&tree).unwrap();
        let restored:FoliageProject=serde_json::from_str(&tree_json).unwrap();
        restored.validate().unwrap();
        assert_eq!(serde_json::to_value(&restored.materials).unwrap(),
            serde_json::to_value(&tree.materials).unwrap());
        let prefab=FloraPrefab::from_preset(species);
        prefab.validate().unwrap();
        assert_eq!(prefab.source.materials.bark_preset,mats.bark_preset);
        assert_eq!(prefab.source.materials.leaf_preset,mats.leaf_preset);
        let loaded=FloraPrefab::from_json(&prefab.to_pretty_json().unwrap()).unwrap();
        assert_eq!(loaded.species,species);
        assert_eq!(loaded.source.materials.leaf_species,species);
    }
}

#[test]
fn new_species_material_recipes_are_distinct_from_legacy_fallback_species() {
    let fixtures=[
        (PlantPreset::CherryBlossom,PlantPreset::Cherry),
        (PlantPreset::KentuckyCoffeetree,PlantPreset::Walnut),
        (PlantPreset::EasternCottonwood,PlantPreset::Birch),
        (PlantPreset::FremontCottonwood,PlantPreset::Birch),
        (PlantPreset::BlackCottonwood,PlantPreset::Birch),
        (PlantPreset::ArabicaCoffee,PlantPreset::Orange),
        (PlantPreset::CaturraCoffee,PlantPreset::Orange),
        (PlantPreset::MaragogipeCoffee,PlantPreset::Orange),
        (PlantPreset::CastilloCoffee,PlantPreset::Orange),
        (PlantPreset::TabiCoffee,PlantPreset::Orange),
        (PlantPreset::CoconutPalm,PlantPreset::Mango),
        (PlantPreset::DatePalm,PlantPreset::Mango),
        (PlantPreset::FanPalm,PlantPreset::Mango),
        (PlantPreset::Shrub,PlantPreset::Oak),
    ];
    for (species,old_fallback) in fixtures {
        let new=FoliageMaterials::for_species(species);
        let old=FoliageMaterials::for_species(old_fallback);
        assert_ne!(species.material_name(),old_fallback.material_name());
        assert_ne!(new.bark_preset,old.bark_preset);
        assert_ne!(new.leaf_preset,old.leaf_preset);
        assert!(new.bark.is_some()&&new.leaf.is_some());
    }
}

#[test]
fn vector_tree_presets_survive_prefab_to_editable_asset_roundtrip() {
    for (species,shape) in [
        (PlantPreset::Oak,LeafSilhouetteSource::VectorOak{lobe_width:1.0}),
        (PlantPreset::Willow,LeafSilhouetteSource::VectorWillow{blade_width:1.0}),
        (PlantPreset::EasternCottonwood,
            LeafSilhouetteSource::VectorCottonwood{blade_width:1.0}),
        (PlantPreset::FremontCottonwood,
            LeafSilhouetteSource::VectorCottonwood{blade_width:1.0}),
        (PlantPreset::BlackCottonwood,
            LeafSilhouetteSource::VectorCottonwood{blade_width:1.0}),
    ] {
        let mut tree=FoliageProject::from_preset(species);
        tree.materials.leaf_silhouette=shape;
        tree.materials.leaf_alpha_sampling=LeafAlphaSampling::Coverage2x2;
        tree.validate().unwrap();
        let expected=tree.materials.bake_leaf(64,tree.leaf_color).unwrap();
        assert!(expected.chunks_exact(4).any(|p|p[3]>0&&p[3]<255),
            "{species:?} lost subpixel vector silhouette");
        let prefab=FloraPrefab::from_project(
            format!("flora/vector-{species:?}"),tree.clone()).unwrap();
        let restored=FloraPrefab::from_json(&prefab.to_pretty_json().unwrap()).unwrap();
        assert_eq!(restored.source.materials.bake_leaf(64,tree.leaf_color).unwrap(),
            expected,"{species:?} failed prefab roundtrip");
        let mesh=restored.source.generate().unwrap();
        let asset=VoxelAsset::from_tree(&restored.source,&mesh);
        asset.validate().unwrap();
        let json=asset.to_json().unwrap();
        let loaded=VoxelAsset::from_json(&json).unwrap();
        assert_eq!(loaded.materials.leaf_species,species);
        assert_eq!(loaded.materials.leaf_silhouette,shape);
        assert_eq!(loaded.materials.leaf_alpha_sampling,LeafAlphaSampling::Coverage2x2);
        assert_eq!(loaded.materials.bake_leaf(64,loaded.leaf_color).unwrap(),
            expected,"{species:?} lost vector alpha through editable asset");
        // Preview-only ordered/MSAA/mip flags must never enter saved tree assets.
        for forbidden in ["leaf_aa_mode","leaf_mipmaps","ordered_leaf_coverage"] {
            assert!(!json.contains(forbidden));
        }
    }
}

#[test]
fn formerly_saved_material_graphs_are_authoritative_not_remapped() {
    let old_graph:Graph=serde_json::from_str(
        include_str!("fixtures/legacy_graph_v0.json")).unwrap();
    let mut old=FoliageProject::from_preset(PlantPreset::BlackCottonwood);
    old.materials.bark=Some(old_graph.clone());
    old.materials.leaf=Some(old_graph.clone());
    // These titles are legitimate provenance from the earlier fallback era.
    old.materials.bark_preset=Some("Birch · Bark / trunk sides".into());
    old.materials.leaf_preset=Some("Birch · Leaf-only canopy".into());
    let saved=serde_json::to_string(&old).unwrap();
    let loaded:FoliageProject=serde_json::from_str(&saved).unwrap();
    loaded.validate().unwrap();
    assert_eq!(loaded.materials.bark_preset,old.materials.bark_preset);
    assert_eq!(loaded.materials.leaf_preset,old.materials.leaf_preset);
    assert_eq!(loaded.materials.bark.as_ref().unwrap().render_rgba(16,16).unwrap(),
        old_graph.render_rgba(16,16).unwrap());
    assert_eq!(loaded.materials.leaf.as_ref().unwrap().render_rgba(16,16).unwrap(),
        old_graph.render_rgba(16,16).unwrap());
}
