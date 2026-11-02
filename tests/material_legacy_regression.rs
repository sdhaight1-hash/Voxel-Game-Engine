//! Pre-roadmap wire format and pixel snapshots. These fixtures intentionally omit
//! newer fields; do not rewrite them when material node metadata evolves.
use texture_studio::{Graph, LibraryNode, NodeKind, VoxelShape};
use texture_studio::asset::VoxelAsset;
use texture_studio::flora_prefab::FloraPrefab;
use texture_studio::foliage::{FoliageProject, PlantPreset};
use texture_studio::foliage_materials::FoliageMaterials;

fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325_u64,
        |hash, byte| (hash ^ *byte as u64).wrapping_mul(0x100000001b3))
}

fn old_graph() -> Graph {
    serde_json::from_str(include_str!("fixtures/legacy_graph_v0.json")).unwrap()
}

#[test]
fn legacy_graph_has_frozen_four_channel_pixels_and_identical_roundtrip() {
    let graph=old_graph();
    graph.validate().unwrap();
    assert_eq!(graph.voxel.shape,VoxelShape::Cube, "missing voxel field defaults to Cube");
    let expected=[
        0x639ef9c25a6387a5, // Albedo: constant [32,96,160,255]
        0x18f61efb17c8e9e5, // Roughness: two-by-two checker [219]/[36]
        0x0852db856e95b5a5, // Metallic: disconnected, black
        0x18f61efb17c8e9e5, // Height: same checker
    ];
    for (channel,expected_hash) in expected.into_iter().enumerate() {
        let actual=graph.render_output_channel_rgba(channel,4,4).unwrap();
        assert_eq!(fnv1a(&actual),expected_hash,"legacy channel {channel} changed");
        let rebuilt:Graph=serde_json::from_str(&serde_json::to_string(&graph).unwrap()).unwrap();
        assert_eq!(rebuilt.render_output_channel_rgba(channel,4,4).unwrap(),actual);
    }
    let height=graph.render_output_channel_rgba(3,4,4).unwrap();
    assert_eq!(&height[0..4],&[219,219,219,255]);
    assert_eq!(&height[8..12],&[36,36,36,255]);
    assert_eq!(graph.render_normal_rgba(4,4).unwrap(),
        serde_json::from_str::<Graph>(&serde_json::to_string(&graph).unwrap())
            .unwrap().render_normal_rgba(4,4).unwrap());
}

/// Verified against the unoptimized CPU sampler on Rust CI Linux x86-64.
#[test]
fn frozen_legacy_albedo_hashes_at_export_resolutions() {
    let graph=old_graph();
    for (size,expected) in [
        (256,0xa632dee47d4a2325_u64),
        (512,0xd520d3e468c22325),
        (1024,0xf37e68e416a22325),
        (2048,0x9750cce2ce222325),
    ] {
        assert_eq!(fnv1a(&graph.render_rgba(size,size).unwrap()),expected,
            "legacy albedo changed at {size}px");
    }
    for (size,expected) in [
        (256,0x1ce3545ecc0e2325_u64),
        (512,0xb137934da3d22325),
        (1024,0x6a57fe8902e22325),
        (2048,0x00a0a3767f222325),
    ] {
        assert_eq!(fnv1a(&graph.render_output_channel_rgba(1,size,size).unwrap()),
            expected,"legacy checker roughness changed at {size}px");
    }
}

#[test]
fn legacy_asset_retains_owned_material_and_editable_cells() {
    let asset=VoxelAsset::from_json(include_str!("fixtures/legacy_asset_v1.json")).unwrap();
    assert_eq!(asset.shape,VoxelShape::HexPrism);
    assert_eq!(asset.voxels.len(),2);
    assert_eq!(asset.materials.bark_tint_strength,0.0);
    assert!(asset.material_graph.is_some(), "legacy general Asset graphs must survive");
    assert_eq!(asset.material_graph.as_ref().unwrap().render_rgba(4,4).unwrap(),
        old_graph().render_rgba(4,4).unwrap());
    let decoded=VoxelAsset::from_json(&asset.to_json().unwrap()).unwrap();
    assert_eq!(decoded.voxels,asset.voxels);
    assert_eq!(decoded.mesh().unwrap().triangle_count(),
        asset.mesh().unwrap().triangle_count());
    assert_eq!(decoded.material_graph.unwrap().render_rgba(4,4).unwrap(),
        old_graph().render_rgba(4,4).unwrap());
}

