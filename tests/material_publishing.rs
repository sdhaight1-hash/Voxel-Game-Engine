use std::{collections::HashMap,sync::Arc};
use texture_studio::{Graph};
use texture_studio::foliage::{FoliageProject,PlantPreset};
use texture_studio::foliage_materials::{LeafAlphaSampling};
use texture_studio::leaf_vector::LeafSilhouetteSource;
use texture_studio::asset::VoxelAsset;
use texture_studio::flora_prefab::FloraPrefab;
use texture_studio::foliage_bake_cache::FoliageBakeCache;
use texture_studio::render_cache::GraphRenderCache;
use texture_studio::material_publish::{
    RuntimeMaterialRecipe,StaticAppearanceSnapshot,
    bake_leaf_season,publish,RECIPE_VERSION,SNAPSHOT_VERSION,
};

fn caches()->(FoliageBakeCache,GraphRenderCache) {
    (FoliageBakeCache::new(16*1024*1024),
     GraphRenderCache::new(16*1024*1024))
}

#[test]
fn oak_nodes_flora_tree_asset_and_static_maps_are_one_material_recipe() {
    let mut tree=FoliageProject::from_preset(PlantPreset::Oak);
    tree.materials.bark_meters_per_repeat=Some(0.55);
    let prefab=FloraPrefab::from_project("flora/test-oak",tree.clone()).unwrap();
    let original=tree.generate().unwrap();
    let asset=VoxelAsset::from_tree(&tree,&original);
    let restored=VoxelAsset::from_json(&asset.to_json().unwrap()).unwrap();

    let recipe=RuntimeMaterialRecipe::from_tree(&tree).unwrap();
    let from_flora=RuntimeMaterialRecipe::from_flora(&prefab).unwrap();
    let from_asset=RuntimeMaterialRecipe::from_asset(&restored).unwrap();
    assert_eq!(recipe.recipe_id,from_flora.recipe_id);
    assert_eq!(recipe.recipe_id,from_asset.recipe_id);
    assert_eq!(from_flora.recipe_id,
        RuntimeMaterialRecipe::from_json(&recipe.to_json().unwrap()).unwrap().recipe_id);
    assert_eq!(recipe.materials.bark_meters_per_repeat,Some(0.55));
    assert_eq!(serde_json::to_value(&from_asset.materials).unwrap(),
        serde_json::to_value(&tree.materials).unwrap());
    assert_eq!(restored.voxels,original.voxels);

    let size=32;
    let (mut f,mut g)=caches();
    let published=publish(&recipe,size,2.25,&mut f,&mut g).unwrap();
    let other=publish(&from_asset,size,2.25,&mut f,&mut g).unwrap();
    assert_eq!(published.snapshot.snapshot_id,other.snapshot.snapshot_id);
    assert_eq!(published.snapshot.textures.len(),other.snapshot.textures.len());
    assert_eq!(published.pixels,other.pixels);
    assert!(f.stats().hits>=2,"Tree and Asset must reuse base bark/leaf bakes");
    assert!(g.stats().hits>=5,"Tree and Asset must reuse authored PBR graph output maps");

    // Nodes graph is the Bark role's source of truth, with the legacy
    // default bark tint_strength=0 preserving exact source pixels.
    let bark_from_nodes=tree.materials.bark.as_ref().unwrap()
        .render_rgba(size,size).unwrap();
    assert_eq!(published.pixel("bark-albedo.png").unwrap(),bark_from_nodes);
    assert_eq!(published.pixel("foliage-leaves.png").unwrap(),
        tree.materials.bake_leaf(size,tree.leaf_color).unwrap());
    let pbr:HashMap<_,_>=tree.materials.bake_pbr(size).unwrap()
        .into_iter().collect();
    for (filename,rgba) in pbr {
        assert_eq!(published.pixel(&filename).unwrap(),rgba,
            "{filename} did not match authored PBR channel bytes");
    }
    let base=published.pixel("foliage-leaves.png").unwrap();
    let season=published.pixel("foliage-leaves-season.png").unwrap();
    assert_ne!(base,season,"Autumn season must tint Oak leaf pixels");
    for (a,b) in base.chunks_exact(4).zip(season.chunks_exact(4)) {
        assert_eq!(a[3],b[3],"season snapshots must preserve leaf cutout");
    }
    let mtl=published.static_mtl(&asset.mtl_pbr());
    assert!(mtl.contains("map_Kd foliage-leaves-season.png"));
    assert!(!mtl.contains("map_Kd foliage-leaves.png"));
    assert!(asset.obj_pbr().unwrap().contains("mtllib asset.mtl"));
    assert!(asset.obj_pbr().unwrap()
        .replace("mtllib asset.mtl","mtllib asset-static.mtl")
        .contains("mtllib asset-static.mtl"));
    let snapshot_json=serde_json::to_string(&published.snapshot).unwrap();
    let parsed=StaticAppearanceSnapshot::from_json(&snapshot_json).unwrap();
    assert_eq!(parsed.snapshot_id,published.snapshot.snapshot_id);
    for texture in &parsed.textures {
        assert_eq!(published.pixel(&texture.name).unwrap().len(),
            (size*size*4) as usize);
    }
}

