use texture_studio::{Graph,NodeKind,FaceAxis,SideMapping,VoxelShape,VoxelSettings};
use texture_studio::node_definitions::SocketType;
use texture_studio::voxel::{face_normal,render_atlas,render_voxel_preview,atlas_from_pixels};

fn facing_graph(axis:FaceAxis,sharpness:f32)->Graph {
    let mut graph=Graph::default();
    let mask=graph.add_node(NodeKind::FaceFacingMask{axis,sharpness},[20.0,50.0]);
    assert_eq!(graph.node(mask).unwrap().kind.type_id(),"face_facing_mask");
    assert_eq!(graph.node(mask).unwrap().kind.output_type(),SocketType::Mask);
    assert_eq!(graph.node(mask).unwrap().kind.parameters().len(),2);
    graph.connect(mask,2,0).unwrap();
    graph
}

fn center(pixels:&[u8],width:u32,shape:VoxelShape,face:usize,size:u32)->u8 {
    let x=(face%shape.columns()) as u32*size+size/2;
    let y=(face/shape.columns()) as u32*size+size/2;
    pixels[((y*width+x)*4) as usize]
}

#[test]
fn outward_normals_match_real_voxel_face_winding() {
    for shape in [VoxelShape::Cube,VoxelShape::HexPrism] {
        let perimeter=shape.footprint();
        for face in 0..shape.sides() {
            let a=perimeter[face];
            let b=perimeter[(face+1)%shape.sides()];
            let (dx,dy)=(b[0]-a[0],b[1]-a[1]);
            let len=(dx*dx+dy*dy).sqrt();
            let expected=[dy/len,-dx/len,0.0];
            let actual=face_normal(shape,face).unwrap();
            for axis in 0..3 {
                assert!((actual[axis]-expected[axis]).abs()<1e-5,
                    "{shape:?} face={face} axis={axis}");
            }
            let norm=actual.iter().map(|component|component*component)
                .sum::<f32>().sqrt();
            assert!((norm-1.0).abs()<1e-5);
        }
        assert_eq!(face_normal(shape,shape.sides()),Some([0.0,0.0,1.0]));
        assert_eq!(face_normal(shape,shape.sides()+1),Some([0.0,0.0,-1.0]));
        assert_eq!(face_normal(shape,shape.face_count()),None);
    }
}

#[test]
fn directional_masks_match_dot_product_on_cube_and_hex_atlases() {
    let axes=[
        FaceAxis::PosX,FaceAxis::NegX,FaceAxis::PosY,
        FaceAxis::NegY,FaceAxis::PosZ,FaceAxis::NegZ
    ];
    for shape in [VoxelShape::Cube,VoxelShape::HexPrism] {
        for side_mapping in [SideMapping::Wrap,SideMapping::Repeat] {
            let settings=VoxelSettings{shape,side_mapping,geometryless:false};
            for axis in axes {
                for sharpness in [0.5,1.0,2.0] {
                    let graph=facing_graph(axis,sharpness);
                    graph.validate().unwrap();
                    // No geometry means no synthetic normal or slope.
                    assert!(graph.render_rgba(8,8).unwrap().chunks_exact(4)
                        .all(|p|p==[0,0,0,255]));
                    let (meta,atlas)=render_atlas(&graph,0,settings,32).unwrap();
                    for face in 0..shape.face_count() {
                        let normal=face_normal(shape,face).unwrap();
                        let expected=(axis.coverage(normal,sharpness)*255.0).round() as u8;
                        assert_eq!(center(&atlas,meta.width,shape,face,32),
                            expected,"{shape:?} {side_mapping:?} {axis:?} face={face}");
                    }
                    let text=serde_json::to_string(&graph).unwrap();
                    assert!(!text.contains("face_context"));
                    let restored:Graph=serde_json::from_str(&text).unwrap();
                    assert_eq!(atlas,render_atlas(&restored,0,settings,32).unwrap().1);
                }
            }
        }
    }
}