#[test]
fn foliage_and_flora_without_new_material_fields_still_load() {
    let original=FoliageProject::from_preset(PlantPreset::Willow);
    let mut value=serde_json::to_value(&original).unwrap();
    let object=value.as_object_mut().unwrap();
    object.remove("materials");
    object.remove("leaf_render");
    object.remove("growth_rules_override");
    object.remove("trunk_rules_override");
    let decoded:FoliageProject=serde_json::from_value(value.clone()).unwrap();
    decoded.validate().unwrap();
    assert_eq!(decoded.materials.bark_tint_strength,0.0);
    assert!(decoded.materials.bark.is_none());
    assert_eq!(decoded.seed,original.seed);
    let roundtrip:FoliageProject=serde_json::from_str(
        &serde_json::to_string(&decoded).unwrap()).unwrap();
    assert_eq!(roundtrip.generate().unwrap().voxels,
        decoded.generate().unwrap().voxels);
    let mut prefab=serde_json::to_value(FloraPrefab::from_preset(PlantPreset::Willow)).unwrap();
    prefab["source"]=value;
    let restored=FloraPrefab::from_json(&prefab.to_string()).unwrap();
    assert_eq!(restored.species,PlantPreset::Willow);
    assert_eq!(restored.source.materials.bark_tint_strength,0.0);
}

#[test]
fn legacy_hexel_tree_export_preserves_cells_and_owner_metadata() {
    let project=FoliageProject::from_preset(PlantPreset::Oak);
    let mesh=project.generate().unwrap();
    let mut old:serde_json::Value=serde_json::from_str(&mesh.voxel_json(&project).unwrap()).unwrap();
    // The historic format predates both tint strength and curved trunk spans.
    old["settings"]["materials"].as_object_mut().unwrap().remove("bark_tint_strength");
    old["structure"].as_object_mut().unwrap().remove("spans");
    let imported=VoxelAsset::from_json(&old.to_string()).unwrap();
    assert_eq!(imported.shape,VoxelShape::HexPrism);
    assert_eq!(imported.voxels,mesh.voxels);
    assert!(imported.tree_structure.is_some());
    assert_eq!(imported.materials.bark_tint_strength,0.0);
    assert!(imported.material_graph.is_none());
    let restored=VoxelAsset::from_json(&imported.to_json().unwrap()).unwrap();
    assert_eq!(restored.voxels,mesh.voxels);
}

#[test]
fn old_materials_without_new_tint_field_keep_bark_graph_pixels() {
    let mut old=serde_json::to_value(FoliageMaterials::default()).unwrap();
    old.as_object_mut().unwrap().remove("bark_tint_strength");
    old["bark"]=serde_json::from_str(include_str!("fixtures/legacy_graph_v0.json")).unwrap();
    let materials:FoliageMaterials=serde_json::from_value(old).unwrap();
    assert_eq!(materials.bark_tint_strength,0.0);
    let baseline=old_graph().render_rgba(32,32).unwrap();
    assert_eq!(materials.bake_bark(32,[210,40,80]).unwrap(),baseline);
    assert_eq!(materials.bake_bark(32,[20,170,180]).unwrap(),baseline);
}

#[test]
fn branching_warped_materials_remain_byte_identical_after_old_graph_roundtrip() {
    // No algorithm rewrite or raster cache is permitted to change lazy UV sampling.
    let mut warped=Graph::default();
    let warp=warped.add_node(NodeKind::Library {
        operation:LibraryNode::Warp,amount:0.4,seed:41,
    },[120.0,10.0]);
    warped.connect(1,warp,0).unwrap();
    warped.connect(warp,2,0).unwrap();
    warped.connect(warp,2,3).unwrap();
    let mut old=serde_json::to_value(&warped).unwrap();
    old.as_object_mut().unwrap().remove("voxel");
    let restored:Graph=serde_json::from_value(old).unwrap();
    for size in [16,64,256] {
        for channel in [0,3] {
            assert_eq!(warped.render_output_channel_rgba(channel,size,size).unwrap(),
                restored.render_output_channel_rgba(channel,size,size).unwrap(),
                "warp changed after legacy JSON decode at {size}px channel {channel}");
        }
    }
}

