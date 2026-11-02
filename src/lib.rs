//! CPU-based, deterministic, tileable procedural texture engine.
//! No UI types are used here, so the graph and renderer can be tested independently.

use serde::{Deserialize, Serialize};
use std::{cell::{Cell,RefCell}, collections::{HashMap, HashSet}};

pub mod voxel;
pub mod foliage;
pub mod tree_growth_rules;
pub mod plant_climate;
pub mod flora_prefab;
pub mod hexel_clip;
pub mod asset;
pub mod foliage_materials;
pub mod presets;
pub mod node_library;
pub mod node_definitions;
pub mod render_cache;
pub mod foliage_bake_cache;
pub use node_library::LibraryNode;
use node_definitions::{PortDefinition, SocketType};
pub use voxel::{VoxelSettings, VoxelShape, SideMapping};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Graph {
    pub nodes: Vec<Node>,
    pub links: Vec<Link>,
    /// Additive setting: old JSON projects without voxel information still load.
    #[serde(default)]
    pub voxel: VoxelSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub id: u64,
    pub position: [f32; 2],
    pub kind: NodeKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Link {
    pub from: u64,
    pub to: u64,
    pub input: usize,
}

/// Per-render CPU evaluation counters (not GPU timings). Samples are cached
/// only within one output pixel, and keyed by exact float UV bit patterns.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MultiChannelStats {
    pub reused_samples: u64,
    pub evaluated_samples: u64,
    pub peak_entries_per_pixel: usize,
}

// A small linear buffer is cheaper than hashing each inexpensive RGB node.
// On overflow evaluation continues normally without storing new samples.
const MAX_PIXEL_SAMPLE_ENTRIES: usize = 64;

struct SampleMemo {
    shared_nodes: Vec<u64>,
    values: RefCell<Vec<((u64,u32,u32),[f32;3])>>,
    hits: Cell<u64>,
    evaluations: Cell<u64>,
    peak: Cell<usize>,
}

impl SampleMemo {
    fn new(shared_nodes:Vec<u64>)->Self {
        Self{shared_nodes,values:RefCell::new(Vec::new()),
            hits:Cell::new(0),evaluations:Cell::new(0),peak:Cell::new(0)}
    }

    fn lookup(&self,id:u64,u:f32,v:f32)->Option<[f32;3]> {
        if !self.shared_nodes.contains(&id) {return None;}
        let key=(id,u.to_bits(),v.to_bits());
        let result=self.values.borrow().iter().find(|(k,_)|*k==key)
            .map(|(_,rgb)|*rgb);
        if result.is_some() {self.hits.set(self.hits.get()+1);}
        result
    }

    fn record(&self,id:u64,u:f32,v:f32,value:[f32;3]) {
        self.evaluations.set(self.evaluations.get()+1);
        if !self.shared_nodes.contains(&id) {return;}
        let mut values=self.values.borrow_mut();
        if values.len()<MAX_PIXEL_SAMPLE_ENTRIES {
            values.push(((id,u.to_bits(),v.to_bits()),value));
            self.peak.set(self.peak.get().max(values.len()));
        }
    }

    fn stats(&self)->MultiChannelStats {
        MultiChannelStats{reused_samples:self.hits.get(),
            evaluated_samples:self.evaluations.get(),
            peak_entries_per_pixel:self.peak.get()}
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum NodeKind {
    Noise { scale: u32, octaves: u8, seed: u32 },
    Checker { tiles: u32 },
    Color { rgb: [u8; 3] },
    /// Typed scalar source; automatically broadcasts into legacy RGB inputs.
    Scalar { value: f32 },
    /// Explicit conversion from RGB to a luminance scalar.
    Luminance,
    /// Scalar-only, three-parameter levels remap. Does not mutate old node kinds.
    Levels { black: f32, white: f32, gamma: f32 },
    /// Explicit scalar-to-mask conversion with optional smooth edge coverage.
    ThresholdMask { threshold: f32, softness: f32 },
    Invert,
    Blend { factor: f32 },
    Bricks { columns: u32, rows: u32, mortar: f32 },
    Voronoi { cells: u32, seed: u32 },
    Strata { bands: u32, warp: f32, seed: u32 },
    Library { operation: LibraryNode, amount: f32, seed: u32 },
    Colorize { dark: [u8; 3], light: [u8; 3] },
    Output,
}

impl NodeKind {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Noise { .. } => "Noise",
            Self::Checker { .. } => "Checker",
            Self::Color { .. } => "Color",
            Self::Scalar { .. } => "Scalar",
            Self::Luminance => "Luminance",
            Self::Levels { .. } => "Levels",
            Self::ThresholdMask { .. } => "Threshold Mask",
            Self::Invert => "Invert",
            Self::Blend { .. } => "Blend",
            Self::Bricks { .. } => "Bricks",
            Self::Voronoi { .. } => "Voronoi",
            Self::Strata { .. } => "Strata",
            Self::Library { operation, .. } => operation.name(),
            Self::Colorize { .. } => "Colorize",
            Self::Output => "Material Output",
        }
    }

    pub fn input_count(&self) -> usize {
        match self {
            Self::Noise { .. } | Self::Checker { .. } | Self::Color { .. }
                | Self::Scalar { .. } | Self::Bricks { .. } | Self::Voronoi { .. }
                | Self::Strata { .. } => 0,
            Self::Invert | Self::Colorize { .. } | Self::Luminance
                | Self::Levels { .. } | Self::ThresholdMask { .. } => 1,
            Self::Blend { .. } => 2,
            Self::Library { operation, .. } => operation.input_count(),
            Self::Output => 4,
        }
    }

