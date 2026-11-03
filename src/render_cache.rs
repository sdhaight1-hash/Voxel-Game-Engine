//! Explicit, bounded caching of complete graph RGBA renders.
//!
//! This is deliberately an *output* cache, not rasterization of intermediate
//! analytic nodes. In particular, LibraryNode coordinate warps must continue
//! evaluating upstream inputs at their transformed UVs in Graph::sample.
//! Once measured, downstream/analytic-node caching can be added separately.
use crate::{Graph, NodeKind};
use std::collections::{HashSet, VecDeque};
use std::sync::Arc;

/// Schema/bake algorithm version. Bump if pixel evaluation semantics change.
const BAKE_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RenderTarget {
    Channel(usize),
    Normal,
    Node(u64),
}

/// The graph's visual positions, unrelated graph branches and voxel-display
/// options do not affect these UV-domain RGBA renders. Only the target's
/// transitive upstream dependency closure participates in the bake identity.
#[derive(Clone, Debug, PartialEq)]
struct RenderKey {
    bake_version: u32,
    width: u32,
    height: u32,
    target: RenderTarget,
    nodes: Vec<(u64,NodeKind)>,
    links: Vec<(u64,u64,usize)>,
    /// Transitive input images only; graph validation verifies fingerprints.
    images: Vec<(u64,u64)>,
}

impl RenderKey {
    fn new(graph:&Graph,width:u32,height:u32,target:RenderTarget)->Self {
        let root=match target {
            RenderTarget::Node(id)=>Some(id),
            RenderTarget::Normal|RenderTarget::Channel(_) => {
                let channel=match target {
                    RenderTarget::Normal=>3,
                    RenderTarget::Channel(index)=>index,
                    RenderTarget::Node(_)=>unreachable!(),
                };
                let output=graph.nodes.iter().find(|n|
                    matches!(n.kind,NodeKind::Output)).expect("validated graph").id;
                graph.links.iter().find(|link|
                    link.to==output && link.input==channel).map(|link|link.from)
            }
        };
        let mut required=HashSet::new();
        let mut pending:Vec<u64>=root.into_iter().collect();
        while let Some(id)=pending.pop() {
            if required.insert(id) {
                pending.extend(graph.links.iter().filter(|link|
                    link.to==id).map(|link|link.from));
            }
        }
        Self {
            bake_version:BAKE_VERSION,width,height,target,
            nodes:graph.nodes.iter().filter(|n|required.contains(&n.id))
                .map(|n|(n.id,n.kind)).collect(),
            links:graph.links.iter().filter(|link|
                required.contains(&link.to) && required.contains(&link.from))
                .map(|l|(l.from,l.to,l.input)).collect(),
            images:graph.images.iter().filter(|image|
                graph.nodes.iter().any(|node|required.contains(&node.id)
                    &&node.kind.bitmap_asset_id()==Some(image.id)))
                .map(|image|(image.id,image.fingerprint)).collect(),
        }
    }
}

struct Entry {
    key: RenderKey,
    pixels: Arc<[u8]>,
}

/// Cache hits reflect output RGBA reuse. Peak/resident bytes count storage held
/// by this cache, not external Arc clones or wgpu/egui texture allocations.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct RenderCacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
    pub entries: usize,
    pub resident_bytes: usize,
    pub peak_resident_bytes: usize,
}

pub struct GraphRenderCache {
    limit_bytes: usize,
    entries: VecDeque<Entry>,
    stats: RenderCacheStats,
}

impl GraphRenderCache {
    /// A zero byte limit disables retained images without changing evaluation.
    pub fn new(limit_bytes:usize)->Self {
        Self{limit_bytes,entries:VecDeque::new(),stats:RenderCacheStats::default()}
    }

    pub fn stats(&self)->RenderCacheStats {self.stats}

    pub fn render_channel(&mut self,graph:&Graph,channel:usize,width:u32,height:u32)
        ->Result<Arc<[u8]>,String> {
        self.render(graph,width,height,RenderTarget::Channel(channel))
    }

    pub fn render_node(&mut self,graph:&Graph,id:u64,width:u32,height:u32)
        ->Result<Arc<[u8]>,String> {
        self.render(graph,width,height,RenderTarget::Node(id))
    }

    pub fn render_normal(&mut self,graph:&Graph,width:u32,height:u32)
        ->Result<Arc<[u8]>,String> {
        self.render(graph,width,height,RenderTarget::Normal)
    }