/// The CI benchmark step runs this alongside the pre-optimization baseline.
/// It measures repeat-output reuse (not GPU upload or intermediate graph caching).
#[test]
#[ignore = "run explicitly to compare cold and warm graph cache output"]
fn profile_bounded_graph_output_cache() {
    use std::time::Instant;
    use texture_studio::render_cache::GraphRenderCache;
    let graph=old_graph();
    let mut cache=GraphRenderCache::new(64*1024*1024);
    for size in [256,512,1024,2048] {
        let cold_started=Instant::now();
        let cold=cache.render_channel(&graph,0,size,size).unwrap();
        let cold_ms=cold_started.elapsed().as_secs_f64()*1000.0;
        let warm_started=Instant::now();
        let warm=cache.render_channel(&graph,0,size,size).unwrap();
        let warm_ms=warm_started.elapsed().as_secs_f64()*1000.0;
        assert!(std::sync::Arc::ptr_eq(&cold,&warm));
        let stats=cache.stats();
        println!("material-cache-baseline graph=legacy_color size={size} cold_ms={cold_ms:.3} warm_ms={warm_ms:.3} hits={} misses={} resident_bytes={} peak_bytes={} fnv64={:016x}",
            stats.hits,stats.misses,stats.resident_bytes,
            stats.peak_resident_bytes,fnv1a(warm.as_ref()));
    }
    let mut warped=Graph::default();
    let warp=warped.add_node(NodeKind::Library{
        operation:LibraryNode::Warp,amount:0.4,seed:41,
    },[120.0,10.0]);
    warped.connect(1,warp,0).unwrap();
    warped.connect(warp,2,0).unwrap();
    let started=Instant::now();
    let cold=cache.render_channel(&warped,0,512,512).unwrap();
    let cold_ms=started.elapsed().as_secs_f64()*1000.0;
    let started=Instant::now();
    let warm=cache.render_channel(&warped,0,512,512).unwrap();
    let warm_ms=started.elapsed().as_secs_f64()*1000.0;
    assert!(std::sync::Arc::ptr_eq(&cold,&warm));
    println!("material-cache-baseline graph=warped_noise size=512 cold_ms={cold_ms:.3} warm_ms={warm_ms:.3} fnv64={:016x}",
        fnv1a(warm.as_ref()));
}

#[test]
fn multi_channel_memo_is_compatible_with_all_catalog_materials() {
    use texture_studio::presets::builtin_presets;
    use texture_studio::VoxelSettings;
    let recipes=builtin_presets().unwrap();
    assert_eq!(recipes.len(),1039);
    for preset in recipes {
        let graph=preset.build_graph(VoxelSettings::default());
        let (channels,stats)=graph.render_channels_memoized_rgba(&[0,1,2,3],2,2)
            .unwrap_or_else(|err|panic!("{} / {}: {err}",preset.material,preset.variant));
        for channel in 0..4 {
            assert_eq!(channels[channel],
                graph.render_output_channel_rgba(channel,2,2).unwrap(),
                "{} / {} channel {channel}",preset.material,preset.variant);
        }
        assert!(stats.peak_entries_per_pixel<=4096);
    }
}

