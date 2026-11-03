use texture_studio::{Graph,NodeKind};
use texture_studio::node_definitions::SocketType;

#[test]
fn region_outputs_are_typed_stable_and_serializable() {
    let mut g=Graph::default();
    let value=g.add_node(NodeKind::RegionValue{cells:7,seed:12},[40.0,20.0]);
    let edge=g.add_node(NodeKind::RegionBorderMask{cells:7,seed:12,width:0.2},[40.0,130.0]);
    assert_eq!(g.node(value).unwrap().kind.output_type(),SocketType::Scalar);
    assert_eq!(g.node(edge).unwrap().kind.output_type(),SocketType::Mask);
    assert_eq!(g.node(value).unwrap().kind.type_id(),"region_value");
    assert_eq!(g.node(edge).unwrap().kind.type_id(),"region_border_mask");
    assert_eq!(g.node(value).unwrap().kind.parameters().len(),2);
    assert_eq!(g.node(edge).unwrap().kind.parameters().len(),3);
    assert!(g.connect(value,edge,0).is_err(),"regions are generators without inputs");
    let mask=g.add_node(NodeKind::ThresholdMask{threshold:0.5,softness:0.0},[200.0,20.0]);
    g.connect(value,mask,0).unwrap();
    g.connect(mask,2,0).unwrap();
    g.connect(edge,2,3).unwrap();
    g.validate().unwrap();
    let before=g.render_rgba(48,48).unwrap();
    assert!(before.chunks_exact(4).any(|px|px[0]==0));
    assert!(before.chunks_exact(4).any(|px|px[0]==255));
    let restored:Graph=serde_json::from_str(&serde_json::to_string(&g).unwrap()).unwrap();
    assert_eq!(before,restored.render_rgba(48,48).unwrap());
    let (all,_)=g.render_channels_memoized_rgba(&[0,3],48,48).unwrap();
    assert_eq!(all[0],before);
    assert_eq!(all[1],g.render_output_channel_rgba(3,48,48).unwrap());
    g.node_mut(edge).unwrap().kind=NodeKind::RegionBorderMask{cells:7,seed:12,width:f32::NAN};
    assert!(g.validate().is_err());
    g.node_mut(edge).unwrap().kind=NodeKind::RegionBorderMask{cells:33,seed:12,width:0.2};
    assert!(g.validate().is_err());
    g.node_mut(value).unwrap().kind=NodeKind::RegionValue{cells:0,seed:12};
    assert!(g.validate().is_err());
}

#[test]
fn region_value_and_borders_drive_scatter_density_and_pbr_height_layers() {
    let mut g=Graph::default();
    let value=g.add_node(NodeKind::RegionValue{cells:6,seed:44},[0.0,0.0]);
    let density=g.add_node(NodeKind::ThresholdMask{threshold:0.4,softness:0.0},[140.0,0.0]);
    let borders=g.add_node(NodeKind::RegionBorderMask{cells:6,seed:44,width:0.18},[0.0,110.0]);
    let circle=g.add_node(NodeKind::SdfCircle{center:[0.5,0.5],radius:0.30},[0.0,220.0]);
    let circle_mask=g.add_node(NodeKind::SdfToMask{edge_width:0.02},[140.0,220.0]);
    let scatter=g.add_node(NodeKind::ShapeScatter{cells:8,scale:0.8,
        jitter:0.2,rotation:1.0,brightness:0.1,seed:31},[290.0,200.0]);
    let ha=g.add_node(NodeKind::Scalar{value:0.2},[0.0,310.0]);
    let hb=g.add_node(NodeKind::Scalar{value:0.85},[0.0,370.0]);
    let weight=g.add_node(NodeKind::HeightBlendMask{influence:0.5},[420.0,80.0]);
    g.connect(value,density,0).unwrap();
    g.connect(circle,circle_mask,0).unwrap();
    g.connect(circle_mask,scatter,0).unwrap();
    g.connect(density,scatter,1).unwrap();
    // Both region borders and scatter are existing-compatible Masks; a
    // common height bias uses the region's border transition.
    g.connect(ha,weight,0).unwrap();
    g.connect(hb,weight,1).unwrap();
    g.connect(borders,weight,2).unwrap();
    let color_a=g.add_node(NodeKind::Color{rgb:[30,80,130]},[0.0,440.0]);
    let color_b=g.add_node(NodeKind::Color{rgb:[180,130,40]},[0.0,510.0]);
    for ch in 0..4 {
        let mix=g.add_node(NodeKind::MaskMix,[630.0,ch as f32*110.0]);
        let (a,b)=match ch {0=>(color_a,color_b),1|2=>(ha,hb),_=>(scatter,value)};
        g.connect(a,mix,0).unwrap();
        g.connect(b,mix,1).unwrap();
        g.connect(weight,mix,2).unwrap();
        g.connect(mix,2,ch).unwrap();
    }
    g.validate().unwrap();
    let images:Vec<_>=(0..4).map(|ch|g.render_output_channel_rgba(ch,32,32).unwrap()).collect();
    let (memo,stats)=g.render_channels_memoized_rgba(&[0,1,2,3],32,32).unwrap();
    assert_eq!(images,memo);
    assert!(stats.reused_samples>0);
    let restored:Graph=serde_json::from_str(&serde_json::to_string(&g).unwrap()).unwrap();
    assert_eq!(restored.render_rgba(32,32).unwrap(),images[0]);
    assert_eq!(restored.render_output_channel_rgba(3,32,32).unwrap(),images[3]);
    assert!(images[0].chunks_exact(4).any(|px|px[0]!=30));
}
