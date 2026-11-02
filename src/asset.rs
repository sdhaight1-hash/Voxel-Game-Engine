//! Standalone editable voxel assets. A procedural tree can become an ordinary
//! block model; edits never depend on re-running the original generator.
//!
//! Core data: integer cell positions + material slots, not an OBJ mesh.
//! The PBR graph can travel with the asset for continued surface design.

use crate::foliage::{FoliageMesh, FoliageProject, Material, VoxelCell, LeafRenderSettings};
use crate::voxel::VoxelShape;
use crate::Graph;
use crate::foliage_materials::FoliageMaterials;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct VoxelAsset {
    pub format_version: u32,
    pub name: String,
    pub shape: VoxelShape,
    pub voxel_size: f32,
    pub voxels: Vec<VoxelCell>,
    pub bark_color: [u8; 3],
    pub leaf_color: [u8; 3],
    #[serde(default)]
    pub leaf_render: LeafRenderSettings,
    #[serde(default)]
    pub materials: FoliageMaterials,
    /// Optional procedural surface material project. Kept editable in Nodes.
    pub material_graph: Option<Graph>,
}

impl Default for VoxelAsset {
    fn default() -> Self {
        Self {
            format_version: 1,
            name: "Untitled voxel asset".into(),
            shape: VoxelShape::Cube,
            voxel_size: 0.5,
            voxels: vec![VoxelCell { x: 0, y: 0, z: 0, material: Material::Bark }],
            bark_color: [125, 94, 68],
            leaf_color: [77, 142, 65],
            leaf_render: LeafRenderSettings::default(),
            materials: FoliageMaterials::default(),
            material_graph: None,
        }
    }
}

