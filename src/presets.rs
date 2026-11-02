//! Data-driven natural-material presets built from the user's descriptive catalog.
//! These are original procedural approximations, not imported game texture assets.

use crate::{Graph, LibraryNode, Link, Node, NodeKind, VoxelSettings};
use serde::Deserialize;
use std::collections::HashSet;

#[derive(Deserialize)]
struct SourceCatalog {
    version: u32,
    groups: Vec<SourceGroup>,
}

#[derive(Deserialize)]
struct SourceGroup {
    category: String,
    family: String,
    names: Vec<String>,
    variants: Vec<String>,
    profile: String,
}

/// One selectable material/appearance variant from the bundled catalog.
#[derive(Debug, Clone)]
pub struct Preset {
    pub category: String,
    pub family: String,
    pub material: String,
    pub variant: String,
    pub profile: String,
    pub seed: u32,
}

impl Preset {
    pub fn title(&self) -> String {
        if self.variant == "Base" {
            self.material.clone()
        } else {
            format!("{} · {}", self.material, self.variant)
        }
    }

    pub fn matches(&self, query: &str) -> bool {
        let terms = query.split_whitespace().map(str::to_lowercase);
        let haystack = format!("{} {} {} {}", self.category, self.family, self.material, self.variant).to_lowercase();
        terms.into_iter().all(|term| haystack.contains(&term))
    }