    pub fn input_name(&self, index: usize) -> &'static str {
        match (self, index) {
            (Self::Blend { .. }, 0) => "A",
            (Self::Blend { .. }, 1) => "B",
            (Self::Output, 0) => "Albedo",
            (Self::Output, 1) => "Roughness",
            (Self::Output, 2) => "Metallic",
            (Self::Output, 3) => "Height",
            (Self::Levels { .. } | Self::ThresholdMask { .. }, 0) => "Scalar",
            (Self::Luminance,0) => "Color",
            (Self::Invert | Self::Colorize { .. }, 0) => "Image",
            (Self::Library { operation, .. }, i) => operation.input_name(i),
            _ => "",
        }
    }

    /// Stable node-kind ID for new authoring metadata. Existing serde tags and
    /// numeric graph socket indices are unchanged.
    pub fn type_id(&self) -> String {
        match self {
            Self::Noise{..}=>"noise".into(),
            Self::Checker{..}=>"checker".into(),
            Self::Color{..}=>"color".into(),
            Self::Scalar{..}=>"scalar".into(),
            Self::Luminance=>"luminance".into(),
            Self::Levels{..}=>"levels".into(),
            Self::ThresholdMask{..}=>"threshold_mask".into(),
            Self::Invert=>"invert".into(),
            Self::Blend{..}=>"blend".into(),
            Self::Bricks{..}=>"bricks".into(),
            Self::Voronoi{..}=>"voronoi".into(),
            Self::Strata{..}=>"strata".into(),
            Self::Colorize{..}=>"colorize".into(),
            Self::Library{operation,..}=>format!("library.{operation:?}"),
            Self::Output=>"material_output".into(),
        }
    }

    pub fn output_type(&self) -> SocketType {
        match self {
            Self::Scalar{..}|Self::Luminance|Self::Levels{..}=>SocketType::Scalar,
            Self::ThresholdMask{..}=>SocketType::Mask,
            _=>SocketType::Color, // Existing RGB evaluation semantics.
        }
    }

    pub fn input_type(&self, index:usize) -> Option<SocketType> {
        if index>=self.input_count() {return None;}
        Some(if matches!(self,Self::Levels{..}|Self::ThresholdMask{..}) {
            SocketType::Scalar
        } else {
            SocketType::Color
        })
    }

    pub fn input_definition(&self,index:usize)->Option<PortDefinition> {
        const IDS:[&str;4]=["input_0","input_1","input_2","input_3"];
        self.input_type(index).map(|data_type|PortDefinition {
            id:IDS[index],label:self.input_name(index),data_type,
        })
    }

    pub fn output_definition(&self)->Option<PortDefinition> {
        if matches!(self,Self::Output) {return None;}
        Some(PortDefinition{id:"output",label:"Output",data_type:self.output_type()})
    }

    pub fn parameters(&self)->&'static [node_definitions::ParameterDefinition] {
        node_definitions::parameters(self)
    }
}

impl Default for Graph {
    fn default() -> Self {
        Self {
            nodes: vec![
                Node { id: 1, position: [60.0, 125.0], kind: NodeKind::Noise { scale: 6, octaves: 4, seed: 42 } },
                Node { id: 2, position: [445.0, 125.0], kind: NodeKind::Output },
            ],
            links: vec![Link { from: 1, to: 2, input: 0 }],
            voxel: VoxelSettings::default(),
        }
    }
}

impl Graph {
    /// A ready-to-edit PBR bricks example, independent of old project saves.
    pub fn demo_bricks() -> Self {
        Self {
            nodes: vec![
                Node { id: 1, position: [50.0, 100.0], kind: NodeKind::Bricks { columns: 8, rows: 12, mortar: 0.13 } },
                Node { id: 2, position: [290.0, 80.0], kind: NodeKind::Colorize { dark: [38, 34, 42], light: [190, 102, 59] } },
                Node { id: 3, position: [275.0, 365.0], kind: NodeKind::Noise { scale: 9, octaves: 4, seed: 27 } },
                Node { id: 4, position: [585.0, 105.0], kind: NodeKind::Output },
            ],
            links: vec![
                Link { from: 1, to: 2, input: 0 },
                Link { from: 2, to: 4, input: 0 },
                Link { from: 3, to: 4, input: 1 },
                Link { from: 1, to: 4, input: 3 },
            ],
            voxel: VoxelSettings::default(),
        }
    }

    pub fn add_node(&mut self, kind: NodeKind, position: [f32; 2]) -> u64 {
        let id = self.nodes.iter().map(|node| node.id).max().unwrap_or(0) + 1;
        self.nodes.push(Node { id, position, kind });
        id
    }

    pub fn remove_node(&mut self, id: u64) -> Result<(), String> {
        let node = self.node(id).ok_or("Node not found")?;
        if matches!(node.kind, NodeKind::Output) {
            return Err("The output node cannot be deleted".to_string());
        }
        self.nodes.retain(|n| n.id != id);
        self.links.retain(|l| l.from != id && l.to != id);
        Ok(())
    }

    pub fn node(&self, id: u64) -> Option<&Node> {
        self.nodes.iter().find(|n| n.id == id)
    }

    pub fn node_mut(&mut self, id: u64) -> Option<&mut Node> {
        self.nodes.iter_mut().find(|n| n.id == id)
    }

    pub fn connect(&mut self, from: u64, to: u64, input: usize) -> Result<(), String> {
        let source = self.node(from).ok_or("Source node not found")?;
        let target = self.node(to).ok_or("Destination node not found")?;
        if matches!(source.kind, NodeKind::Output) {
            return Err("Output nodes cannot feed other nodes".to_string());
        }
        let Some(expected)=target.kind.input_type(input) else {
            return Err("That input socket does not exist".to_string());
        };
        let actual=source.kind.output_type();
        if !actual.can_feed(expected) {
            return Err(format!("Cannot connect {actual:?} to {expected:?} on {}",
                target.kind.input_name(input)));
        }
        if from == to || self.reaches(to, from) {
            return Err("Connection would create a cycle".to_string());
        }
        // One cable per input: replacing a connection is intentional.
        self.links.retain(|l| l.to != to || l.input != input);
        self.links.push(Link { from, to, input });
        Ok(())
    }