#[test]
fn different_seeded_trees_reuse_actual_content_arc_and_texture_ids() {
    let prefab=FloraPrefab::from_preset(PlantPreset::Oak);
    let first=RuntimeMaterialRecipe::from_flora(&prefab).unwrap();
    let mut second_seed=prefab.source.clone();
    second_seed.seed=second_seed.seed.wrapping_add(19);
    let second=RuntimeMaterialRecipe::from_tree(&second_seed).unwrap();
    assert_eq!(first.recipe_id,second.recipe_id,
        "independent tree growth seeds must not multiply material variants");
    let (mut f,mut g)=caches();
    let bark=f.bake_bark(&first.materials,32,first.bark_color).unwrap();
    let leaf=f.bake_leaf(&first.materials,32,first.leaf_color).unwrap();
    let other_bark=f.bake_bark(&second.materials,32,second.bark_color).unwrap();
    let other_leaf=f.bake_leaf(&second.materials,32,second.leaf_color).unwrap();
    assert!(Arc::ptr_eq(&bark,&other_bark));
    assert!(Arc::ptr_eq(&leaf,&other_leaf));
    assert_eq!(f.stats().hits,2);
    assert_eq!(f.stats().misses,2);
    let a=publish(&first,32,1.0,&mut f,&mut g).unwrap();
    let b=publish(&second,32,1.0,&mut f,&mut g).unwrap();
    assert_eq!(a.snapshot.snapshot_id,b.snapshot.snapshot_id);
    assert_eq!(a.snapshot.textures.iter().map(|x|&x.resource_id)
        .collect::<Vec<_>>(),b.snapshot.textures.iter().map(|x|&x.resource_id)
        .collect::<Vec<_>>());
    assert_eq!(a.snapshot.preview_only_effects,b.snapshot.preview_only_effects);
}

#[test]
fn season_only_changes_the_static_snapshot_not_bark_leaf_or_pbr_resources() {
    let tree=FoliageProject::from_preset(PlantPreset::Oak);
    let recipe=RuntimeMaterialRecipe::from_tree(&tree).unwrap();
    let (mut f,mut g)=caches();
    let summer=publish(&recipe,32,1.0,&mut f,&mut g).unwrap();
    let misses=f.stats().misses;
    let autumn=publish(&recipe,32,2.0,&mut f,&mut g).unwrap();
    assert_eq!(f.stats().misses,misses);
    assert_ne!(summer.snapshot.snapshot_id,autumn.snapshot.snapshot_id);
    let references=|p:&texture_studio::material_publish::MaterialPublication| {
        p.snapshot.textures.iter().map(|x|(x.name.clone(),x.resource_id.clone()))
            .collect::<HashMap<_,_>>()
    };
    let s=references(&summer);
    let a=references(&autumn);
    assert_eq!(s.len(),a.len());
    for (name,id) in s {
        if name=="foliage-leaves-season.png" {
            assert_ne!(id,a[name]);
        } else {
            assert_eq!(id,a[name],"{name} incorrectly rebaked for season only");
        }
    }
    let wrap=publish(&recipe,32,4.0,&mut f,&mut g).unwrap();
    let spring=publish(&recipe,32,0.0,&mut f,&mut g).unwrap();
    assert_eq!(wrap.snapshot.snapshot_id,spring.snapshot.snapshot_id);
    assert_eq!(wrap.pixel("foliage-leaves-season.png"),
        spring.pixel("foliage-leaves-season.png"));
}