    pub fn colors(&self) -> ([u8; 3], [u8; 3]) {
        let name = self.material.to_lowercase();
        let variant = self.variant.to_lowercase();
        let mut colors = match self.profile.as_str() {
            "rock" => ([57, 57, 62], [160, 157, 151]),
            "ore" => ([63, 63, 66], [174, 146, 116]),
            "crystal" => ([47, 78, 92], [174, 209, 222]),
            "soil" => ([49, 32, 23], [136, 104, 70]),
            "sand" => ([133, 105, 68], [223, 203, 151]),
            "gravel" => ([67, 68, 67], [179, 169, 149]),
            "clay" => ([65, 69, 79], [155, 155, 170]),
            "wood" => ([54, 34, 24], [165, 117, 68]),
            "fruit" => ([46, 74, 27], [139, 157, 74]),
            "foliage" => ([35, 64, 32], [119, 159, 67]),
            "flower" => ([55, 81, 42], [184, 173, 77]),
            "grass" => ([65, 78, 37], [151, 165, 79]),
            "fungus" => ([70, 54, 38], [182, 149, 103]),
            "crop" => ([47, 84, 30], [155, 170, 73]),
            "fluid" => ([16, 55, 90], [67, 136, 169]),
            "lava" => ([41, 16, 13], [255, 111, 22]),
            "ice" => ([99, 151, 170], [218, 236, 240]),
            "snow" => ([161, 180, 187], [242, 248, 250]),
            "bone" => ([111, 106, 82], [225, 217, 183]),
            "organic" => ([49, 58, 30], [154, 165, 88]),
            "shell" => ([116, 95, 80], [233, 217, 194]),
            _ => ([64, 68, 62], [169, 177, 156]),
        };
        // Explicit palette overrides for named natural materials. The rest are
        // deterministic variations of a family palette, not an attempt at
        // scientifically precise or source-game colors.
        if name.contains("basalt") || name.contains("obsidian")
            || name.contains("anthracite") || name.contains("graphite")
            || name.contains("black coal") || name.contains("ebony") {
            colors = ([17, 21, 25], [78, 82, 87]);
        } else if name.contains("chalk") || name.contains("limestone") || name.contains("white marble")
            || name.contains("quartz") || name.contains("diamond") || name.contains("white oyster") {
            colors = ([121, 127, 128], [236, 232, 218]);
        } else if name.contains("sandstone") || name.contains("travertine") {
            colors = ([111, 78, 55], [216, 181, 130]);
        } else if name.contains("bauxite") || name.contains("hematite") || name.contains("cinnabar")
            || name.contains("red clay") {
            colors = ([76, 33, 27], [177, 82, 58]);
        } else if name.contains("malachite") || name.contains("emerald")
            || name.contains("green marble") || name.contains("peridot") || name.contains("olivine") {
            colors = ([12, 69, 55], [116, 187, 133]);
        } else if name.contains("native copper") {
            colors = ([81, 48, 37], [211, 132, 74]);
        } else if name.contains("native gold") || name.contains("sulfur") {
            colors = ([131, 96, 17], [244, 219, 70]);
        } else if name.contains("native silver") || name.contains("galena")
            || name.contains("magnetite") {
            colors = ([51, 62, 71], [184, 198, 204]);
        } else if name.contains("pink marble") || name.contains("rhodochrosite") {
            colors = ([116, 71, 87], [230, 166, 182]);
        } else if name.contains("lapis") || name.contains("blue clay") {
            colors = ([23, 47, 96], [108, 138, 213]);
        } else if name.contains("peat") || name.contains("bituminous") {
            colors = ([26, 21, 18], [88, 67, 52]);
        } else if name.contains("willow") {
            colors = ([48, 70, 31], [145, 168, 67]);
        } else if name.contains("purpleheart") || name.contains("crimson king") {
            colors = ([68, 31, 58], [155, 73, 121]);
        } else if name.contains("redwood") {
            colors = ([83, 40, 30], [188, 96, 63]);
        } else if name.contains("pink") || name.contains("raspberry")
            || name.contains("cranberry") {
            colors = ([92, 36, 59], [206, 100, 139]);
        } else if name.contains("bluebell") || name.contains("blueberry")
            || name.contains("blue lupine") {
            colors = ([38, 60, 99], [118, 143, 225]);
        } else if name.contains("daffodil") || name.contains("sunflower")
            || name.contains("golden poppy") {
            colors = ([100, 83, 20], [250, 218, 77]);
        } else if name.contains("orange") || name.contains("chanterelle") {
            colors = ([116, 61, 23], [237, 153, 58]);
        } else if name.contains("fly agaric") || name.contains("red rafflesia") {
            colors = ([94, 23, 23], [227, 71, 57]);
        } else if name.contains("green bamboo") {
            colors = ([43, 76, 36], [150, 179, 83]);
        } else if name.contains("brown bamboo") {
            colors = ([61, 46, 25], [158, 125, 72]);
        } else if name.contains("snow") || name.contains("ice") {
            colors = ([119, 158, 178], [236, 243, 246]);
        }
        if variant.contains("autumn") || variant.contains("withered") || variant.contains("dead")
            || variant.contains("dormant") || variant.contains("dry") {
            colors = ([69, 45, 24], [192, 143, 71]);
        } else if variant.contains("snow-covered") || variant.contains("snow layers") {
            colors = ([132, 156, 171], [238, 248, 250]);
        } else if variant.contains("grass") || variant.contains("leaves")
            || variant.contains("foliage") || variant.contains("sapling") {
            colors = ([36, 69, 27], [144, 178, 66]);
        } else if variant.contains("bark") || variant.contains("log") ||
            variant.contains("wood") || variant.contains("stem") {
            colors = ([51, 35, 25], [174, 125, 75]);
        } else if variant.contains("blossom") || variant.contains("flower") {
            colors = ([70, 94, 39], [215, 149, 170]);
        } else if variant.contains("ripe fruit") {
            colors = ([61, 80, 29], [215, 76, 55]);
        } else if variant.contains("crystal") {
            colors = ([49, 91, 111], [204, 220, 222]);
        } else if variant.contains("surface ore exposure") || variant.contains("rich deposit") {
            colors.1 = [195, 165, 103];
        } else if variant.contains("bare") && self.profile == "soil" {
            colors = ([45, 29, 20], [123, 84, 51]);
        }
        if self.profile == "shell" {
            if variant == "Turquoise" { colors = ([24, 100, 112], [126, 208, 209]); }
            if variant == "Seafoam" { colors = ([79, 128, 114], [191, 221, 201]); }
            if variant == "Dark purple" { colors = ([62, 43, 82], [148, 116, 160]); }
            if variant == "Cinnamon" { colors = ([109, 51, 26], [199, 125, 78]); }
            if variant == "Latte" { colors = ([113, 82, 65], [213, 187, 158]); }
        }
        colors
    }

