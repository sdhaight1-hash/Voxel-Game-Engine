//! CPU-based, deterministic, tileable procedural texture engine.
//! No UI types are used here, so the graph and renderer can be tested independently.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Graph {
    pub nodes: Vec<Node>,
    pub links: Vec<Link>,
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum NodeKind {
    Noise { scale: u32, octaves: u8, seed: u32 },
    Checker { tiles: u32 },
    Color { rgb: [u8; 3] },
    Invert,
    Blend { factor: f32 },
    Output,
}

impl NodeKind {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Noise { .. } => "Noise",
            Self::Checker { .. } => "Checker",
            Self::Color { .. } => "Color",
            Self::Invert => "Invert",
            Self::Blend { .. } => "Blend",
            Self::Output => "Output",
        }
    }

    pub fn input_count(&self) -> usize {
        match self {
            Self::Noise { .. } | Self::Checker { .. } | Self::Color { .. } => 0,
            Self::Invert | Self::Output => 1,
            Self::Blend { .. } => 2,
        }
    }

    pub fn input_name(&self, index: usize) -> &'static str {
        match (self, index) {
            (Self::Blend { .. }, 0) => "A",
            (Self::Blend { .. }, 1) => "B",
            (Self::Output, 0) => "Image",
            (Self::Invert, 0) => "Image",
            _ => "",
        }
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
        }
    }
}

impl Graph {
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
        if input >= target.kind.input_count() {
            return Err("That input socket does not exist".to_string());
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
                _ => {}
            }
        }
        for link in &self.links {
            let from = self.node(link.from).ok_or("Broken source link")?;
            let to = self.node(link.to).ok_or("Broken destination link")?;
            if matches!(from.kind, NodeKind::Output) || link.input >= to.kind.input_count() {
                return Err("Invalid link socket".into());
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
        self.validate()?;
        if width == 0 || height == 0 || width > 4096 || height > 4096 {
            return Err("Image dimensions must be 1–4096".into());
        }
        let output = self.nodes.iter().find(|n| matches!(n.kind, NodeKind::Output)).unwrap().id;
        let mut rgba = Vec::with_capacity(width as usize * height as usize * 4);
        for y in 0..height {
            for x in 0..width {
                let color = self.sample(output, x as f32 / width as f32, y as f32 / height as f32);
                for component in color {
                    rgba.push((component.clamp(0.0, 1.0) * 255.0).round() as u8);
                }
                rgba.push(255);
            }
        }
        Ok(rgba)
    }

    fn sample_input(&self, node: u64, input: usize, u: f32, v: f32) -> [f32; 3] {
        match self.links.iter().find(|l| l.to == node && l.input == input) {
            Some(link) => self.sample(link.from, u, v),
            None => [0.0; 3],
        }
    }

    fn sample(&self, id: u64, u: f32, v: f32) -> [f32; 3] {
        let node = self.node(id).expect("validated graph");
        match node.kind {
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
            NodeKind::Invert => self.sample_input(id, 0, u, v).map(|c| 1.0 - c),
            NodeKind::Blend { factor } => {
                let a = self.sample_input(id, 0, u, v);
                let b = self.sample_input(id, 1, u, v);
                std::array::from_fn(|i| a[i] * (1.0 - factor) + b[i] * factor)
            }
            NodeKind::Output => self.sample_input(id, 0, u, v),
        }
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
}