    pub fn disconnect(&mut self, to: u64, input: usize) {
        self.links.retain(|l| l.to != to || l.input != input);
    }

    /// Move either end of an existing cable as a single, validated operation.
    /// If the new connection would create a cycle, preserve the original cable.
    pub fn rewire(
        &mut self, old_to: u64, old_input: usize,
        new_from: u64, new_to: u64, new_input: usize,
    ) -> Result<(), String> {
        if !self.links.iter().any(|l| l.to == old_to && l.input == old_input) {
            return Err("Original connection not found".into());
        }
        let mut proposed = self.clone();
        proposed.disconnect(old_to, old_input);
        proposed.connect(new_from, new_to, new_input)?;
        *self = proposed;
        Ok(())
    }

    fn reaches(&self, start: u64, goal: u64) -> bool {
        let mut stack = vec![start];
        let mut seen = HashSet::new();
        while let Some(id) = stack.pop() {
            if id == goal {
                return true;
            }
            if seen.insert(id) {
                stack.extend(self.links.iter().filter(|l| l.from == id).map(|l| l.to));
            }
        }
        false
    }

    pub fn validate(&self) -> Result<(), String> {
        let ids: HashSet<_> = self.nodes.iter().map(|n| n.id).collect();
        if ids.len() != self.nodes.len() {
            return Err("Duplicate node IDs in project".into());
        }
        if self.nodes.iter().filter(|n| matches!(n.kind, NodeKind::Output)).count() != 1 {
            return Err("A project needs exactly one Output node".into());
        }
        let mut ports = HashSet::new();
        for node in &self.nodes {
            if !node.position.iter().all(|p| p.is_finite()) {
                return Err("Invalid node position".into());
            }
            match node.kind {
                NodeKind::Noise { scale, octaves, .. } if !(1..=64).contains(&scale) || !(1..=6).contains(&octaves) => return Err("Noise settings out of range".into()),
                NodeKind::Checker { tiles } if !(2..=64).contains(&tiles) || tiles % 2 != 0 => return Err("Checker tiles must be even and between 2 and 64".into()),
                NodeKind::Blend { factor } if !factor.is_finite() || !(0.0..=1.0).contains(&factor) => return Err("Blend factor out of range".into()),
                NodeKind::Bricks { columns, rows, mortar }
                    if !(2..=64).contains(&columns) || !(2..=64).contains(&rows) ||
                        rows % 2 != 0 || !mortar.is_finite() || !(0.0..=0.4).contains(&mortar)
                    => return Err("Brick settings out of range".into()),
                NodeKind::Voronoi { cells, .. } if !(2..=32).contains(&cells)
                    => return Err("Voronoi cells out of range".into()),
                NodeKind::Strata { bands, warp, .. } if !(1..=64).contains(&bands) || !warp.is_finite() || !(0.0..=1.0).contains(&warp)
                    => return Err("Strata settings out of range".into()),
                NodeKind::Library { amount, .. } if !amount.is_finite() || !(0.0..=1.0).contains(&amount)
                    => return Err("Library node amount must be between 0 and 1".into()),
                NodeKind::Scalar { value } if !value.is_finite() ||
                    !(0.0..=1.0).contains(&value)
                    => return Err("Scalar value must be 0..1".into()),
                NodeKind::Levels { black,white,gamma } if
                    !black.is_finite() || !white.is_finite() || !gamma.is_finite() ||
                    !(0.0..1.0).contains(&black) || !(0.0..=1.0).contains(&white) ||
                    black>=white || !(0.1..=5.0).contains(&gamma)
                    => return Err("Levels require 0 <= black < white <= 1 and gamma 0.1..5".into()),
                NodeKind::ThresholdMask { threshold,softness }
                    if !threshold.is_finite() || !softness.is_finite() ||
                    !(0.0..=1.0).contains(&threshold) ||
                    !(0.0..=1.0).contains(&softness)
                    => return Err("Threshold Mask requires threshold and softness in 0..1".into()),
                _ => {}
            }
        }
        for link in &self.links {
            let from = self.node(link.from).ok_or("Broken source link")?;
            let to = self.node(link.to).ok_or("Broken destination link")?;
            if matches!(from.kind, NodeKind::Output) || link.input >= to.kind.input_count() {
                return Err("Invalid link socket".into());
            }
            if !from.kind.output_type().can_feed(
                to.kind.input_type(link.input).expect("validated socket")) {
                return Err(format!("Incompatible socket types at node {}",link.to));
            }
            if !ports.insert((link.to, link.input)) {
                return Err("Multiple links to the same input".into());
            }
            if link.from == link.to || self.reaches(link.to, link.from) {
                return Err("Cycle detected in project".into());
            }
        }
        Ok(())
    }

    pub fn render_rgba(&self, width: u32, height: u32) -> Result<Vec<u8>, String> {
        self.render_output_channel_rgba(0, width, height)
    }

    /// Preview any node, including nodes not wired into Material Output.
    pub fn render_node_rgba(&self, id: u64, width: u32, height: u32) -> Result<Vec<u8>, String> {
        self.validate()?;
        if self.node(id).is_none() { return Err("Node not found".into()); }
        self.render_pixels(width, height, |u, v| self.sample(id, u, v))
    }