    /// Build a distinct, editable material network from the expanded node library.
    /// Every preset gets its own seeded geometry-independent texture pipeline:
    /// primary pattern -> filter -> Colorize -> detail blend -> Material Output,
    /// plus independently adjustable roughness and metallic channels.
    pub fn build_graph(&self, voxel: VoxelSettings) -> Graph {
        let (dark, light) = self.colors();
        let variant = self.variant.to_lowercase();
        let profile = self.profile.as_str();
        let seed = self.seed;

        let primary = if variant.contains("bark") || variant.contains("log")
            || variant.contains("branch") || variant.contains("wood")
            || variant.contains("stem") || variant.contains("stalk")
            || matches!(profile, "wood" | "shell") {
            NodeKind::Strata {
                bands: if profile == "shell" { 22 } else { 16 },
                warp: 0.2 + (seed % 20) as f32 / 100.0,
                seed,
            }
        } else {
            let operation = if variant.contains("fragment") || variant.contains("loose")
                || variant.contains("boulder") || variant.contains("crystal")
                || matches!(profile, "gravel" | "crystal" | "fungus" | "flower") {
                LibraryNode::Crystal
            } else {
                match profile {
                    "ore" => LibraryNode::Crystal,
                    "sand" | "soil" | "clay" => LibraryNode::Dirt,
                    "foliage" | "fruit" => LibraryNode::Spots,
                    "grass" | "crop" => LibraryNode::Fibers,
                    "fluid" | "lava" => LibraryNode::SineWave,
                    "ice" => LibraryNode::Crystal,
                    "snow" => LibraryNode::Clouds,
                    "organic" => LibraryNode::Dirt,
                    "bone" => LibraryNode::ValueNoise,
                    _ => LibraryNode::FractalNoise,
                }
            };
            NodeKind::Library {
                operation,
                amount: 0.20 + (seed % 36) as f32 / 100.0,
                seed,
            }
        };

        // Pick a filter that enhances each material family without introducing
        // expensive neighborhood sampling on a graph rendered repeatedly by the UI.
        let filter = if matches!(profile, "wood" | "shell" | "fluid" | "lava") {
            LibraryNode::WaveDistort
        } else if matches!(profile, "ore" | "crystal" | "gravel" | "foliage" | "flower" | "ice") {
            LibraryNode::Smoothstep
        } else if matches!(profile, "sand" | "soil" | "clay" | "rock" | "snow") {
            LibraryNode::BrightnessContrast
        } else {
            LibraryNode::Gamma
        };

        let detail = match profile {
            "wood" | "shell" | "grass" => LibraryNode::Fibers,
            "ore" | "crystal" | "gravel" | "flower" | "fungus" => LibraryNode::Spots,
            "sand" | "soil" | "clay" | "organic" => LibraryNode::Dirt,
            "fluid" | "lava" | "snow" | "ice" => LibraryNode::Clouds,
            _ => LibraryNode::ValueNoise,
        };

        let rough = match profile {
            "fluid" => 0.14, "lava" => 0.39, "ice" => 0.23,
            "crystal" => 0.25, "ore" => 0.52, "rock" => 0.75,
            "wood" => 0.82, "snow" => 0.84, "sand" => 0.94,
            "gravel" => 0.87, "soil" => 0.93, "foliage" => 0.76,
            "flower" => 0.66, "fruit" => 0.58, "shell" => 0.54,
            _ => 0.79,
        };
        let metallic = if profile == "ore" {
            if variant.contains("host rock") { 0.08 } else { 0.56 }
        } else { 0.0 };

        // Separate positions leave room for all inputs and make the network
        // editable using the existing draggable-node/cable interface.
        let nodes = vec![
            Node { id: 1, position: [35.0, 90.0], kind: primary },
            Node { id: 2, position: [250.0, 100.0],
                kind: NodeKind::Library {
                    operation: filter,
                    amount: 0.38 + (seed % 23) as f32 / 100.0,
                    seed: seed.wrapping_add(7),
                } },
            Node { id: 3, position: [250.0, 400.0],
                kind: NodeKind::Library {
                    operation: detail,
                    amount: 0.18 + (seed % 34) as f32 / 100.0,
                    seed: seed.wrapping_add(83),
                } },
            Node { id: 4, position: [475.0, 95.0],
                kind: NodeKind::Colorize { dark, light } },
            Node { id: 5, position: [700.0, 115.0],
                kind: NodeKind::Library {
                    operation: LibraryNode::Lerp,
                    amount: if profile == "ore" || profile == "flower" { 0.22 } else { 0.11 },
                    seed,
                } },
            Node { id: 6, position: [35.0, 670.0],
                kind: NodeKind::Color { rgb: [to_byte(rough); 3] } },
            Node { id: 7, position: [470.0, 615.0],
                kind: NodeKind::Library {
                    operation: LibraryNode::Lerp,
                    amount: 0.06,
                    seed,
                } },
            Node { id: 8, position: [255.0, 685.0],
                kind: NodeKind::Color { rgb: [to_byte(metallic); 3] } },
            Node { id: 9, position: [935.0, 140.0], kind: NodeKind::Output },
        ];
        Graph {
            nodes,
            links: vec![
                Link { from: 1, to: 2, input: 0 }, // primary -> filter
                Link { from: 2, to: 4, input: 0 }, // filtered pattern -> colorize
                Link { from: 4, to: 5, input: 0 }, // colorized base
                Link { from: 3, to: 5, input: 1 }, // fine secondary detail
                Link { from: 6, to: 7, input: 0 }, // roughness baseline
                Link { from: 2, to: 7, input: 1 }, // subtle roughness variation
                Link { from: 5, to: 9, input: 0 }, // final albedo
                Link { from: 7, to: 9, input: 1 }, // final roughness
                Link { from: 8, to: 9, input: 2 }, // metallic
                Link { from: 2, to: 9, input: 3 }, // height / derived normals
            ],
            voxel,
        }
    }
}