#[test]
fn vector_silhouette_palette_and_authored_graph_inputs_invalidate_correctly() {
    let tree=FoliageProject::from_preset(PlantPreset::Oak);
    let original=RuntimeMaterialRecipe::from_tree(&tree).unwrap();
    let mut vector=tree.clone();
    vector.materials.leaf_silhouette=LeafSilhouetteSource::VectorOak{lobe_width:1.0};
    vector.materials.leaf_alpha_sampling=LeafAlphaSampling::Coverage2x2;
    let vector_recipe=RuntimeMaterialRecipe::from_tree(&vector).unwrap();
    assert_ne!(original.recipe_id,vector_recipe.recipe_id);
    let mut palette=tree.clone();
    let mut stops=palette.materials.effective_palettes(palette.leaf_color);
    stops.seasons[2][2]=[250,35,15];
    palette.materials.seasonal_palettes=Some(stops);
    let palette_recipe=RuntimeMaterialRecipe::from_tree(&palette).unwrap();
    assert_ne!(original.recipe_id,palette_recipe.recipe_id);
    let (mut f,mut g)=caches();
    let base=publish(&original,32,2.0,&mut f,&mut g).unwrap();
    let palette_only=publish(&palette_recipe,32,2.0,&mut f,&mut g).unwrap();
    assert_eq!(base.pixel("bark-albedo.png"),
        palette_only.pixel("bark-albedo.png"));
    assert_eq!(base.pixel("foliage-leaves.png"),
        palette_only.pixel("foliage-leaves.png"));
    assert_ne!(base.pixel("foliage-leaves-season.png"),
        palette_only.pixel("foliage-leaves-season.png"));
    let vec_bake=publish(&vector_recipe,32,2.0,&mut f,&mut g).unwrap();
    assert_ne!(base.pixel("foliage-leaves.png"),
        vec_bake.pixel("foliage-leaves.png"));
    assert_eq!(base.pixel("bark-albedo.png"),vec_bake.pixel("bark-albedo.png"));
    let mut tinted=tree.clone();
    tinted.materials.bark_tint_strength=1.0;
    let tint_recipe=RuntimeMaterialRecipe::from_tree(&tinted).unwrap();
    assert_ne!(tint_recipe.recipe_id,original.recipe_id);
    let tint=publish(&tint_recipe,32,2.0,&mut f,&mut g).unwrap();
    assert_ne!(tint.pixel("bark-albedo.png"),base.pixel("bark-albedo.png"));
}

#[test]
fn all_flora_materials_and_representative_bakes_are_valid_and_portable() {
    assert_eq!(PlantPreset::ALL.len(),43);
    for species in PlantPreset::ALL {
        let tree=FoliageProject::from_preset(species);
        let recipe=RuntimeMaterialRecipe::from_tree(&tree).unwrap();
        assert_eq!(recipe.format_version,RECIPE_VERSION);
        assert!(recipe.embedded_sources.is_empty());
        assert_eq!(RuntimeMaterialRecipe::from_json(&recipe.to_json().unwrap())
            .unwrap().recipe_id,recipe.recipe_id);
    }
    let selected=[
        PlantPreset::Oak,PlantPreset::Pine,PlantPreset::Willow,
        PlantPreset::FremontCottonwood,PlantPreset::CoconutPalm,
        PlantPreset::GreenBamboo,PlantPreset::CaturraCoffee,
    ];
    let (mut f,mut g)=caches();
    for species in selected {
        let tree=FoliageProject::from_preset(species);
        let recipe=RuntimeMaterialRecipe::from_tree(&tree).unwrap();
        let pubd=publish(&recipe,16,2.0,&mut f,&mut g).unwrap();
        assert_eq!(pubd.snapshot.format_version,SNAPSHOT_VERSION);
        assert_eq!(pubd.pixel("bark-albedo.png").unwrap().len(),16*16*4);
        assert_eq!(pubd.pixel("foliage-leaves-season.png").unwrap().len(),16*16*4);
        let native=tree.materials.bake_leaf(16,tree.leaf_color).unwrap();
        assert_eq!(pubd.pixel("foliage-leaves.png").unwrap(),native);
    }
    assert!(f.stats().resident_bytes<=16*1024*1024);
    assert!(g.stats().resident_bytes<=16*1024*1024);
}

