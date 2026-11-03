use texture_studio::{Graph,NodeKind};
use texture_studio::node_definitions::SocketType;
use texture_studio::render_cache::GraphRenderCache;
use std::sync::Arc;

#[test]
fn bitmap_color_and_alpha_are_typed_portable_and_memoized() {
    let mut g=Graph::default();
    assert!(!serde_json::to_string(&g).unwrap().contains("\"images\""));
    let image=g.add_bitmap_rgba("leaf.png",2,2,vec![
        255,0,0,0, 0,255,0,255,
        0,0,255,128, 255,255,255,255]).unwrap();
    let color=g.add_node(NodeKind::BitmapColor{asset_id:image,bilinear:false},[20.0,20.0]);
    let alpha=g.add_node(NodeKind::BitmapAlpha{asset_id:image,bilinear:false},[20.0,150.0]);
    assert_eq!(g.node(color).unwrap().kind.output_type(),SocketType::Color);
    assert_eq!(g.node(alpha).unwrap().kind.output_type(),SocketType::Mask);
    assert_eq!(g.node(color).unwrap().kind.type_id(),"bitmap_color");
    assert_eq!(g.node(alpha).unwrap().kind.type_id(),"bitmap_alpha");
    let mix=g.add_node(NodeKind::MaskMix,[250.0,80.0]);
    g.connect(1,mix,0).unwrap();
    g.connect(color,mix,1).unwrap();
    g.connect(alpha,mix,2).unwrap();
    assert!(g.connect(color,mix,2).is_err(),"Color cannot feed Mask");
    for ch in 0..4 {g.connect(mix,2,ch).unwrap();}
    g.validate().unwrap();
    let saved=serde_json::to_string(&g).unwrap();
    let restored:Graph=serde_json::from_str(&saved).unwrap();
    assert_eq!(g.render_rgba(32,32).unwrap(),restored.render_rgba(32,32).unwrap());
    let (memo,stats)=g.render_channels_memoized_rgba(&[0,1,2,3],32,32).unwrap();
    assert!(stats.reused_samples>0);
    for ch in 0..4 {
        assert_eq!(memo[ch],g.render_output_channel_rgba(ch,32,32).unwrap());
    }
}

#[test]
fn image_edits_invalidate_dependent_preview_but_not_unrelated_outputs() {
    let mut g=Graph::demo_bricks();
    let id=g.add_bitmap_rgba("bark.png",1,1,vec![100,30,10,255]).unwrap();
    let source=g.add_node(NodeKind::BitmapColor{asset_id:id,bilinear:false},[20.0,20.0]);
    g.connect(source,4,0).unwrap();
    let mut cache=GraphRenderCache::new(1024*1024);
    let first=cache.render_channel(&g,0,16,16).unwrap();
    let rough=cache.render_channel(&g,1,16,16).unwrap();
    assert!(Arc::ptr_eq(&first,&cache.render_channel(&g,0,16,16).unwrap()));
    g.replace_bitmap_rgba(id,vec![240,30,10,255]).unwrap();
    let changed=cache.render_channel(&g,0,16,16).unwrap();
    assert!(!Arc::ptr_eq(&first,&changed));
    assert_ne!(first.as_ref(),changed.as_ref());
    assert!(Arc::ptr_eq(&rough,&cache.render_channel(&g,1,16,16).unwrap()));
    g.images[0].rgba[0]=0;
    assert!(g.validate().is_err());
    assert!(cache.render_channel(&g,0,16,16).is_err());
}

#[test]
fn missing_assets_and_corrupted_json_fail_validation() {
    let mut g=Graph::default();
    let node=g.add_node(NodeKind::BitmapAlpha{asset_id:777,bilinear:true},[0.0,0.0]);
    assert!(g.validate().is_err());
    g.node_mut(node).unwrap().kind=NodeKind::BitmapAlpha{asset_id:0,bilinear:true};
    g.connect(node,2,0).unwrap();
    g.validate().unwrap(); // unassigned editor placeholder
    assert!(g.render_rgba(4,4).unwrap().chunks_exact(4).all(|p|p[0]==0));
    let id=g.add_bitmap_rgba("a.png",1,1,vec![1,2,3,4]).unwrap();
    assert_eq!(g.add_bitmap_rgba("same.png",1,1,vec![1,2,3,4]).unwrap(),id);
    g.node_mut(node).unwrap().kind=NodeKind::BitmapAlpha{asset_id:id,bilinear:true};
    let mut roundtrip:Graph=serde_json::from_str(&serde_json::to_string(&g).unwrap()).unwrap();
    roundtrip.images[0].id=99;
    assert!(roundtrip.validate().is_err());
    roundtrip.images[0].id=id;
    roundtrip.images[0].rgba.pop();
    assert!(roundtrip.validate().is_err());
}

#[test]
fn unused_images_can_be_explicitly_pruned_and_reimported() {
    let mut graph=Graph::default();
    let keep=graph.add_bitmap_rgba("keep.png",1,1,vec![1,2,3,255]).unwrap();
    let drop_id=graph.add_bitmap_rgba("drop.png",1,1,vec![4,5,6,255]).unwrap();
    let node=graph.add_node(NodeKind::BitmapColor{asset_id:keep,bilinear:true},[0.0,0.0]);
    assert_eq!(graph.prune_unused_bitmaps(),1);
    assert_eq!(graph.images.len(),1);
    assert_eq!(graph.images[0].id,keep);
    graph.validate().unwrap();
    graph.node_mut(node).unwrap().kind=NodeKind::BitmapColor{asset_id:0,bilinear:true};
    assert_eq!(graph.prune_unused_bitmaps(),1);
    assert!(graph.images.is_empty());
    let new_id=graph.add_bitmap_rgba("new.png",1,1,vec![7,8,9,255]).unwrap();
    assert!(new_id==keep||new_id==drop_id);
    graph.validate().unwrap();
}