    /// Output channel indices are 0=albedo, 1=roughness, 2=metallic, 3=height.
    pub fn render_output_channel_rgba(&self, channel: usize, width: u32, height: u32) -> Result<Vec<u8>, String> {
        self.validate()?;
        if channel >= 4 { return Err("Unsupported material channel".into()); }
        let output = self.nodes.iter().find(|n| matches!(n.kind, NodeKind::Output)).unwrap().id;
        let default = match channel {
            1 => [0.72; 3], // dielectric matte roughness
            3 => [0.5; 3],  // neutral height
            _ => [0.0; 3],
        };
        let source = self.links.iter().find(|l| l.to == output && l.input == channel).map(|l| l.from);
        self.render_pixels(width, height, |u, v| match source {
            Some(id) => self.sample(id, u, v),
            None => default,
        })
    }

    /// Render several PBR output channels in one pixel traversal. Upstream
    /// node evaluations are reused only at the *same exact UV coordinate*,
    /// including transformed/filter sample positions. No upstream raster
    /// interpolation or u8 quantization occurs before the final output.
    ///
    /// The original one-channel API does not allocate a memo. The memo stores
    /// only branches shared by the requested channels, with at most 64
    /// node/coordinate values per pixel (falling back to evaluation on overflow).
    /// Compare byte-equivalence and CPU times before opting into this path.
    pub fn render_channels_memoized_rgba(&self, channels:&[usize], width:u32, height:u32)
        -> Result<(Vec<Vec<u8>>, MultiChannelStats),String> {
        self.validate()?;
        if width==0 || height==0 || width>4096 || height>4096 {
            return Err("Image dimensions must be 1–4096".into());
        }
        if channels.len()>4 || channels.iter().any(|&channel|channel>=4) {
            return Err("Unsupported material channel".into());
        }
        let output=self.nodes.iter().find(|n|matches!(n.kind,NodeKind::Output))
            .expect("validated graph").id;
        let mut sources=[None;4];
        for link in &self.links {
            if link.to==output && link.input<4 {
                sources[link.input]=Some(link.from);
            }
        }
        // Count each node once per requested channel. Only branches that are
        // shared across channel closures are candidates for memoization.
        let mut usage=HashMap::<u64,u32>::new();
        for &channel in channels {
            let mut seen=HashSet::new();
            let mut stack:Vec<u64>=sources[channel].into_iter().collect();
            while let Some(id)=stack.pop() {
                if seen.insert(id) {
                    *usage.entry(id).or_default()+=1;
                    stack.extend(self.links.iter().filter(|link|link.to==id)
                        .map(|link|link.from));
                }
            }
        }
        let shared:Vec<u64>=usage.into_iter().filter(|(_,count)|*count>1)
            .map(|(id,_)|id).collect();
        let mut images:Vec<Vec<u8>>=channels.iter().map(|_|
            Vec::with_capacity(width as usize*height as usize*4)).collect();
        let memo=SampleMemo::new(shared);
        for y in 0..height {
            for x in 0..width {
                memo.values.borrow_mut().clear();
                let u=x as f32/width as f32;
                let v=y as f32/height as f32;
                for (slot,&channel) in channels.iter().enumerate() {
                    let default=match channel {
                        1=>[0.72;3],
                        3=>[0.5;3],
                        _=>[0.0;3],
                    };
                    let rgb=match sources[channel] {
                        Some(source)=>self.sample_memo(source,u,v,Some(&memo)),
                        None=>default,
                    };
                    for component in rgb {
                        images[slot].push((component.clamp(0.0,1.0)*255.0).round() as u8);
                    }
                    images[slot].push(255);
                }
            }
        }
        Ok((images,memo.stats()))
    }

    pub fn render_normal_rgba(&self, width: u32, height: u32) -> Result<Vec<u8>, String> {
        let height_map = self.render_output_channel_rgba(3, width, height)?;
        let mut normals = vec![0u8; height_map.len()];
        for y in 0..height {
            for x in 0..width {
                let height_at = |xi: u32, yi: u32| -> f32 {
                    height_map[((yi * width + xi) * 4) as usize] as f32 / 255.0
                };
                let left = height_at((x + width - 1) % width, y);
                let right = height_at((x + 1) % width, y);
                let above = height_at(x, (y + height - 1) % height);
                let below = height_at(x, (y + 1) % height);
                let nx = (left - right) * 4.0;
                let ny = (above - below) * 4.0;
                let length = (nx * nx + ny * ny + 1.0).sqrt();
                let index = ((y * width + x) * 4) as usize;
                normals[index] = ((nx / length * 0.5 + 0.5) * 255.0).round() as u8;
                normals[index + 1] = ((ny / length * 0.5 + 0.5) * 255.0).round() as u8;
                normals[index + 2] = ((1.0 / length * 0.5 + 0.5) * 255.0).round() as u8;
                normals[index + 3] = 255;
            }
        }
        Ok(normals)
    }

    fn render_pixels(&self, width: u32, height: u32, sample: impl Fn(f32, f32) -> [f32; 3])
        -> Result<Vec<u8>, String>
    {
        if width == 0 || height == 0 || width > 4096 || height > 4096 {
            return Err("Image dimensions must be 1–4096".into());
        }
        let mut rgba = Vec::with_capacity(width as usize * height as usize * 4);
        for y in 0..height {
            for x in 0..width {
                let color = sample(x as f32 / width as f32, y as f32 / height as f32);
                for component in color {
                    rgba.push((component.clamp(0.0, 1.0) * 255.0).round() as u8);
                }
                rgba.push(255);
            }
        }
        Ok(rgba)
    }

    fn sample_input_memo(&self,node:u64,input:usize,u:f32,v:f32,
                         memo:Option<&SampleMemo>)->[f32;3] {
        match self.links.iter().find(|l|l.to==node && l.input==input) {
            Some(link)=>self.sample_memo(link.from,u,v,memo),
            None=>[0.0;3],
        }
    }

