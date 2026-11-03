use texture_studio::foliage::{FoliageProject,PlantPreset,Material,FoliageMesh};
use texture_studio::asset::VoxelAsset;

fn vt_lines(obj:&str)->Vec<&str> {
    obj.lines().filter(|line|line.starts_with("vt ")).collect()
}
fn non_uv_lines(obj:&str)->Vec<&str> {
    obj.lines().filter(|line|!line.starts_with("vt ")).collect()
}

#[test]
fn default_and_none_exports_are_byte_identical_to_legacy_obj() {
    for preset in [PlantPreset::Oak,PlantPreset::Birch] {
        let project=FoliageProject::from_preset(preset);
        assert!(project.materials.bark_meters_per_repeat.is_none());
        let mesh=project.generate().unwrap();
        assert_eq!(mesh.obj_with_bark_meters_per_repeat(None).unwrap(),mesh.obj());
        // The legacy OBJ's material, vertex, face and UV ordering is untouched.
        assert!(mesh.obj().starts_with("# Voxel tree surface mesh"));
        assert!(mesh.obj().contains("mtllib foliage.mtl"));
    }
}

#[test]
fn static_metric_obj_uvs_preserve_wood_topology_leaves_and_physical_density() {
    let project=FoliageProject::from_preset(PlantPreset::Oak);
    let mesh=project.generate().unwrap();
    let legacy=mesh.obj();
    let meters=0.4_f32;
    let metric=mesh.obj_with_bark_meters_per_repeat(Some(meters)).unwrap();
    assert_ne!(legacy,metric,"organic bark must use metric UV when enabled");
    // All non-UV OBJ records, including texture material bindings and
    // triangle faces, must remain byte-for-byte identical.
    assert_eq!(non_uv_lines(&metric),non_uv_lines(&legacy));
    let legacy_uv=vt_lines(&legacy);
    let metric_uv=vt_lines(&metric);
    assert_eq!(metric_uv.len(),mesh.triangles.len()*3);
    let mut changed=0;
    let mut unchanged=0;
    let mut inner=0;
    let mut seam=0;
    for (i,triangle) in mesh.triangles.iter().enumerate() {
        for corner in 0..3 {
            let index=i*3+corner;
            let uv=triangle.uv[corner];
            let perimeter=triangle.bark_circumference_m[corner];
            if triangle.material!=Material::Bark||perimeter<=0.0 {
                assert_eq!(metric_uv[index],legacy_uv[index],
                    "unsupported bark, caps and foliage must retain original coordinates");
                unchanged+=1;
                continue;
            }
            changed+=1;
            let repeats=perimeter/meters;
            if uv[0]<=0.8 {
                assert_eq!(metric_uv[index],
                    format!("vt {:.6} {:.6}",uv[0]*repeats,1.0-uv[1]/meters));
                inner+=1;
            }
            if (uv[0]-1.0).abs()<1e-6 {
                assert_eq!(metric_uv[index],
                    format!("vt {:.6} {:.6}",repeats.round(),1.0-uv[1]/meters));
                seam+=1;
            }
        }
    }
    assert!(changed>100 && unchanged>100 && inner>50 && seam>0,
        "all tube, seam and non-tube paths must be exercised");
    assert!(metric_uv.iter().zip(&legacy_uv).any(|(metric,legacy)|metric!=legacy));
    // Editing a static export's coordinates never mutates physical hexels.
    assert_eq!(mesh.voxels,project.generate().unwrap().voxels);
}

#[test]
fn editable_tree_assets_export_the_same_metric_uvs_and_material_binding() {
    let mut tree=FoliageProject::from_preset(PlantPreset::Oak);
    tree.materials.bark_meters_per_repeat=Some(0.65);
    let mesh=tree.generate().unwrap();
    let asset=VoxelAsset::from_tree(&tree,&mesh);
    let restored=VoxelAsset::from_json(&asset.to_json().unwrap()).unwrap();
    let exported=restored.obj().unwrap();
    assert!(exported.contains("mtllib asset.mtl"));
    let model=restored.mesh().unwrap();
    let direct=model.obj_with_bark_meters_per_repeat(Some(0.65)).unwrap()
        .replace("mtllib foliage.mtl","mtllib asset.mtl");
    assert_eq!(exported,direct);
    // Disabled means exact legacy Asset behavior.
    let mut old=restored.clone();
    old.materials.bark_meters_per_repeat=None;
    assert_eq!(old.obj().unwrap(),model.obj()
        .replace("mtllib foliage.mtl","mtllib asset.mtl"));
    assert_eq!(vt_lines(&old.obj().unwrap()).len(),
        vt_lines(&exported).len());
    // Authoring data and cell ownership still roundtrip independently.
    assert_eq!(restored.voxels,asset.voxels);
    assert_eq!(restored.materials.bark_meters_per_repeat,Some(0.65));
}

#[test]
fn invalid_export_scales_fail_without_generating_unusable_obj() {
    let mesh=FoliageMesh::from_voxels(texture_studio::VoxelShape::Cube,
        0.5,vec![texture_studio::foliage::VoxelCell{
            x:0,y:0,z:0,material:Material::Bark
        }]).unwrap();
    for invalid in [f32::NAN,f32::INFINITY,-2.0,0.0,0.049,8.001] {
        assert!(mesh.obj_with_bark_meters_per_repeat(Some(invalid)).is_err());
    }
    for value in [0.05,0.4,8.0] {
        assert!(mesh.obj_with_bark_meters_per_repeat(Some(value)).is_ok());
    }
    assert_eq!(mesh.obj_with_bark_meters_per_repeat(Some(0.4)).unwrap(),
        mesh.obj(),"flat block-like bark has no fabricated ring circumference");
}