    fn render(&mut self,graph:&Graph,width:u32,height:u32,target:RenderTarget)
        ->Result<Arc<[u8]>,String> {
        // Validation precedes lookup: an invalid project must not recover a
        // previously valid image because only node positions changed.
        graph.validate()?;
        if width==0 || height==0 || width>4096 || height>4096 {
            return Err("Image dimensions must be 1–4096".into());
        }
        match target {
            RenderTarget::Channel(channel) if channel>=4 =>
                return Err("Unsupported material channel".into()),
            RenderTarget::Node(id) if graph.node(id).is_none() =>
                return Err("Node not found".into()),
            _=>{}
        }
        let key=RenderKey::new(graph,width,height,target);
        if let Some(index)=self.entries.iter().position(|entry|entry.key==key) {
            let entry=self.entries.remove(index).expect("indexed entry");
            let result=entry.pixels.clone();
            self.entries.push_front(entry);
            self.stats.hits+=1;
            return Ok(result);
        }
        self.stats.misses+=1;
        let pixels=match target {
            RenderTarget::Channel(channel) =>
                graph.render_output_channel_rgba(channel,width,height)?,
            RenderTarget::Node(id) => graph.render_node_rgba(id,width,height)?,
            RenderTarget::Normal => graph.render_normal_rgba(width,height)?,
        };
        let result:Arc<[u8]>=pixels.into();
        if result.len()<=self.limit_bytes {
            while self.stats.resident_bytes>self.limit_bytes-result.len() {
                let old=self.entries.pop_back().expect("over cap implies occupied cache");
                self.stats.resident_bytes-=old.pixels.len();
                self.stats.evictions+=1;
            }
            self.stats.resident_bytes+=result.len();
            self.stats.peak_resident_bytes=self.stats.peak_resident_bytes
                .max(self.stats.resident_bytes);
            self.entries.push_front(Entry{key,pixels:result.clone()});
            self.stats.entries=self.entries.len();
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{LibraryNode, NodeKind};

    #[test]
    fn repeated_render_hits_and_positions_do_not_affect_recipe() {
        let graph=Graph::demo_bricks();
        let reference=graph.render_rgba(16,16).unwrap();
        let mut cache=GraphRenderCache::new(16*16*4*2);
        let cold=cache.render_channel(&graph,0,16,16).unwrap();
        assert_eq!(&*cold,reference.as_slice());
        let warm=cache.render_channel(&graph,0,16,16).unwrap();
        assert!(Arc::ptr_eq(&cold,&warm));
        let mut relocated=graph.clone();
        relocated.node_mut(1).unwrap().position=[900.0,-900.0];
        assert!(Arc::ptr_eq(&cold,
            &cache.render_channel(&relocated,0,16,16).unwrap()));
        assert_eq!(cache.stats().hits,2);
        assert_eq!(cache.stats().misses,1);
        assert_eq!(cache.stats().resident_bytes,1024);
    }

    #[test]
    fn editing_an_unrelated_branch_keeps_its_other_output_warm() {
        let mut graph=Graph::demo_bricks();
        // Noise #3 feeds Roughness, whereas Bricks->Colorize feeds Albedo.
        let mut cache=GraphRenderCache::new(16*16*4*3);
        let albedo=cache.render_channel(&graph,0,16,16).unwrap();
        let roughness=cache.render_channel(&graph,1,16,16).unwrap();
        graph.node_mut(3).unwrap().kind=NodeKind::Noise{
            scale:14,octaves:4,seed:27,
        };
        let albedo_again=cache.render_channel(&graph,0,16,16).unwrap();
        assert!(Arc::ptr_eq(&albedo,&albedo_again));
        let roughness_again=cache.render_channel(&graph,1,16,16).unwrap();
        assert!(!Arc::ptr_eq(&roughness,&roughness_again));
        assert_ne!(roughness.as_ref(),roughness_again.as_ref());
        assert_eq!(cache.stats().hits,1);
        assert_eq!(cache.stats().misses,3);
    }

    #[test]
    fn changes_to_parameters_links_channels_and_resolution_invalidate() {
        let mut graph=Graph::default();
        let mut cache=GraphRenderCache::new(2048);
        let before=cache.render_channel(&graph,0,8,8).unwrap();
        graph.node_mut(1).unwrap().kind=NodeKind::Noise{
            scale:7,octaves:4,seed:42
        };
        let altered=cache.render_channel(&graph,0,8,8).unwrap();
        assert_ne!(&*before,&*altered);
        graph.disconnect(2,0);
        assert_ne!(&*altered,&*cache.render_channel(&graph,0,8,8).unwrap());
        let roughness=cache.render_channel(&graph,1,8,8).unwrap();
        let normal=cache.render_normal(&graph,8,8).unwrap();
        assert_ne!(&*roughness,&*normal);
        let larger=cache.render_channel(&graph,1,16,16).unwrap();
        assert_eq!(larger.len(),4*16*16);
        assert_eq!(cache.stats().hits,0);
        assert_eq!(cache.stats().misses,6);
    }

    #[test]
    fn bounded_lru_eviction_and_oversize_bypass() {
        let graph=Graph::demo_bricks();
        let mut cache=GraphRenderCache::new(8*8*4);
        let a=cache.render_channel(&graph,0,8,8).unwrap();
        assert_eq!(cache.stats().resident_bytes,256);
        cache.render_channel(&graph,1,8,8).unwrap();
        assert_eq!(cache.stats().evictions,1);
        assert_eq!(cache.stats().entries,1);
        assert_eq!(cache.stats().resident_bytes,256);
        // The caller's external Arc remains readable after eviction.
        assert_eq!(a.as_ref(),graph.render_rgba(8,8).unwrap().as_slice());
        cache.render_channel(&graph,0,16,16).unwrap();
        assert_eq!(cache.stats().entries,1);
        assert_eq!(cache.stats().resident_bytes,256);
        assert!(cache.stats().peak_resident_bytes<=256);
    }

    #[test]
    fn warped_uv_sampling_is_byte_identical_cold_and_warm() {
        let mut graph=Graph::default();
        let warp=graph.add_node(NodeKind::Library{
            operation:LibraryNode::Warp,amount:0.4,seed:41,
        },[120.0,10.0]);
        graph.connect(1,warp,0).unwrap();
        graph.connect(warp,2,0).unwrap();
        let expected=graph.render_rgba(64,64).unwrap();
        let mut cache=GraphRenderCache::new(64*64*4);
        assert_eq!(&*cache.render_channel(&graph,0,64,64).unwrap(),
            expected.as_slice());
        assert_eq!(&*cache.render_channel(&graph,0,64,64).unwrap(),
            expected.as_slice());
        assert_eq!(cache.stats().hits,1);
        assert_eq!(cache.stats().misses,1);
    }

    #[test]
    fn invalid_inputs_are_rejected_even_if_key_would_hit() {
        let mut graph=Graph::default();
        let mut cache=GraphRenderCache::new(4096);
        cache.render_channel(&graph,0,8,8).unwrap();
        assert!(cache.render_channel(&graph,9,8,8).is_err());
        assert!(cache.render_channel(&graph,0,0,8).is_err());
        assert!(cache.render_node(&graph,999,8,8).is_err());
        graph.node_mut(1).unwrap().position=[f32::NAN,0.0];
        assert!(cache.render_channel(&graph,0,8,8).is_err());
        assert_eq!(cache.stats().hits,0);
    }

    #[test]
    fn cache_distinguishes_top_and_side_face_context() {
        use crate::{FaceSurface,VoxelShape};
        let mut graph=Graph::default();
        let surface=graph.add_node(NodeKind::FaceSurfaceMask{
            surface:FaceSurface::Top},[30.0,60.0]);
        graph.connect(surface,2,0).unwrap();
        let top=graph.for_voxel_face(VoxelShape::Cube,4);
        let side=graph.for_voxel_face(VoxelShape::Cube,0);
        let mut cache=GraphRenderCache::new(1024*1024);
        let first=cache.render_channel(&side,0,8,8).unwrap();
        let second=cache.render_channel(&top,0,8,8).unwrap();
        assert_ne!(first.as_ref(),second.as_ref());
        assert!(Arc::ptr_eq(&first,&cache.render_channel(&side,0,8,8).unwrap()));
        assert_eq!(cache.stats().misses,2);
        assert_eq!(cache.stats().hits,1);
        assert!(!serde_json::to_string(&top).unwrap().contains("face_context"));
        let after:Graph=serde_json::from_str(&serde_json::to_string(&top).unwrap())
            .unwrap();
        assert!(after.render_rgba(8,8).unwrap().chunks_exact(4)
            .all(|pixel|pixel[0]==0));
    }

}
