use texture_studio::foliage::{FoliageMesh,FoliageProject,PlantPreset,Material,VoxelCell};
use texture_studio::foliage_materials::FoliageMaterials;
use texture_studio::asset::VoxelAsset;
use texture_studio::voxel::VoxelShape;
use std::collections::HashMap;

fn records<'a>(text:&'a str,prefix:&str)->Vec<&'a str> {
    text.lines().filter(|line|line.starts_with(prefix)).collect()
}

#[test]
fn pbr_obj_adds_one_geometric_normal_per_triangle_without_touching_legacy_geometry() {
    let tree=FoliageProject::from_preset(PlantPreset::Oak);
    let mesh=tree.generate().unwrap();
    let legacy=mesh.obj();
    let enhanced=mesh.obj_with_pbr_normals(None).unwrap();
    assert_eq!(mesh.obj(),legacy,"legacy surface syntax must be identical");
    assert_eq!(records(&enhanced,"v "),records(&legacy,"v "));
    assert_eq!(records(&enhanced,"vt "),records(&legacy,"vt "));
    assert_eq!(records(&enhanced,"usemtl "),records(&legacy,"usemtl "));
    let faces=records(&enhanced,"f ");
    let normals=records(&enhanced,"vn ");
    assert_eq!(faces.len(),mesh.triangles.len());
    assert_eq!(normals.len(),mesh.triangles.len());
    assert!(faces.len()>100);
    for (i,(normal,face)) in normals.iter().zip(&faces).enumerate() {
        let xyz:Vec<f32>=normal.split_whitespace().skip(1)
            .map(|v|v.parse::<f32>().unwrap()).collect();
        assert_eq!(xyz.len(),3);
        let points=mesh.triangles[i].points;
        let a:[f32;3]=std::array::from_fn(|axis|points[1][axis]-points[0][axis]);
        let b:[f32;3]=std::array::from_fn(|axis|points[2][axis]-points[0][axis]);
        let cross=[a[1]*b[2]-a[2]*b[1],a[2]*b[0]-a[0]*b[2],
                   a[0]*b[1]-a[1]*b[0]];
        let len=cross.iter().map(|v|v*v).sum::<f32>().sqrt();
        let expected=if len>1e-7&&len.is_finite() {
            cross.map(|v|v/len)
        } else {[0.0,0.0,1.0]};
        for axis in 0..3 {
            assert!((xyz[axis]-expected[axis]).abs()<2e-6);
        }
        let fields:Vec<_>=face.split_whitespace().skip(1).collect();
        assert_eq!(fields.len(),3);
        for (k,vertex) in fields.iter().enumerate() {
            let indices:Vec<_>=vertex.split('/')
                .map(|v|v.parse::<usize>().unwrap()).collect();
            assert_eq!(indices,vec![i*3+k+1,i*3+k+1,i+1]);
        }
    }
    let meter=mesh.obj_with_pbr_normals(Some(0.35)).unwrap();
    let old_meter=mesh.obj_with_bark_meters_per_repeat(Some(0.35)).unwrap();
    assert_eq!(records(&meter,"vt "),records(&old_meter,"vt "));
    assert_ne!(records(&meter,"vt "),records(&legacy,"vt "));
    for invalid in [0.0,-1.0,0.049,8.001,f32::INFINITY,f32::NAN] {
        assert!(mesh.obj_with_pbr_normals(Some(invalid)).is_err());
    }
}

