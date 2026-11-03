use texture_studio::{Graph,NodeKind,FaceSurface,SideMapping,VoxelShape,VoxelSettings};
use texture_studio::node_definitions::SocketType;
use texture_studio::voxel::{render_atlas,render_voxel_preview,atlas_from_pixels};

fn face_byte(pixels:&[u8],width:u32,face:usize,shape:VoxelShape,size:u32)->u8 {
    let col=face%shape.columns();
    let row=face/shape.columns();
    let x=col as u32*size+size/2;
    let y=row as u32*size+size/2;
    pixels[((y*width+x)*4) as usize]
}

fn graph_for_surface(surface:FaceSurface)->Graph {
    let mut graph=Graph::default();
    let mask=graph.add_node(NodeKind::FaceSurfaceMask{surface},[10.0,50.0]);
    assert_eq!(graph.node(mask).unwrap().kind.output_type(),SocketType::Mask);
    assert_eq!(graph.node(mask).unwrap().kind.type_id(),"face_surface_mask");
    assert_eq!(graph.node(mask).unwrap().kind.parameters().len(),1);
    graph.connect(mask,2,0).unwrap();
    graph
}

#[test]
fn surface_mask_distinguishes_side_top_bottom_on_cube_and_hex_atlases() {
    for shape in [VoxelShape::Cube,VoxelShape::HexPrism] {
        for side_mapping in [SideMapping::Wrap,SideMapping::Repeat] {
            let settings=VoxelSettings{shape,side_mapping,geometryless:false};
            for surface in [FaceSurface::Side,FaceSurface::Top,FaceSurface::Bottom] {
                let graph=graph_for_surface(surface);
                graph.validate().unwrap();
                assert!(graph.render_rgba(8,8).unwrap().chunks_exact(4)
                    .all(|p|p==[0,0,0,255]));
                let (meta,atlas)=render_atlas(&graph,0,settings,32).unwrap();
                assert_eq!(meta.faces.len(),shape.face_count());
                for face in 0..shape.face_count() {
                    assert_eq!(face_byte(&atlas,meta.width,face,shape,32),
                        if surface.matches(shape,face) {255} else {0},
                        "{shape:?} {side_mapping:?} {surface:?} face {face}");
                }
                let saved=serde_json::to_string(&graph).unwrap();
                assert!(!saved.contains("face_context"));
                let restored:Graph=serde_json::from_str(&saved).unwrap();
                assert_eq!(atlas,render_atlas(&restored,0,settings,32).unwrap().1);
            }
        }
    }
}

#[test]
fn geometry_aware_mask_drives_all_four_pbr_atlas_outputs() {
    let mut graph=graph_for_surface(FaceSurface::Top);
    graph.disconnect(2,0);
    let mask=graph.nodes.iter().find(|n|matches!(n.kind,NodeKind::FaceSurfaceMask{..}))
        .unwrap().id;
    let dark=graph.add_node(NodeKind::Color{rgb:[25,70,100]},[30.0,20.0]);
    let light=graph.add_node(NodeKind::Color{rgb:[180,120,40]},[30.0,100.0]);
    let low=graph.add_node(NodeKind::Scalar{value:0.15},[30.0,180.0]);
    let high=graph.add_node(NodeKind::Scalar{value:0.85},[30.0,260.0]);
    for channel in 0..4 {
        let mix=graph.add_node(NodeKind::MaskMix,[240.0,channel as f32*105.0]);
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
        let side=face_byte(&atlas,meta.width,0,VoxelShape::Cube,32);
        let top=face_byte(&atlas,meta.width,4,VoxelShape::Cube,32);
        let bottom=face_byte(&atlas,meta.width,5,VoxelShape::Cube,32);
        assert_eq!(bottom,side);
        assert!(top>side,"top must select higher material for PBR channel {channel}");
    }
    let (_meta,normals)=render_atlas(&graph,4,settings,32).unwrap();
    assert!(normals.chunks_exact(4).any(|p|p[3]==255));
    // Both graphs are black in context-free 2D. Only a face-aware
    // 3D renderer can make these two otherwise-identical masks differ.
    let screen=render_voxel_preview(&graph_for_surface(FaceSurface::Top),
        settings,96,0.8).unwrap();
    let other=render_voxel_preview(&graph_for_surface(FaceSurface::Bottom),
        settings,96,0.8).unwrap();
    assert_ne!(screen,other,"3D preview must consume per-face mask, not flat 2D fallback");
}

#[test]
fn legacy_atlas_matches_original_bake_resample_path() {
    let graph=Graph::demo_bricks();
    for shape in [VoxelShape::Cube,VoxelShape::HexPrism] {
        for mapping in [SideMapping::Wrap,SideMapping::Repeat] {
            let settings=VoxelSettings{shape,side_mapping:mapping,geometryless:false};
            for channel in 0..4 {
                let pixels=graph.render_output_channel_rgba(channel,64,64).unwrap();
                let expected=atlas_from_pixels(&pixels,64,settings,32).unwrap().1;
                assert_eq!(render_atlas(&graph,channel,settings,32).unwrap().1,
                    expected,"legacy atlas output changed");
            }
        }
    }
}

#[test]
fn disconnected_surface_authoring_node_does_not_switch_legacy_preview_path() {
    let mut graph=Graph::demo_bricks();
    let settings=VoxelSettings::default();
    let reference_atlas=render_atlas(&graph,0,settings,32).unwrap().1;
    let reference_preview=render_voxel_preview(&graph,settings,64,0.8).unwrap();
    graph.add_node(NodeKind::FaceSurfaceMask{surface:FaceSurface::Bottom},
        [10.0,20.0]);
    assert_eq!(render_atlas(&graph,0,settings,32).unwrap().1,
        reference_atlas,"disconnected mask must not change atlas pixels");
    assert_eq!(render_voxel_preview(&graph,settings,64,0.8).unwrap(),
        reference_preview,"disconnected mask must not change 3D pixels");
}