#[test]
fn snapshot_is_texel_equivalent_to_wgsl_palette_formula_and_preserves_alpha() {
    let tree=FoliageProject::from_preset(PlantPreset::Oak);
    let recipe=RuntimeMaterialRecipe::from_tree(&tree).unwrap();
    let size=32;
    let raw=recipe.materials.bake_leaf(size,recipe.leaf_color).unwrap();
    let season=1.75;
    let output=bake_leaf_season(&raw,size,recipe.leaf_color,
        &recipe.palette,season).unwrap();
    for (index,(src,dst)) in raw.chunks_exact(4)
        .zip(output.chunks_exact(4)).enumerate() {
        let y=index as u32/size;
        let factor=1.0-(y as f32+0.5)/size as f32;
        let grad=recipe.palette.sample(season,factor);
        for c in 0..3 {
            let base=(recipe.leaf_color[c] as f32/255.0).max(0.12);
            let expected=((src[c] as f32/255.0*grad[c]/base).min(1.0)*255.0)
                .round() as u8;
            assert_eq!(dst[c],expected);
        }
        assert_eq!(dst[3],src[3]);
    }
}

#[test]
fn corrupted_versions_sources_and_invalid_seasons_are_rejected() {
    let tree=FoliageProject::from_preset(PlantPreset::Oak);
    let recipe=RuntimeMaterialRecipe::from_tree(&tree).unwrap();
    let mut value:serde_json::Value=serde_json::from_str(
        &recipe.to_json().unwrap()).unwrap();
    value["format_version"]=serde_json::json!(999);
    assert!(RuntimeMaterialRecipe::from_json(&value.to_string()).is_err());
    value["format_version"]=serde_json::json!(RECIPE_VERSION);
    value["bark_color"]=serde_json::json!([20,40,60]);
    assert!(RuntimeMaterialRecipe::from_json(&value.to_string()).is_err());
    let (mut f,mut g)=caches();
    for season in [-1.0,4.1,f32::NAN,f32::INFINITY] {
        assert!(publish(&recipe,16,season,&mut f,&mut g).is_err());
    }
    assert!(publish(&recipe,0,1.0,&mut f,&mut g).is_err());
    assert!(publish(&recipe,2048,1.0,&mut f,&mut g).is_err());
    assert!(bake_leaf_season(&[255;5],1,recipe.leaf_color,
        &recipe.palette,1.0).is_err());

    let p=publish(&recipe,16,1.0,&mut f,&mut g).unwrap();
    let json=serde_json::to_value(&p.snapshot).unwrap();
    let mut tampered=json.clone();
    tampered["resolution"]=serde_json::json!(0);
    assert!(StaticAppearanceSnapshot::from_json(&tampered.to_string()).is_err());
    tampered=json.clone();
    tampered["textures"][0]["color_space"]=serde_json::json!("unknown");
    assert!(StaticAppearanceSnapshot::from_json(&tampered.to_string()).is_err());
}

#[test]
fn embedded_source_provenance_is_reported_as_unverified_not_cc0() {
    let mut graph=Graph::default();
    graph.add_bitmap_rgba("author-photo.png",2,2,vec![40;16]).unwrap();
    let mut tree=FoliageProject::from_preset(PlantPreset::Oak);
    tree.materials.bark=Some(graph);
    let recipe=RuntimeMaterialRecipe::from_tree(&tree).unwrap();
    assert_eq!(recipe.embedded_sources.len(),1);
    assert_eq!(recipe.embedded_sources[0].name,"author-photo.png");
    assert_eq!(recipe.embedded_sources[0].role,"bark");
    assert_eq!(recipe.embedded_sources[0].license_status,"unverified");
    let json=recipe.to_json().unwrap();
    assert!(json.contains("unverified"));
    assert_eq!(RuntimeMaterialRecipe::from_json(&json).unwrap()
        .recipe_id,recipe.recipe_id);
}
