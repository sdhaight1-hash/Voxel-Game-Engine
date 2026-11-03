use texture_studio::{Graph,NodeKind};
use texture_studio::node_definitions::SocketType;

fn scatter_graph() -> (Graph,u64) {
    let mut graph=Graph::default();
    let circle=graph.add_node(NodeKind::SdfCircle{
        center:[0.5,0.5],radius:0.32
    },[30.0,70.0]);
    let mask=graph.add_node(NodeKind::SdfToMask{edge_width:0.02},[180.0,70.0]);
    let scatter=graph.add_node(NodeKind::ShapeScatter{
        cells:10,scale:0.8,jitter:0.2,rotation:1.0,brightness:0.3,seed:55
    },[340.0,70.0]);
    assert_eq!(graph.node(scatter).unwrap().kind.output_type(),SocketType::Mask);
    assert_eq!(graph.node(scatter).unwrap().kind.input_type(0),Some(SocketType::Mask));
    assert_eq!(graph.node(scatter).unwrap().kind.input_type(1),Some(SocketType::Mask));
    assert!(graph.connect(circle,scatter,0).is_err(),"Distance needs conversion");
    assert!(graph.connect(1,scatter,0).is_err(),"Color cannot feed Mask");
    graph.connect(circle,mask,0).unwrap();
    graph.connect(mask,scatter,0).unwrap();
    graph.connect(scatter,2,0).unwrap();
    graph.connect(scatter,2,3).unwrap();
    (graph,scatter)
}

#[test]
fn scatter_roundtrip_seed_and_four_channel_memo_parity() {
    let (mut graph,scatter)=scatter_graph();
    graph.validate().unwrap();
    let before=graph.render_rgba(64,64).unwrap();
    assert!(before.chunks_exact(4).any(|p|p[0]==0));
    assert!(before.chunks_exact(4).any(|p|p[0]>0));
    let saved=serde_json::to_string(&graph).unwrap();
    let loaded:Graph=serde_json::from_str(&saved).unwrap();
    assert_eq!(before,loaded.render_rgba(64,64).unwrap());
    for size in [64,128] {
        let (channels,_)=graph.render_channels_memoized_rgba(&[0,3],size,size).unwrap();
        assert_eq!(channels[0],graph.render_rgba(size,size).unwrap());
        assert_eq!(channels[1],graph.render_output_channel_rgba(3,size,size).unwrap());
    }
    if let NodeKind::ShapeScatter{seed,..}=&mut graph.node_mut(scatter).unwrap().kind {
        *seed=100;
    }
    assert_ne!(before,graph.render_rgba(64,64).unwrap());
}

#[test]
fn scatter_density_mask_and_invalid_params() {
    let (mut graph,scatter)=scatter_graph();
    let scalar=graph.add_node(NodeKind::Scalar{value:0.0},[10.0,230.0]);
    let density=graph.add_node(NodeKind::ThresholdMask{
        threshold:0.5,softness:0.0
    },[170.0,230.0]);
    assert!(graph.connect(scalar,scatter,1).is_err(),"Scalar is not Mask");
    graph.connect(scalar,density,0).unwrap();
    graph.connect(density,scatter,1).unwrap();
    assert!(graph.render_rgba(64,64).unwrap().chunks_exact(4)
        .all(|p|p[0]==0));
    graph.node_mut(scalar).unwrap().kind=NodeKind::Scalar{value:1.0};
    assert!(graph.render_rgba(64,64).unwrap().chunks_exact(4)
        .any(|p|p[0]>0));
    if let NodeKind::ShapeScatter{cells,..}=&mut graph.node_mut(scatter).unwrap().kind {
        *cells=100;
    }
    assert!(graph.validate().is_err());
}
