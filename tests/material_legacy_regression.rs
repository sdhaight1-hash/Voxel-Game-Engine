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
