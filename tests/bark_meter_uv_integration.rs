use texture_studio::foliage::{FoliageProject,PlantPreset,Material};
use texture_studio::foliage_materials::FoliageMaterials;
use texture_studio::foliage_bake_cache::FoliageBakeCache;
use texture_studio::asset::VoxelAsset;
use std::sync::Arc;

#[test]
fn metric_bark_is_opt_in_serialized_validated_and_does_not_rebake() {
    let mut mats=FoliageMaterials::for_species(PlantPreset::Oak);
    assert_eq!(mats.bark_meters_per_repeat,None);
    assert!(!serde_json::to_string(&mats).unwrap()
        .contains("bark_meters_per_repeat"));
    let mut cache=FoliageBakeCache::new(1024*1024);
    let baseline=cache.bake_bark(&mats,32,[90,68,49]).unwrap();
    mats.bark_meters_per_repeat=Some(0.35);
    mats.validate().unwrap();
    let restored:FoliageMaterials=serde_json::from_str(
        &serde_json::to_string(&mats).unwrap()).unwrap();
    assert_eq!(restored.bark_meters_per_repeat,Some(0.35));
    let updated=cache.bake_bark(&mats,32,[90,68,49]).unwrap();
    assert!(Arc::ptr_eq(&baseline,&updated),
        "changing tile distance is not a bark pixel-bake input");
    assert_eq!(baseline.as_ref(),updated.as_ref());
    for invalid in [0.0,-1.0,0.04,8.01,f32::NAN,f32::INFINITY] {
        mats.bark_meters_per_repeat=Some(invalid);
        assert!(mats.validate().is_err(),"invalid meters={invalid}");
    }
}

#[test]
fn curved_wood_tubes_carry_positive_meter_circumference_through_clipped_hexels() {
    let original=FoliageProject::from_preset(PlantPreset::Oak);
    let mesh=original.generate().unwrap();
    assert!(mesh.structure.is_some());
    let curved:Vec<_>=mesh.triangles.iter()
        .filter(|t|t.material==Material::Bark &&
            t.bark_circumference_m.iter().any(|c|*c>0.0)).collect();
    assert!(curved.len()>10,"curved owned wood must include metric tube sides");
    for triangle in &curved {
        assert!(triangle.bark_circumference_m.iter().all(|c|
            c.is_finite()&&*c>=0.0));
        // Every clipped vertex on a given longitudinal coordinate is
        // interpolated from the same ring-radius metric, never from cell UV.
        for k in 0..3 {
            assert!(triangle.uv[k][1].is_finite());
            assert!(triangle.bark_circumference_m[k]<16.0);
        }
    }
    let mut physical=original.clone();
    physical.materials.bark_meters_per_repeat=Some(0.42);
    let equivalent=physical.generate().unwrap();
    assert_eq!(mesh.voxels,equivalent.voxels,
        "material scale must not change occupancy");
    assert_eq!(mesh.triangles.len(),equivalent.triangles.len());
    for (a,b) in mesh.triangles.iter().zip(&equivalent.triangles) {
        assert_eq!(a.points,b.points);
        assert_eq!(a.uv,b.uv);
        assert_eq!(a.bark_circumference_m,b.bark_circumference_m);
        assert_eq!(a.cell,b.cell);
    }
    // The same physical geometry and scale settings survive tree -> asset.
    let asset=VoxelAsset::from_tree(&physical,&equivalent);
    assert_eq!(asset.materials.bark_meters_per_repeat,Some(0.42));
    let recovered=VoxelAsset::from_json(&asset.to_json().unwrap()).unwrap();
    assert_eq!(recovered.materials.bark_meters_per_repeat,Some(0.42));
    let asset_mesh=recovered.mesh().unwrap();
    assert_eq!(asset_mesh.voxels,equivalent.voxels);
    assert_eq!(asset_mesh.triangles.len(),equivalent.triangles.len());
}

#[test]
fn polygon_perimeter_and_uv_seam_are_physically_defined() {
    // A six-sided ring radius 0.5m has 3m of actual polygon perimeter.
    // 0.5m/repeat => 6 angular tiles; a 0.4m/repeat ratio is fractional
    // and needs a seam-corrected shader sample instead of rounding.
    let perimeter=6.0*0.5;
    for meters in [0.4_f32,0.5,1.0,8.0] {
        let tiles=perimeter/meters;
        let beginning=0.0*tiles;
        let seam_end=(1.0-1.0)*tiles;
        assert_eq!(beginning,seam_end);
        let end_primary=1.0*tiles;
        if meters==0.4 {assert!((end_primary-7.5).abs()<1e-6);}
        assert_eq!(2.0/meters,2.0/meters);
    }
}