/// CI's ignored benchmark step measures *CPU* savings from shared channel
/// sampling. It is a comparative metric, not an assertion of speedup.
#[test]
#[ignore = "compare standalone versus exact-UV memoized output baking"]
fn profile_multi_channel_node_memoization() {
    use std::time::Instant;
    let mut graph=Graph::demo_bricks();
    // Share one Noise graph between two PBR output channels.
    graph.connect(3,4,2).unwrap();
    for size in [64,256] {
        let started=Instant::now();
        let independent:Vec<_>=(0..4).map(|channel|
            graph.render_output_channel_rgba(channel,size,size).unwrap()).collect();
        let independent_ms=started.elapsed().as_secs_f64()*1000.0;
        let started=Instant::now();
        let (memoized,stats)=graph.render_channels_memoized_rgba(
            &[0,1,2,3],size,size).unwrap();
        let memoized_ms=started.elapsed().as_secs_f64()*1000.0;
        assert_eq!(independent,memoized);
        println!("material-node-memo size={size} independent_ms={independent_ms:.3} memoized_ms={memoized_ms:.3} sample_hits={} node_evaluations={} peak_entries_per_pixel={} albedo_fnv64={:016x}",
            stats.reused_samples,stats.evaluated_samples,stats.peak_entries_per_pixel,
            fnv1a(&memoized[0]));
    }
}

/// Opt-in comparison on exactly the same hardware/build profile. Measures the
/// final material recipe cache, not graph intermediate caching or GPU uploads.
#[test]
#[ignore = "measure cold and reused bark/leaf material bakes"]
fn profile_shared_foliage_bake_cache() {
    use std::time::Instant;
    use texture_studio::foliage_bake_cache::FoliageBakeCache;
    let materials=FoliageMaterials::for_species(PlantPreset::Willow);
    let mut cache=FoliageBakeCache::new(32*1024*1024);
    for size in [64,256] {
        for (role,tint) in [("leaf",[81,139,73]),("bark",[117,91,62])] {
            let before=Instant::now();
            let cold=if role=="leaf" {
                cache.bake_leaf(&materials,size,tint).unwrap()
            } else {
                cache.bake_bark(&materials,size,tint).unwrap()
            };
            let cold_ms=before.elapsed().as_secs_f64()*1000.0;
            let before=Instant::now();
            let warm=if role=="leaf" {
                cache.bake_leaf(&materials,size,tint).unwrap()
            } else {
                cache.bake_bark(&materials,size,tint).unwrap()
            };
            let warm_ms=before.elapsed().as_secs_f64()*1000.0;
            assert!(std::sync::Arc::ptr_eq(&cold,&warm));
            let stats=cache.stats();
            println!("material-recipe-cache role={role} size={size} cold_ms={cold_ms:.3} warm_ms={warm_ms:.3} hits={} misses={} resident_bytes={} fnv64={:016x}",
                stats.hits,stats.misses,stats.resident_bytes,fnv1a(warm.as_ref()));
        }
    }
}

/// Opt-in measurement, never a CI correctness assertion. GitHub Actions can
/// execute it with `cargo test --test material_legacy_regression -- --ignored --nocapture`.
/// Record the CPU/runner for meaningful before/after comparisons.
#[test]
#[ignore = "run explicitly to record a CPU/graph-render baseline"]
fn profile_material_graph_baseline() {
    use std::time::Instant;
    let reference=old_graph();
    let mut warped=Graph::default();
    let warp=warped.add_node(NodeKind::Library {
        operation:LibraryNode::Warp,amount:0.4,seed:41,
    },[120.0,10.0]);
    warped.connect(1,warp,0).unwrap();
    warped.connect(warp,2,0).unwrap();
    println!("material-baseline version={} os={} arch={} workers={}",
        env!("CARGO_PKG_VERSION"),std::env::consts::OS,std::env::consts::ARCH,
        std::thread::available_parallelism().map_or(1,|n|n.get()));
    for (name,graph,sizes,channel) in [
        ("legacy_color",&reference,[256,512,1024,2048].as_slice(),0),
        ("legacy_checker",&reference,[256,512,1024,2048].as_slice(),1),
        ("warped_noise",&warped,[64,128,256,512].as_slice(),0),
    ] {
        for &size in sizes {
            for repetition in 0..2 {
                let started=Instant::now();
                let pixels=graph.render_output_channel_rgba(channel,size,size).unwrap();
                let elapsed=started.elapsed();
                println!("material-baseline graph={name} size={size} run={repetition} elapsed_ms={:.3} fnv64={:016x} bytes={}",
                    elapsed.as_secs_f64()*1000.0,fnv1a(&pixels),pixels.len());
                std::hint::black_box(pixels);
            }
        }
    }
}
