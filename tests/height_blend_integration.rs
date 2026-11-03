use texture_studio::{Graph,NodeKind};
use texture_studio::node_definitions::SocketType;

/// Every channel uses the *same* height-conditioned mask, rather than
/// computing a potentially different blend weight for each PBR map.
fn layered_graph()->(Graph,u64,u64) {
    let mut g=Graph::default(); // legacy Noise=1 and Output=2 remain intact
    let mask_value=g.add_node(NodeKind::Scalar{value:0.5},[0.0,0.0]);
    let mask=g.add_node(NodeKind::ThresholdMask{threshold:0.5,softness:0.3},[50.0,0.0]);
    let ha=g.add_node(NodeKind::Scalar{value:0.15},[0.0,60.0]);
    let hb=g.add_node(NodeKind::Luminance,[0.0,120.0]);
    let factor=g.add_node(NodeKind::HeightBlendMask{influence:0.4},[150.0,30.0]);
    g.connect(mask_value,mask,0).unwrap();
    g.connect(1,hb,0).unwrap();
    assert!(g.connect(1,factor,0).is_err(),"Color -> Scalar requires Luminance");
    assert!(g.connect(ha,factor,2).is_err(),"Scalar -> Mask must be explicit");
    g.connect(ha,factor,0).unwrap();
    g.connect(hb,factor,1).unwrap();
    g.connect(mask,factor,2).unwrap();
    assert_eq!(g.node(factor).unwrap().kind.output_type(),SocketType::Mask);
    assert!(g.connect(factor,2,0).is_ok(),"Mask may drive old color outputs");
    let a=g.add_node(NodeKind::Color{rgb:[36,73,124]},[0.0,200.0]);
    let b=g.add_node(NodeKind::Color{rgb:[179,116,55]},[0.0,280.0]);
    let ra=g.add_node(NodeKind::Scalar{value:0.2},[0.0,340.0]);
    let rb=g.add_node(NodeKind::Scalar{value:0.8},[0.0,400.0]);
    let ma=g.add_node(NodeKind::Scalar{value:0.03},[0.0,460.0]);
    let mb=g.add_node(NodeKind::Scalar{value:0.25},[0.0,520.0]);
    for (channel,source_a,source_b) in [
        (0,a,b),(1,ra,rb),(2,ma,mb),(3,ha,hb)
    ] {
        let mix=g.add_node(NodeKind::MaskMix,[300.0,channel as f32*110.0]);
        g.connect(source_a,mix,0).unwrap();
        g.connect(source_b,mix,1).unwrap();
        g.connect(factor,mix,2).unwrap();
        g.connect(mix,2,channel).unwrap();
    }
    (g,factor,mask_value)
}

#[test]
fn layers_share_one_weight_over_all_four_pbr_outputs_and_roundtrip() {
    let (g,_,_)=layered_graph();
    g.validate().unwrap();
    assert_eq!(g.node(2).unwrap().kind.input_count(),4);
    for size in [64,128] {
        let (rendered,stats)=g.render_channels_memoized_rgba(&[0,1,2,3],size,size).unwrap();
        assert!(stats.reused_samples>0,"shared height mask should memoize");
        for channel in 0..4 {
            assert_eq!(rendered[channel],g.render_output_channel_rgba(channel,size,size).unwrap());
        }
        assert!(rendered[0].chunks_exact(4).any(|p|p[0]>36));
        let restored:Graph=serde_json::from_str(&serde_json::to_string(&g).unwrap()).unwrap();
        assert_eq!(rendered[3],restored.render_output_channel_rgba(3,size,size).unwrap());
        assert_eq!(rendered[0],restored.render_rgba(size,size).unwrap());
    }
}

#[test]
fn height_influence_changes_only_new_layer_outputs_not_existing_node_bytes() {
    let (mut g,factor,_) =layered_graph();
    let before=g.render_rgba(64,64).unwrap();
    let legacy=g.render_node_rgba(1,64,64).unwrap();
    g.node_mut(factor).unwrap().kind=NodeKind::HeightBlendMask{influence:0.0};
    g.validate().unwrap();
    assert_ne!(before,g.render_rgba(64,64).unwrap());
    assert_eq!(legacy,g.render_node_rgba(1,64,64).unwrap());
    g.node_mut(factor).unwrap().kind=NodeKind::HeightBlendMask{influence:f32::NAN};
    assert!(g.validate().is_err());
}

#[test]
fn typed_mask_mix_rejects_ambiguous_mask_and_does_not_change_old_blend() {
    let mut g=Graph::default();
    let mix=g.add_node(NodeKind::MaskMix,[70.0,70.0]);
    assert!(g.connect(1,mix,2).is_err());
    let scalar=g.add_node(NodeKind::Scalar{value:0.5},[0.0,0.0]);
    assert!(g.connect(scalar,mix,2).is_err());
    let mask=g.add_node(NodeKind::ThresholdMask{threshold:0.5,softness:0.0},[0.0,30.0]);
    g.connect(scalar,mask,0).unwrap();
    g.connect(mask,mix,2).unwrap();
    g.connect(1,mix,0).unwrap();
    let red=g.add_node(NodeKind::Color{rgb:[255,0,0]},[20.0,30.0]);
    g.connect(red,mix,1).unwrap();
    g.connect(mix,2,0).unwrap();
    let restored:Graph=serde_json::from_str(&serde_json::to_string(&g).unwrap()).unwrap();
    assert_eq!(g.render_rgba(32,32).unwrap(),restored.render_rgba(32,32).unwrap());
    assert_eq!(g.node(mix).unwrap().kind.type_id(),"mask_mix");
}