    fn sample(&self,id:u64,u:f32,v:f32)->[f32;3] {
        self.sample_memo(id,u,v,None)
    }

    fn sample_memo(&self,id:u64,u:f32,v:f32,
                   memo:Option<&SampleMemo>)->[f32;3] {
        if let Some(memo)=memo {
            if let Some(rgb)=memo.lookup(id,u,v) {return rgb;}
        }
        let node=self.node(id).expect("validated graph");
        let rgb=match node.kind {
            NodeKind::Noise { scale, octaves, seed } => {
                let mut value = 0.0;
                let mut weight = 1.0;
                let mut total = 0.0;
                for octave in 0..octaves {
                    value += periodic_noise(u, v, scale << octave, seed.wrapping_add(octave as u32)) * weight;
                    total += weight;
                    weight *= 0.5;
                }
                let grey = value / total;
                [grey; 3]
            }
            NodeKind::Checker { tiles } => {
                let col = (u * tiles as f32).floor() as i32;
                let row = (v * tiles as f32).floor() as i32;
                let grey = if (row + col) % 2 == 0 { 0.86 } else { 0.14 };
                [grey; 3]
            }
            NodeKind::Color { rgb } => rgb.map(|c| c as f32 / 255.0),
            NodeKind::Scalar { value } => [value;3],
            NodeKind::Luminance => {
                let rgb=self.sample_input_memo(id,0,u,v,memo);
                let grey=(0.2126*rgb[0]+0.7152*rgb[1]+0.0722*rgb[2])
                    .clamp(0.0,1.0);
                [grey;3]
            }
            NodeKind::Levels { black,white,gamma } => {
                let value=self.sample_input_memo(id,0,u,v,memo)[0];
                let mapped=((value-black)/(white-black)).clamp(0.0,1.0).powf(gamma);
                [mapped;3]
            }
            NodeKind::ThresholdMask { threshold,softness } => {
                let value=self.sample_input_memo(id,0,u,v,memo)[0];
                let coverage=if softness==0.0 {
                    if value>=threshold {1.0} else {0.0}
                } else {
                    let t=((value-threshold)/softness+0.5).clamp(0.0,1.0);
                    t*t*(3.0-2.0*t)
                };
                [coverage;3]
            }
            NodeKind::Invert => self.sample_input_memo(id, 0, u, v,memo).map(|c| 1.0 - c),
            NodeKind::Blend { factor } => {
                let a = self.sample_input_memo(id, 0, u, v,memo);
                let b = self.sample_input_memo(id, 1, u, v,memo);
                std::array::from_fn(|i| a[i] * (1.0 - factor) + b[i] * factor)
            }
            NodeKind::Bricks { columns, rows, mortar } => {
                let row_position = v * rows as f32;
                let row = row_position.floor() as i32;
                let horizontal = (u * columns as f32 + if row % 2 == 0 { 0.0 } else { 0.5 }).fract();
                let vertical = row_position.fract();
                let edge = horizontal.min(1.0 - horizontal)
                    .min(vertical.min(1.0 - vertical));
                let brick = ((edge - mortar * 0.5) / 0.025).clamp(0.0, 1.0);
                [brick; 3]
            }
            NodeKind::Voronoi { cells, seed } => {
                let x = u * cells as f32;
                let y = v * cells as f32;
                let bx = x.floor() as i32;
                let by = y.floor() as i32;
                let mut best = f32::MAX;
                for dy in -1..=1 {
                    for dx in -1..=1 {
                        let cx = bx + dx;
                        let cy = by + dy;
                        let rx = lattice(cx, cy, cells as i32, seed);
                        let ry = lattice(cx, cy, cells as i32, seed ^ 0x97f4_a7c1);
                        let px = cx as f32 + rx - x;
                        let py = cy as f32 + ry - y;
                        best = best.min((px * px + py * py).sqrt());
                    }
                }
                [(best * 1.15).clamp(0.0, 1.0); 3]
            }
            NodeKind::Strata { bands, warp, seed } => {
                // Periodic warp makes layered wood, water and shell-band patterns.
                // sin(2π·bands·(v + 1)) repeats exactly at opposing edges.
                let perturb = periodic_noise(u, v, 8, seed) - 0.5;
                let phase = std::f32::consts::TAU * bands as f32 * (v + perturb * warp * 0.08);
                let stripe = phase.sin() * 0.5 + 0.5;
                [stripe; 3]
            }
            NodeKind::Colorize { dark, light } => {
                let input = self.sample_input_memo(id, 0, u, v,memo);
                let t = (input[0] * 0.2126 + input[1] * 0.7152 + input[2] * 0.0722).clamp(0.0, 1.0);
                std::array::from_fn(|i| (dark[i] as f32 * (1.0 - t) + light[i] as f32 * t) / 255.0)
            }
            NodeKind::Library { operation, amount, seed } => operation.evaluate(u, v, amount, seed,
                |input, su, sv| self.sample_input_memo(id, input, su, sv,memo)),
            NodeKind::Output => self.sample_input_memo(id, 0, u, v,memo),
        };
        if let Some(memo)=memo {memo.record(id,u,v,rgb);}
        rgb
    }
}

fn lattice(x: i32, y: i32, period: i32, seed: u32) -> f32 {
    // Wrap the lattice so opposite edges repeat exactly.
    let x = x.rem_euclid(period) as u32;
    let y = y.rem_euclid(period) as u32;
    let mut h = seed ^ x.wrapping_mul(0x9e37_79b9) ^ y.wrapping_mul(0x85eb_ca6b);
    h ^= h >> 16;
    h = h.wrapping_mul(0x7feb_352d);
    h ^= h >> 15;
    h = h.wrapping_mul(0x846c_a68b);
    h ^= h >> 16;
    h as f32 / u32::MAX as f32
}