#[test]
fn pbr_mtl_maps_all_exist_and_normal_pixels_match_obj_v_flip() {
    let tree=FoliageProject::from_preset(PlantPreset::Oak);
    let legacy=texture_studio::foliage::mtl(&tree);
    let enhanced=texture_studio::foliage::mtl_pbr(&tree);
    assert!(!legacy.contains("map_Pr "));
    assert!(!legacy.contains("map_Pm "));
    assert!(!legacy.contains("norm "));
    assert!(enhanced.contains("map_Kd foliage-leaves.png"));
    assert!(enhanced.contains("map_d foliage-leaves-alpha.png"));
    assert!(enhanced.contains("map_Pm bark-metallic.png"));
    assert!(enhanced.contains("map_Pr bark-roughness.png"));
    assert!(enhanced.contains("norm bark-normal-obj.png"));
    assert!(enhanced.contains("map_Pr foliage-leaves-roughness.png"));
    assert!(enhanced.contains("norm foliage-leaves-normal-obj.png"));
    let files:HashMap<_,_>=tree.materials.bake_pbr(16).unwrap()
        .into_iter().collect();
    for line in enhanced.lines() {
        if line.starts_with("map_Pr ")||line.starts_with("map_Pm ")
            ||line.starts_with("norm ") {
            let name=line.split_whitespace().nth(1).unwrap();
            assert!(files.contains_key(name),"unbaked MTL map: {name}");
        }
    }
    assert_eq!(files["bark-metallic.png"].len(),16*16*4);
    for kind in ["bark","foliage-leaves"] {
        let native=&files[&format!("{kind}-normal.png")];
        let obj=&files[&format!("{kind}-normal-obj.png")];
        assert_eq!(obj.len(),native.len());
        for (before,after) in native.chunks_exact(4).zip(obj.chunks_exact(4)) {
            assert_eq!(before[0],after[0]);
            assert_eq!(after[1],255-before[1],
                "OBJ vt's 1-v reverses bitangent, so green must invert");
            assert_eq!(before[2],after[2]);
            assert_eq!(before[3],after[3]);
        }
    }
    // A dedicated normal map is needed because the original preview image
    // must remain byte-identical for the Rust wgpu viewer.
    let tiny=FoliageMaterials::default();
    assert!(tiny.bake_pbr(16).unwrap().is_empty());
}

#[test]
fn editable_asset_explicit_pbr_export_keeps_legacy_api_and_cell_ownership() {
    let mut tree=FoliageProject::from_preset(PlantPreset::Oak);
    tree.materials.bark_meters_per_repeat=Some(0.55);
    let grown=tree.generate().unwrap();
    let asset=VoxelAsset::from_tree(&tree,&grown);
    let exported=asset.obj_pbr().unwrap();
    let mesh=asset.mesh().unwrap();
    assert_eq!(exported,mesh.obj_with_pbr_normals(Some(0.55)).unwrap()
        .replace("mtllib foliage.mtl","mtllib asset.mtl"));
    assert_eq!(asset.obj().unwrap(),
        mesh.obj_with_bark_meters_per_repeat(Some(0.55)).unwrap()
        .replace("mtllib foliage.mtl","mtllib asset.mtl"));
    assert_eq!(asset.mtl_pbr(),texture_studio::foliage::mtl_pbr(&tree));
    assert_eq!(asset.voxels.len(),grown.voxels.len());
    let recovered=VoxelAsset::from_json(&asset.to_json().unwrap()).unwrap();
    assert_eq!(recovered.obj_pbr().unwrap(),exported);
    assert_eq!(recovered.mtl_pbr(),asset.mtl_pbr());
}

#[test]
fn flat_voxel_pbr_obj_normals_have_no_nan_and_do_not_require_a_tree() {
    let mesh=FoliageMesh::from_voxels(VoxelShape::Cube,0.5,vec![
        VoxelCell{x:0,y:0,z:0,material:Material::Bark},
    ]).unwrap();
    let obj=mesh.obj_with_pbr_normals(None).unwrap();
    assert_eq!(records(&obj,"vn ").len(),mesh.triangles.len());
    assert!(records(&obj,"vn ").iter().all(|s|!s.contains("NaN")
        &&!s.contains("inf")));
    assert_eq!(records(&obj,"vt "),records(&mesh.obj(),"vt "));
}