#[test]
fn hex_directional_sharpness_is_useful_and_cube_cardinals_remain_exact() {
    let side=face_normal(VoxelShape::HexPrism,0).unwrap();
    let soft=FaceAxis::PosX.coverage(side,0.5);
    let standard=FaceAxis::PosX.coverage(side,1.0);
    let hard=FaceAxis::PosX.coverage(side,2.0);
    assert!(soft>standard&&standard>hard&&hard>0.0);
    assert!((standard-0.5).abs()<1e-5);
    let cube=VoxelShape::Cube;
    assert_eq!(FaceAxis::PosY.coverage(face_normal(cube,0).unwrap(),2.0),1.0);
    assert_eq!(FaceAxis::PosY.coverage(face_normal(cube,2).unwrap(),0.5),0.0);
    assert_eq!(FaceAxis::PosZ.coverage(face_normal(cube,4).unwrap(),8.0),1.0);
}

#[test]
fn shared_direction_mask_drives_pbr_channels_and_3d_preview() {
    let mut graph=facing_graph(FaceAxis::PosY,1.0);
    graph.disconnect(2,0);
    let mask=graph.nodes.iter()
        .find(|n|matches!(n.kind,NodeKind::FaceFacingMask{..})).unwrap().id;
    let dark=graph.add_node(NodeKind::Color{rgb:[25,70,100]},[20.0,110.0]);
    let light=graph.add_node(NodeKind::Color{rgb:[200,140,60]},[20.0,180.0]);
    let low=graph.add_node(NodeKind::Scalar{value:0.15},[20.0,230.0]);
    let high=graph.add_node(NodeKind::Scalar{value:0.85},[20.0,290.0]);
    for channel in 0..4 {
        let mix=graph.add_node(NodeKind::MaskMix,[220.0,channel as f32*110.0]);
        let (a,b)=if channel==0 {(dark,light)} else {(low,high)};
        graph.connect(a,mix,0).unwrap();
        graph.connect(b,mix,1).unwrap();
        graph.connect(mask,mix,2).unwrap();
        graph.connect(mix,2,channel).unwrap();
    }
    graph.validate().unwrap();
    let settings=VoxelSettings::default();
    for channel in 0..4 {
        let (meta,atlas)=render_atlas(&graph,channel,settings,32).unwrap();
        assert!(center(&atlas,meta.width,VoxelShape::Cube,0,32)>
                center(&atlas,meta.width,VoxelShape::Cube,2,32),
            "positive Y must select light material for channel {channel}");
    }
    let preview=render_voxel_preview(&facing_graph(FaceAxis::PosZ,1.0),
        settings,96,0.75).unwrap();
    let reverse=render_voxel_preview(&facing_graph(FaceAxis::NegZ,1.0),
        settings,96,0.75).unwrap();
    assert_ne!(preview,reverse);
}

#[test]
fn invalid_node_params_are_rejected_and_disconnected_node_keeps_legacy_pixels() {
    let mut graph=Graph::demo_bricks();
    let settings=VoxelSettings::default();
    let expected=render_atlas(&graph,0,settings,32).unwrap().1;
    let old_preview=render_voxel_preview(&graph,settings,64,0.75).unwrap();
    let node=graph.add_node(NodeKind::FaceFacingMask{
        axis:FaceAxis::PosX,sharpness:1.0},[10.0,10.0]);
    assert_eq!(render_atlas(&graph,0,settings,32).unwrap().1,expected);
    assert_eq!(render_voxel_preview(&graph,settings,64,0.75).unwrap(),old_preview);
    graph.node_mut(node).unwrap().kind=NodeKind::FaceFacingMask{
        axis:FaceAxis::PosX,sharpness:f32::NAN};
    assert!(graph.validate().is_err());
    graph.node_mut(node).unwrap().kind=NodeKind::FaceFacingMask{
        axis:FaceAxis::PosX,sharpness:8.1};
    assert!(graph.validate().is_err());

    graph.node_mut(node).unwrap().kind=NodeKind::FaceFacingMask{
        axis:FaceAxis::PosX,sharpness:1.0};
    graph.validate().unwrap();
    for channel in 0..4 {
        // Existing material nodes without orientation inputs still take
        // the exact old atlas bake + resample path.
        let source_size=64;
        let source=graph.render_output_channel_rgba(channel,source_size,source_size)
            .unwrap();
        let old=atlas_from_pixels(&source,source_size,settings,32).unwrap().1;
        assert_eq!(render_atlas(&graph,channel,settings,32).unwrap().1,old);
    }
}