fn to_byte(value: f32) -> u8 { (value * 255.0).round() as u8 }

fn stable_hash(text: &str) -> u32 {
    text.bytes().fold(2166136261u32, |hash, b| (hash ^ b as u32).wrapping_mul(16777619))
}

/// The JSON is embedded in the binary; no extra asset folder or network needed.
pub fn builtin_presets() -> Result<Vec<Preset>, String> {
    let source: SourceCatalog = serde_json::from_str(include_str!("presets/catalog.json"))
        .map_err(|e| format!("Preset catalog JSON error: {e}"))?;
    if source.version != 1 { return Err("Unsupported preset catalog version".into()); }
    let mut seen = HashSet::new();
    let mut presets = Vec::new();
    for group in source.groups {
        if group.category.is_empty() || group.family.is_empty() ||
            group.profile.is_empty() || group.names.is_empty() || group.variants.is_empty() {
            return Err("Incomplete preset catalog group".into());
        }
        for material in &group.names {
            for variant in &group.variants {
                let key = format!("{}|{}|{}|{}", group.category, group.family, material, variant);
                if !seen.insert(key.clone()) {
                    return Err(format!("Duplicate material preset: {key}"));
                }
                presets.push(Preset {
                    category: group.category.clone(),
                    family: group.family.clone(),
                    material: material.clone(),
                    variant: variant.clone(),
                    profile: group.profile.clone(),
                    seed: stable_hash(&key),
                });
            }
        }
    }
    Ok(presets)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{VoxelShape, SideMapping};

    #[test]
    fn all_attached_catalog_categories_load() {
        let presets = builtin_presets().unwrap();
        assert_eq!(presets.len(), 1039);
        let cats: HashSet<_> = presets.iter().map(|p| p.category.as_str()).collect();
        assert_eq!(cats.len(), 14);
        assert!(presets.iter().any(|p|p.material == "Willow" &&
            p.variant == "Willow hanging leaf clusters"));
        assert!(presets.iter().any(|p|p.material == "Scallop" && p.variant == "Turquoise"));
        assert!(presets.iter().any(|p|p.material == "Diamond"));
        assert!(presets.iter().any(|p|p.material == "Fly agaric"));
        assert!(presets.iter().any(|p|p.material == "Glacier ice"));
        assert!(presets.iter().any(|p|p.material == "Blue clay"));
        assert!(presets.iter().any(|p|p.material == "Rye"));
        assert_eq!(presets.iter().filter(|p|p.category=="Seashells").count(),42);
    }

    #[test]
    fn every_preset_produces_valid_editable_node_graph() {
        let presets = builtin_presets().unwrap();
        for preset in presets {
            let graph = preset.build_graph(VoxelSettings::default());
            graph.validate().unwrap_or_else(|err|
                panic!("{} / {}: {}", preset.material, preset.variant, err));
        }
    }

    #[test]
    fn loading_presets_preserves_voxel_mode_and_project_serialization() {
        let preset = builtin_presets().unwrap().into_iter().find(|p|
            p.material == "Willow").unwrap();
        let voxel = VoxelSettings {
            shape: VoxelShape::HexPrism,
            side_mapping: SideMapping::Repeat,
            geometryless: false,
        };
        let graph = preset.build_graph(voxel);
        let saved = serde_json::to_string(&graph).unwrap();
        let restored: Graph = serde_json::from_str(&saved).unwrap();
        assert_eq!(restored.voxel, voxel);
        assert_eq!(restored.nodes.len(), 9);
        assert_eq!(restored.links.len(), 10);
    }

    #[test]
    fn material_presets_use_expanded_node_operations_and_have_distinct_profiles() {
        let presets = builtin_presets().unwrap();
        let profiles = [
            ("Basalt", "rock", LibraryNode::FractalNoise),
            ("Native copper", "ore", LibraryNode::Crystal),
            ("Blue clay", "clay", LibraryNode::Dirt),
            ("Lake ice", "ice", LibraryNode::Crystal),
            ("Scallop", "shell", LibraryNode::Fibers),
            ("Willow", "wood", LibraryNode::Fibers),
        ];
        for (name, profile, detail) in profiles {
            let p = presets.iter().find(|p|p.material == name).unwrap();
            assert_eq!(p.profile, profile);
            let graph = p.build_graph(VoxelSettings::default());
            assert!(graph.nodes.iter().any(|n|matches!(n.kind,
                NodeKind::Library { operation, .. } if operation==detail)),
                "No {detail:?} node for {name}");
            graph.validate().unwrap();
            assert_eq!(graph.render_rgba(8,8).unwrap().len(),8*8*4);
        }
    }

    #[test]
    fn presets_support_geometryless_and_voxel_modes() {
        let p = builtin_presets().unwrap().into_iter()
            .find(|p|p.material=="Willow").unwrap();
        for geometryless in [true, false] {
            let settings = VoxelSettings { geometryless, ..Default::default() };
            let graph = p.build_graph(settings);
            graph.validate().unwrap();
            assert_eq!(graph.voxel.geometryless,geometryless);
            let round_trip:Graph=serde_json::from_str(&serde_json::to_string(&graph).unwrap()).unwrap();
            assert_eq!(round_trip.voxel,settings);
        }
    }

    #[test]
    fn search_matches_material_category_and_variant() {
        let willow = builtin_presets().unwrap().into_iter().find(|p|p.material=="Willow").unwrap();
        assert!(willow.matches("willow"));
        assert!(willow.matches("trees bark"));
        assert!(!willow.matches("diamond"));
    }

    #[test]
    fn variant_seeds_are_deterministic_and_distinct() {
        let first = builtin_presets().unwrap();
        let second = builtin_presets().unwrap();
        assert_eq!(first[0].seed, second[0].seed);
        assert_ne!(first[0].seed, first[1].seed);
    }
}