fn periodic_noise(u: f32, v: f32, period: u32, seed: u32) -> f32 {
    let period = period as i32;
    let px = u * period as f32;
    let py = v * period as f32;
    let x0 = px.floor() as i32;
    let y0 = py.floor() as i32;
    let fx = px - x0 as f32;
    let fy = py - y0 as f32;
    let sx = fx * fx * (3.0 - 2.0 * fx);
    let sy = fy * fy * (3.0 - 2.0 * fy);
    let a = lattice(x0, y0, period, seed);
    let b = lattice(x0 + 1, y0, period, seed);
    let c = lattice(x0, y0 + 1, period, seed);
    let d = lattice(x0 + 1, y0 + 1, period, seed);
    (a + (b - a) * sx) * (1.0 - sy) + (c + (d - c) * sx) * sy
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starter_graph_renders_opaque_pixels() {
        let rgba = Graph::default().render_rgba(16, 8).unwrap();
        assert_eq!(rgba.len(), 16 * 8 * 4);
        assert!(rgba.chunks_exact(4).all(|px| px[3] == 255));
    }

    #[test]
    fn cycles_are_rejected() {
        let mut graph = Graph::default();
        let a = graph.add_node(NodeKind::Invert, [0.0, 0.0]);
        let b = graph.add_node(NodeKind::Invert, [0.0, 0.0]);
        graph.connect(a, b, 0).unwrap();
        assert!(graph.connect(b, a, 0).is_err());
        assert_eq!(graph.links.len(), 2);
    }

    #[test]
    fn relinking_replaces_previous_input() {
        let mut graph = Graph::default();
        let c = graph.add_node(NodeKind::Color { rgb: [255, 0, 0] }, [0.0, 0.0]);
        graph.connect(c, 2, 0).unwrap();
        assert_eq!(graph.links.iter().filter(|l| l.to == 2).count(), 1);
        assert_eq!(&graph.render_rgba(1, 1).unwrap()[..4], &[255, 0, 0, 255]);
    }

    #[test]
    fn noise_tiles_on_both_axes() {
        for u in [0.0, 0.135, 0.5, 0.901] {
            assert!((periodic_noise(u, 0.0, 8, 17) - periodic_noise(u, 1.0, 8, 17)).abs() < 0.00001);
            assert!((periodic_noise(0.0, u, 8, 17) - periodic_noise(1.0, u, 8, 17)).abs() < 0.00001);
        }
    }

    #[test]
    fn projects_round_trip_and_validate() {
        let graph = Graph::default();
        let json = serde_json::to_string(&graph).unwrap();
        let restored: Graph = serde_json::from_str(&json).unwrap();
        restored.validate().unwrap();
        assert_eq!(restored.nodes.len(), 2);
    }

    #[test]
    fn imported_cycles_are_rejected() {
        let mut graph = Graph::default();
        let a = graph.add_node(NodeKind::Invert, [0.0, 0.0]);
        let b = graph.add_node(NodeKind::Invert, [0.0, 0.0]);
        graph.links.push(Link { from: a, to: b, input: 0 });
        graph.links.push(Link { from: b, to: a, input: 0 });
        assert!(graph.validate().is_err());
    }
    #[test]
    fn demo_material_exports_channels_and_flat_normal_when_disconnected() {
        let demo = Graph::demo_bricks();
        demo.validate().unwrap();
        assert_eq!(demo.render_output_channel_rgba(0, 16, 16).unwrap().len(), 1024);
        let mut empty = Graph::default();
        empty.disconnect(2, 0);
        let normals = empty.render_normal_rgba(8, 8).unwrap();
        assert!(normals.chunks_exact(4).all(|px| px == [128, 128, 255, 255]));
    }

    #[test]
    fn reconnect_source_preserves_target_socket() {
        let mut graph = Graph::default();
        let alternate = graph.add_node(NodeKind::Color { rgb: [10, 20, 30] }, [2.0, 3.0]);
        graph.rewire(2, 0, alternate, 2, 0).unwrap();
        assert_eq!(graph.links.len(), 1);
        assert_eq!(graph.links[0].from, alternate);
        assert_eq!(graph.links[0].to, 2);
    }

    #[test]
    fn move_connection_to_different_input() {
        let mut graph = Graph::default();
        let blend = graph.add_node(NodeKind::Blend { factor: 0.5 }, [2.0, 3.0]);
        graph.rewire(2, 0, 1, blend, 1).unwrap();
        assert_eq!(graph.links.len(), 1);
        assert_eq!(graph.links[0].to, blend);
        assert_eq!(graph.links[0].input, 1);
        assert!(graph.links.iter().all(|link| link.to != 2));
    }

    #[test]
    fn failed_rewire_does_not_destroy_previous_connection() {
        let mut graph = Graph::default();
        let filter = graph.add_node(NodeKind::Invert, [2.0, 3.0]);
        graph.connect(filter, 2, 0).unwrap(); // replaces original output cable
        let before = graph.links.len();
        assert!(graph.rewire(2, 0, 2, filter, 0).is_err()); // Output cannot be a source
        assert_eq!(graph.links.len(), before);
        assert_eq!(graph.links[0].from, filter);
        assert_eq!(graph.links[0].to, 2);
        graph.validate().unwrap();
    }

    #[test]
    fn failed_cycle_rewire_preserves_graph() {
        let mut graph = Graph::default();
        let first = graph.add_node(NodeKind::Invert, [0.0, 0.0]);
        let second = graph.add_node(NodeKind::Invert, [1.0, 1.0]);
        let third = graph.add_node(NodeKind::Invert, [2.0, 2.0]);
        graph.connect(first, second, 0).unwrap();
        graph.connect(second, third, 0).unwrap();
        // Rewire the independent output cable to third -> first: this would
        // create first -> second -> third -> first, so it must be rejected.
        assert!(graph.rewire(2, 0, third, first, 0).is_err());
        assert!(graph.links.iter().any(|l| l.from == 1 && l.to == 2));
        assert!(graph.links.iter().any(|l| l.from == first && l.to == second));
        graph.validate().unwrap();
    }

    #[test]
    fn voxel_shape_settings_survive_project_round_trip() {
        let mut graph=Graph::default();
        graph.voxel.shape=VoxelShape::HexPrism;
        graph.voxel.side_mapping=SideMapping::Repeat;
        let json=serde_json::to_string(&graph).unwrap();
        let restored:Graph=serde_json::from_str(&json).unwrap();
        assert_eq!(restored.voxel.shape,VoxelShape::HexPrism);
        assert_eq!(restored.voxel.side_mapping,SideMapping::Repeat);
        restored.validate().unwrap();
    }

    #[test]
    fn legacy_graphs_without_voxel_field_still_load() {
        let graph=Graph::default();
        let mut value=serde_json::to_value(&graph).unwrap();
        value.as_object_mut().unwrap().remove("voxel");
        let loaded:Graph=serde_json::from_value(value).unwrap();
        assert_eq!(loaded.voxel.shape,VoxelShape::Cube);
        loaded.validate().unwrap();
    }

    #[test]
    fn strata_is_seamless_and_parameters_valid() {
        let mut graph=Graph::default();
        let id=graph.add_node(NodeKind::Strata { bands: 17, warp: 0.4, seed: 3 }, [0.0,0.0]);
        graph.connect(id,2,0).unwrap();
        graph.validate().unwrap();
        for p in [0.0, 0.1, 0.5, 0.93] {
            assert!((graph.sample(id, p, 0.0)[0] - graph.sample(id, p, 1.0)[0]).abs() < 0.0001);
            assert!((graph.sample(id, 0.0, p)[0] - graph.sample(id, 1.0, p)[0]).abs() < 0.0001);
        }
        graph.node_mut(id).unwrap().kind = NodeKind::Strata { bands: 0, warp: 0.4, seed: 3 };
        assert!(graph.validate().is_err());
    }

    #[test]
    fn old_projects_remain_readable() {
        let graph = Graph::default();
        let text = serde_json::to_string(&graph).unwrap();
        let restored: Graph = serde_json::from_str(&text).unwrap();
        restored.validate().unwrap();
        assert_eq!(restored.node(2).unwrap().kind.input_count(), 4);
    }

    #[test]
    fn typed_ports_allow_scalar_to_color_but_not_implicit_color_to_scalar() {
        let mut graph=Graph::default();
        let scalar=graph.add_node(NodeKind::Scalar{value:0.5},[0.0,0.0]);
        let levels=graph.add_node(NodeKind::Levels{
            black:0.25,white:0.75,gamma:1.0
        },[25.0,0.0]);
        assert_eq!(graph.node(scalar).unwrap().kind.output_type(),SocketType::Scalar);
        assert_eq!(graph.node(levels).unwrap().kind.input_type(0),Some(SocketType::Scalar));
        assert!(graph.connect(1,levels,0).unwrap_err().contains("Color to Scalar"));
        graph.connect(scalar,levels,0).unwrap();
        graph.connect(levels,2,0).unwrap();
        assert_eq!(&graph.render_rgba(1,1).unwrap()[..4],&[128,128,128,255]);
        let saved=serde_json::to_string(&graph).unwrap();
        let restored:Graph=serde_json::from_str(&saved).unwrap();
        assert_eq!(graph.render_rgba(8,8).unwrap(),restored.render_rgba(8,8).unwrap());
        graph.connect(scalar,2,0).unwrap();
        assert_eq!(&graph.render_rgba(1,1).unwrap()[..4],&[128,128,128,255]);
        assert_eq!(graph.node(levels).unwrap().kind.parameters().len(),3);
    }

    #[test]
    fn explicit_luminance_converts_legacy_rgb_for_scalar_levels() {
        let mut graph=Graph::default();
        let luminance=graph.add_node(NodeKind::Luminance,[10.0,10.0]);
        let levels=graph.add_node(NodeKind::Levels{
            black:0.2,white:0.8,gamma:1.0
        },[20.0,10.0]);
        graph.connect(1,luminance,0).unwrap();
        graph.connect(luminance,levels,0).unwrap();
        graph.connect(levels,2,0).unwrap();
        assert_eq!(graph.node(luminance).unwrap().kind.input_type(0),
            Some(SocketType::Color));
        assert_eq!(graph.node(luminance).unwrap().kind.output_type(),
            SocketType::Scalar);
        let pixels=graph.render_rgba(32,32).unwrap();
        let restored:Graph=serde_json::from_str(&serde_json::to_string(&graph).unwrap()).unwrap();
        assert_eq!(restored.render_rgba(32,32).unwrap(),pixels);
    }

    #[test]
    fn threshold_mask_is_typed_and_roundtrips_without_changing_legacy_outputs() {
        let mut graph=Graph::default();
        let scalar=graph.add_node(NodeKind::Scalar{value:0.5},[0.0,0.0]);
        let mask=graph.add_node(NodeKind::ThresholdMask{
            threshold:0.5,softness:0.2
        },[100.0,0.0]);
        let levels=graph.add_node(NodeKind::Levels{
            black:0.0,white:1.0,gamma:1.0
        },[200.0,0.0]);
        assert!(graph.connect(1,mask,0).unwrap_err().contains("Color to Scalar"));
        graph.connect(scalar,mask,0).unwrap();
        graph.connect(mask,levels,0).unwrap();
        graph.connect(mask,2,0).unwrap();
        assert_eq!(graph.node(mask).unwrap().kind.output_type(),SocketType::Mask);
        assert_eq!(&graph.render_rgba(1,1).unwrap()[..4],&[128,128,128,255]);
        let restored:Graph=serde_json::from_str(&serde_json::to_string(&graph).unwrap()).unwrap();
        assert_eq!(graph.render_rgba(16,16).unwrap(),restored.render_rgba(16,16).unwrap());
        assert_eq!(graph.node(mask).unwrap().kind.type_id(),"threshold_mask");
        assert_eq!(graph.node(mask).unwrap().kind.parameters().len(),2);
        graph.node_mut(mask).unwrap().kind=NodeKind::ThresholdMask{
            threshold:0.5,softness:0.0
        };
        assert_eq!(&graph.render_rgba(1,1).unwrap()[..4],&[255,255,255,255]);
    }

    #[test]
    fn threshold_mask_rejects_invalid_parameters() {
        let mut graph=Graph::default();
        let mask=graph.add_node(NodeKind::ThresholdMask{
            threshold:f32::NAN,softness:0.0
        },[0.0,0.0]);
        assert!(graph.validate().is_err());
        graph.node_mut(mask).unwrap().kind=NodeKind::ThresholdMask{
            threshold:0.5,softness:-0.1
        };
        assert!(graph.validate().is_err());
        graph.node_mut(mask).unwrap().kind=NodeKind::ThresholdMask{
            threshold:0.5,softness:0.0
        };
        assert!(graph.validate().is_ok());
    }

    #[test]
    fn typed_socket_invalid_saved_links_and_level_parameters_are_rejected() {
        let mut graph=Graph::default();
        let levels=graph.add_node(NodeKind::Levels{
            black:0.0,white:1.0,gamma:1.0,
        },[0.0,0.0]);
        graph.links.push(Link{from:1,to:levels,input:0});
        assert!(graph.validate().unwrap_err().contains("Incompatible socket"));
        graph.links.clear();
        graph.node_mut(levels).unwrap().kind=NodeKind::Levels{
            black:0.8,white:0.2,gamma:1.0,
        };
        assert!(graph.validate().is_err());
        graph.node_mut(levels).unwrap().kind=NodeKind::Levels{
            black:0.0,white:1.0,gamma:f32::NAN,
        };
        assert!(graph.validate().is_err());
    }

    #[test]
    fn memoized_shared_channels_match_unmodified_legacy_pixels() {
        let mut graph=Graph::demo_bricks();
        // The same seeded Noise feeds Roughness and Metallic, so its exact
        // pixel evaluation can be reused without quantizing intermediate RGB.
        graph.connect(3,4,2).unwrap();
        for size in [8,32,64] {
            let (images,stats)=graph.render_channels_memoized_rgba(
                &[0,1,2,3],size,size).unwrap();
            for (index,channel) in [0,1,2,3].into_iter().enumerate() {
                assert_eq!(images[index],
                    graph.render_output_channel_rgba(channel,size,size).unwrap());
            }
            assert!(stats.reused_samples>=size as u64*size as u64,
                "shared seeded source was recomputed for every output pixel");
            assert!(stats.evaluated_samples>0);
            assert!(stats.peak_entries_per_pixel<=MAX_PIXEL_SAMPLE_ENTRIES);
        }
    }

    #[test]
    fn memoized_uv_warps_and_neighborhood_filters_are_byte_exact() {
        for operation in [LibraryNode::Warp,LibraryNode::Rotate,
                          LibraryNode::Blur,LibraryNode::EdgeDetect] {
            let mut graph=Graph::default();
            let op=graph.add_node(NodeKind::Library{
                operation,amount:0.4,seed:41
            },[120.0,10.0]);
            graph.connect(1,op,0).unwrap();
            graph.connect(op,2,0).unwrap();
            graph.connect(op,2,3).unwrap();
            let (images,stats)=graph.render_channels_memoized_rgba(
                &[0,3],12,12).unwrap();
            assert_eq!(images[0],graph.render_rgba(12,12).unwrap(),"{operation:?}");
            assert_eq!(images[1],
                graph.render_output_channel_rgba(3,12,12).unwrap(),"{operation:?}");
            assert!(stats.reused_samples>0,"{operation:?}");
        }
    }

    #[test]
    fn memoized_channels_validate_inputs_and_support_disconnected_outputs() {
        let mut graph=Graph::default();
        graph.disconnect(2,0);
        let (images,stats)=graph.render_channels_memoized_rgba(
            &[0,1,2,3],4,4).unwrap();
        assert_eq!(stats.evaluated_samples,0);
        for channel in 0..4 {
            assert_eq!(images[channel],
                graph.render_output_channel_rgba(channel,4,4).unwrap());
        }
        assert!(graph.render_channels_memoized_rgba(&[4],4,4).is_err());
        assert!(graph.render_channels_memoized_rgba(&[0],0,4).is_err());
        graph.node_mut(1).unwrap().position=[f32::NAN,0.0];
        assert!(graph.render_channels_memoized_rgba(&[0],4,4).is_err());
    }

    #[test]
    fn voronoi_is_tileable() {
        let mut graph = Graph::default();
        let id = graph.add_node(NodeKind::Voronoi { cells: 8, seed: 9 }, [0.0, 0.0]);
        for t in [0.0, 0.13, 0.41, 0.91] {
            assert!((graph.sample(id, t, 0.0)[0] - graph.sample(id, t, 1.0)[0]).abs() < 0.0001);
            assert!((graph.sample(id, 0.0, t)[0] - graph.sample(id, 1.0, t)[0]).abs() < 0.0001);
        }
    }

}