impl VoxelAsset {
    pub fn from_tree(project: &FoliageProject, mesh: &FoliageMesh, graph: &Graph) -> Self {
        Self {
            format_version: 1,
            name: format!("{} voxel tree", project.preset.name()),
            shape: mesh.shape,
            voxel_size: mesh.voxel_size,
            voxels: mesh.voxels.clone(),
            bark_color: project.bark_color,
            leaf_color: project.leaf_color,
            leaf_render: project.leaf_render,
            materials: project.materials.clone(),
            material_graph: Some(graph.clone()),
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.format_version != 1 {
            return Err("Unsupported voxel asset version".into());
        }
        if self.name.trim().is_empty() || self.name.len() > 120 {
            return Err("Asset name must be 1..120 characters".into());
        }
        self.mesh()?;
        self.materials.validate()?;
        if let Some(graph) = &self.material_graph { graph.validate()?; }
        Ok(())
    }

    pub fn mesh(&self) -> Result<FoliageMesh, String> {
        FoliageMesh::from_voxels_with_render(self.shape, self.voxel_size,
            self.voxels.clone(), self.leaf_render)
    }

    pub fn contains(&self, pos: [i32; 3]) -> bool {
        self.voxels.iter().any(|v| [v.x, v.y, v.z] == pos)
    }

    pub fn material_at(&self, pos: [i32; 3]) -> Option<Material> {
        self.voxels.iter().find(|v| [v.x, v.y, v.z] == pos).map(|v| v.material)
    }

    pub fn add(&mut self, pos: [i32; 3], material: Material) -> Result<(), String> {
        if pos.iter().any(|value| !(-128..=128).contains(value)) {
            return Err("Block position must be between -128 and 128".into());
        }
        if self.voxels.len() >= 40_000 {
            return Err("Asset limit is 40,000 occupied voxels".into());
        }
        if self.contains(pos) {
            return Err("A voxel already occupies that position".into());
        }
        self.voxels.push(VoxelCell { x: pos[0], y: pos[1], z: pos[2], material });
        Ok(())
    }

    /// Remove one actual occupied cell and return its material for future
    /// harvest/drop resolution. Visual leaf cards are not separate fake blocks.
    pub fn harvest_cell(&mut self, pos: [i32; 3]) -> Option<Material> {
        let i = self.voxels.iter().position(|v| [v.x, v.y, v.z] == pos)?;
        Some(self.voxels.remove(i).material)
    }

    pub fn erase(&mut self, pos: [i32; 3]) -> bool {
        self.harvest_cell(pos).is_some()
    }

    pub fn paint(&mut self, pos: [i32; 3], material: Material) -> bool {
        if let Some(voxel) = self.voxels.iter_mut()
            .find(|v| [v.x, v.y, v.z] == pos) {
            voxel.material = material;
            true
        } else { false }
    }

    pub fn move_voxel(&mut self, from: [i32; 3], to: [i32; 3])
        -> Result<(), String> {
        if from == to { return Ok(()); }
        let material = self.material_at(from).ok_or("Source cell is empty")?;
        self.add(to, material)?;
        self.erase(from);
        Ok(())
    }

    pub fn to_json(&self) -> Result<String, String> {
        self.validate()?;
        serde_json::to_string_pretty(self).map_err(|e| e.to_string())
    }

    pub fn from_json(text: &str) -> Result<Self, String> {
        let value: serde_json::Value =
            serde_json::from_str(text).map_err(|e| e.to_string())?;
        let asset = if value["format"].as_str() == Some("hexel-game-tree-v1") {
            if value["shape"].as_str() != Some("HexPrism") ||
                value["cell_dimensions_m"]["across_flats"].as_f64() != Some(1.0) ||
                value["cell_dimensions_m"]["height"].as_f64() != Some(1.0) {
                return Err("Tree asset must use standard 1m hexagonal-prism cells".into());
            }
            let project: FoliageProject = serde_json::from_value(value["settings"].clone())
                .map_err(|e| format!("Tree settings: {e}"))?;
            project.validate()?;
            let voxels: Vec<VoxelCell> = serde_json::from_value(value["voxels"].clone())
                .map_err(|e| format!("Tree cells: {e}"))?;
            Self {
                name: format!("{} voxel tree", project.preset.name()),
                shape: VoxelShape::HexPrism,
                voxel_size: 1.0,
                voxels,
                bark_color: project.bark_color,
                leaf_color: project.leaf_color,
                leaf_render: project.leaf_render,
                materials: project.materials,
                ..Self::default()
            }
        } else {
            serde_json::from_value(value).map_err(|e| e.to_string())?
        };
        asset.validate()?;
        Ok(asset)
    }

    /// A compatibility surface mesh; the JSON retains real editable blocks.
    pub fn obj(&self) -> Result<String, String> {
        Ok(self.mesh()?.obj().replace("mtllib foliage.mtl", "mtllib asset.mtl"))
    }

    pub fn mtl(&self) -> String {
        let mut pseudo_project = FoliageProject::default();
        pseudo_project.bark_color = self.bark_color;
        pseudo_project.leaf_color = self.leaf_color;
        pseudo_project.leaf_render = self.leaf_render;
        pseudo_project.materials = self.materials.clone();
        crate::foliage::mtl(&pseudo_project)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::foliage::PlantPreset;

    #[test]
    fn add_move_paint_erase_and_occupied_validation() {
        let mut asset = VoxelAsset::default();
        assert!(asset.add([0, 0, 0], Material::Leaf).is_err());
        asset.add([0, 0, 1], Material::Leaf).unwrap();
        assert!(asset.move_voxel([0, 0, 0], [0, 0, 1]).is_err());
        assert_eq!(asset.material_at([0, 0, 0]), Some(Material::Bark));
        asset.move_voxel([0, 0, 1], [1, 0, 1]).unwrap();
        assert!(!asset.contains([0, 0, 1]));
        asset.paint([1, 0, 1], Material::LeafDark);
        assert_eq!(asset.material_at([1, 0, 1]), Some(Material::LeafDark));
        assert!(asset.erase([1, 0, 1]));
        assert!(!asset.erase([1, 0, 1]));
        assert!(asset.add([129, 0, 0], Material::Bark).is_err());
    }

    #[test]
    fn generated_tree_is_editable_1m_hexel_asset() {
        let project = FoliageProject::from_preset(PlantPreset::Willow);
        let tree = project.generate().unwrap();
        let mut asset = VoxelAsset::from_tree(&project, &tree, &Graph::default());
        assert_eq!(asset.shape, VoxelShape::HexPrism);
        assert_eq!(asset.voxel_size, 1.0);
        assert_eq!(asset.voxels.len(), tree.voxels.len());
        assert!(asset.material_graph.is_some());
        let first = asset.voxels[0];
        asset.erase([first.x, first.y, first.z]);
        assert_eq!(asset.voxels.len() + 1, tree.voxels.len());
        let decoded = VoxelAsset::from_json(&asset.to_json().unwrap()).unwrap();
        assert_eq!(decoded.voxels.len(), asset.voxels.len());
        assert!(decoded.mesh().unwrap().triangle_count() > 0);
        assert!(decoded.obj().unwrap().contains("mtllib asset.mtl"));
    }

    #[test]
    fn world_hexel_tree_has_removable_wood_and_foliage_cells() {
        let project = FoliageProject::from_preset(PlantPreset::Oak);
        let mesh = project.generate().unwrap();
        let mut asset = VoxelAsset::from_tree(&project, &mesh, &Graph::default());
        assert_eq!(asset.shape, VoxelShape::HexPrism);
        assert_eq!(asset.voxel_size, 1.0);
        assert_eq!(asset.leaf_render.geometry, LeafRenderSettings::default().geometry);
        for wood in [true, false] {
            let cell = asset.voxels.iter().copied()
                .find(|v| v.material.is_wood() == wood).unwrap();
            let pos = [cell.x, cell.y, cell.z];
            let harvested = asset.harvest_cell(pos).unwrap();
            assert_eq!(harvested.is_wood(), wood);
            assert!(!asset.contains(pos));
            assert_eq!(asset.harvest_cell(pos), None);
        }
        assert!(VoxelAsset::from_json(&asset.to_json().unwrap()).is_ok());
    }

    #[test]
    fn asset_file_rejects_duplicates_and_invalid_settings() {
        let mut asset = VoxelAsset::default();
        asset.voxels.push(asset.voxels[0]);
        assert!(asset.to_json().is_err());
        asset.voxels.pop();
        asset.voxel_size = f32::NAN;
        assert!(asset.to_json().is_err());
        asset.voxel_size = 0.5;
        asset.name = " ".into();
        assert!(asset.to_json().is_err());
    }

    #[test]
    fn game_tree_export_imports_into_editable_hexel_asset() {
        let project = FoliageProject::from_preset(PlantPreset::Oak);
        let mesh = project.generate().unwrap();
        let json = mesh.voxel_json(&project).unwrap();
        let mut asset = VoxelAsset::from_json(&json).unwrap();
        assert_eq!(asset.shape, VoxelShape::HexPrism);
        assert_eq!(asset.voxel_size, 1.0);
        assert_eq!(asset.voxels.len(), mesh.voxels.len());
        let cell = asset.voxels[0];
        assert_eq!(asset.harvest_cell([cell.x, cell.y, cell.z]), Some(cell.material));
        assert!(VoxelAsset::from_json(&asset.to_json().unwrap()).is_ok());
    }

    #[test]
    fn adjoining_voxels_cull_internal_faces() {
        let mut asset = VoxelAsset::default();
        for shape in [VoxelShape::Cube, VoxelShape::HexPrism] {
            asset.shape = shape;
            let single = asset.mesh().unwrap().triangle_count();
            asset.add([0, 0, 1], Material::Leaf).unwrap();
            assert!(asset.mesh().unwrap().triangle_count() < single * 2);
            asset.erase([0, 0, 1]);
        }
    }
}
