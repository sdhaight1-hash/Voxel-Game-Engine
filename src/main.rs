use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, TextureHandle, Vec2};
use std::{collections::HashMap, path::{Path, PathBuf}};
use texture_studio::{Graph, NodeKind, LibraryNode, VoxelShape, SideMapping};
use texture_studio::presets::{builtin_presets, Preset};
use texture_studio::foliage::{FoliageProject, FoliageMesh, FoliageStage, PlantPreset, Material};
use texture_studio::asset::VoxelAsset;
use texture_studio::voxel::{render_atlas, render_voxel_preview, export_obj};

const NODE_W: f32 = 170.0;
const PREVIEW_SIZE: u32 = 256;

#[derive(Clone, Copy, PartialEq, Eq)]
enum AssetTool { Select, Add, Erase, Paint }

fn hit_triangle(p: Pos2, a: Pos2, b: Pos2, c: Pos2) -> bool {
    let cross = |u: Pos2, v: Pos2, w: Pos2| -> f32 {
        (v.x - u.x) * (w.y - u.y) - (v.y - u.y) * (w.x - u.x)
    };
    let x = cross(a,b,p); let y = cross(b,c,p); let z = cross(c,a,p);
    (x >= 0.0 && y >= 0.0 && z >= 0.0) ||
        (x <= 0.0 && y <= 0.0 && z <= 0.0)
}


#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Port {
    Output(u64),
    Input(u64, usize),
}

#[derive(Clone, Copy, Debug)]
struct WireGesture {
    start: Port,
    // Preserve the existing cable until a new connection has passed validation.
    original: Option<(u64, usize, u64)>, // to, input, from
    pressed_at: Pos2,
    dragging: bool,
}

struct Studio {
    graph: Graph,
    foliage: FoliageProject,
    foliage_mesh: Option<FoliageMesh>,
    foliage_dirty: bool,
    foliage_stage: FoliageStage,
    foliage_yaw: f32,
    asset: VoxelAsset,
    asset_mesh: Option<FoliageMesh>,
    asset_dirty: bool,
    asset_tool: AssetTool,
    asset_material: Material,
    asset_selected: Option<[i32; 3]>,
    asset_cursor: [i32; 3],
    asset_yaw: f32,
    asset_zoom: f32,
    asset_path: Option<PathBuf>,
    asset_undo: Vec<VoxelAsset>,
    asset_redo: Vec<VoxelAsset>,
    selected: Option<u64>,
    connecting: Option<u64>,
    wire_gesture: Option<WireGesture>,
    preview: Option<TextureHandle>,
    selected_preview: Option<TextureHandle>,
    previewed_id: Option<u64>,
    voxel_preview: Option<TextureHandle>,
    atlas_preview: Option<TextureHandle>,
    voxel_yaw: f32,
    thumbs: HashMap<u64, TextureHandle>,
    preview_channel: usize,
    view: u8,
    pan: Vec2,
    zoom: f32,
    library_search: String,
    library_tab: u8,
    preset_search: String,
    preset_category: String,
    preset_selected: Option<usize>,
    presets: Vec<Preset>,
    dirty: bool,
    export_size: u32,
    project_path: Option<PathBuf>,
    status: String,
}

impl Default for Studio {
    fn default() -> Self {
        Self {
            graph: Graph::demo_bricks(),
            foliage: FoliageProject::default(),
            foliage_mesh: None,
            foliage_dirty: true,
            foliage_stage: FoliageStage::PresetLoader,
            foliage_yaw: 0.8,
            asset: VoxelAsset::default(),
            asset_mesh: None,
            asset_dirty: true,
            asset_tool: AssetTool::Select,
            asset_material: Material::Bark,
            asset_selected: Some([0, 0, 0]),
            asset_cursor: [0, 0, 1],
            asset_yaw: 0.8,
            asset_zoom: 1.0,
            asset_path: None,
            asset_undo: Vec::new(),
            asset_redo: Vec::new(),
            selected: Some(2),
            connecting: None,
            wire_gesture: None,
            preview: None,
            selected_preview: None,
            previewed_id: None,
            voxel_preview: None,
            atlas_preview: None,
            voxel_yaw: 0.82,
            thumbs: HashMap::new(),
            preview_channel: 0,
            view: 0,
            pan: Vec2::ZERO,
            zoom: 1.0,
            library_search: String::new(),
            library_tab: 1,
            preset_search: String::new(),
            preset_category: "All categories".into(),
            preset_selected: None,
            presets: builtin_presets().expect("bundled preset catalog is checked by tests"),
            dirty: true,
            export_size: 1024,
            project_path: None,
            status: "Drag output to input to connect. Drag an existing cable to rewire it.".to_owned(),
        }
    }
}

impl Studio {
    fn refresh_preview(&mut self, ctx: &egui::Context) {
        if !self.dirty && self.previewed_id == self.selected { return; }
        self.dirty = false;
        self.previewed_id = self.selected;
        // Only render the visible preview. With the expanded preset graphs,
        // computing every PBR channel, a voxel and a full atlas on each edit
        // blocks the native UI for no benefit.
        match self.view {
            0 if !self.graph.voxel.geometryless => {
                match render_voxel_preview(&self.graph, self.graph.voxel, PREVIEW_SIZE, self.voxel_yaw) {
                    Ok(pixels) => put_texture(ctx, &mut self.voxel_preview,
                        "voxel-preview", pixels, PREVIEW_SIZE),
                    Err(err) => self.status = format!("Voxel preview error: {err}"),
                }
            }
            1 => {
                let rendered = if self.preview_channel == 4 {
                    self.graph.render_normal_rgba(PREVIEW_SIZE, PREVIEW_SIZE)
                } else {
                    self.graph.render_output_channel_rgba(self.preview_channel, PREVIEW_SIZE, PREVIEW_SIZE)
                };
                match rendered {
                    Ok(pixels) => put_texture(ctx, &mut self.preview,
                        "output-preview", pixels, PREVIEW_SIZE),
                    Err(err) => self.status = format!("Preview error: {err}"),
                }
            }
            2 => {
                if let Some(id) = self.selected {
                    if let Ok(pixels) = self.graph.render_node_rgba(id, PREVIEW_SIZE, PREVIEW_SIZE) {
                        put_texture(ctx, &mut self.selected_preview,
                            "selected-preview", pixels, PREVIEW_SIZE);
                    }
                } else {
                    self.selected_preview = None;
                }
            }
            3 if !self.graph.voxel.geometryless => {
                match render_atlas(&self.graph, self.preview_channel, self.graph.voxel, 64) {
                    Ok((meta, pixels)) => put_texture_rect(ctx, &mut self.atlas_preview,
                        "voxel-atlas", pixels, [meta.width as usize, meta.height as usize]),
                    Err(err) => self.status = format!("Voxel atlas error: {err}"),
                }
            }
            _ => {}
        }
        self.thumbs.retain(|id, _| self.graph.node(*id).is_some());
        for node in &self.graph.nodes {
            if matches!(node.kind, NodeKind::Output) { continue; }
            if let Ok(pixels) = self.graph.render_node_rgba(node.id, 64, 64) {
                let image = egui::ColorImage::from_rgba_unmultiplied([64, 64], &pixels);
                if let Some(texture) = self.thumbs.get_mut(&node.id) {
                    texture.set(image, egui::TextureOptions::LINEAR);
                } else {
                    self.thumbs.insert(node.id, ctx.load_texture(
                        format!("node-thumb-{}", node.id), image, egui::TextureOptions::LINEAR
                    ));
                }
            }
        }
    }

    fn new_project(&mut self) {
        self.graph = Graph::default();
        self.selected = Some(1);
        self.connecting = None;
        self.wire_gesture = None;
        self.pan = Vec2::ZERO;
        self.zoom = 1.0;
        self.project_path = None;
        self.dirty = true;
        self.status = "New project created".into();
    }

    fn open_project(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Texture Studio project", &["json"])
            .pick_file() else { return; };
        let result = std::fs::read_to_string(&path)
            .map_err(|e| e.to_string())
            .and_then(|data| serde_json::from_str::<Graph>(&data).map_err(|e| e.to_string()))
            .and_then(|graph| { graph.validate()?; Ok(graph) });
        match result {
            Ok(graph) => {
                self.graph = graph;
                self.selected = None;
                self.connecting = None;
                self.wire_gesture = None;
                self.pan = Vec2::ZERO;
                self.zoom = 1.0;
                self.dirty = true;
                self.project_path = Some(path);
                self.status = "Project opened".into();
            }
            Err(err) => self.status = format!("Could not open project: {err}"),
        }
    }

    fn save_project(&mut self, save_as: bool) {
        let path = if !save_as { self.project_path.clone() } else { None }
            .or_else(|| rfd::FileDialog::new()
                .add_filter("Texture Studio project", &["json"])
                .set_file_name("project.texture.json")
                .save_file());
        let Some(path) = path else { return; };
        let result = self.graph.validate()
            .and_then(|_| serde_json::to_string_pretty(&self.graph).map_err(|e| e.to_string()))
            .and_then(|json| std::fs::write(&path, json).map_err(|e| e.to_string()));
        match result {
            Ok(()) => {
                self.project_path = Some(path);
                self.status = "Project saved".into();
            }
            Err(err) => self.status = format!("Could not save project: {err}"),
        }
    }

    fn export_png(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("PNG image", &["png"])
            .set_file_name("texture.png")
            .save_file() else { return; };
        let size = self.export_size;
        let result = (if self.preview_channel == 4 {
            self.graph.render_normal_rgba(size, size)
        } else {
            self.graph.render_output_channel_rgba(self.preview_channel, size, size)
        }).and_then(|pixels| save_png(&path, size, &pixels));
        self.status = match result {
            Ok(()) => format!("PNG exported: {}", path.display()),
            Err(err) => format!("Export failed: {err}"),
        };
    }

    fn export_material(&mut self) {
        let Some(folder) = rfd::FileDialog::new().pick_folder() else { return; };
        let size = self.export_size;
        let result: Result<(), String> = (|| {
            for name in ["albedo.png", "roughness.png", "metallic.png", "height.png", "normal.png"] {
                if folder.join(name).exists() {
                    return Err(format!("{name} already exists in that folder — choose an empty folder"));
                }
            }
            for (channel, name) in ["albedo", "roughness", "metallic", "height"].iter().enumerate() {
                let pixels = self.graph.render_output_channel_rgba(channel, size, size)?;
                save_png(&folder.join(format!("{name}.png")), size, &pixels)?;
            }
            let normals = self.graph.render_normal_rgba(size, size)?;
            save_png(&folder.join("normal.png"), size, &normals)?;
            Ok(())
        })();
        self.status = match result {
            Ok(()) => format!("Five PBR maps exported to {}", folder.display()),
            Err(err) => format!("Material export failed: {err}"),
        };
    }

    fn export_voxel(&mut self) {
        if self.graph.voxel.geometryless {
            self.status = "Geometryless mode exports square 2D maps. Use Export PBR maps.".into();
            return;
        }
        let Some(folder) = rfd::FileDialog::new().pick_folder() else { return; };
        let size = self.export_size;
        let settings = self.graph.voxel;
        let result: Result<(), String> = (|| {
            let channels = ["albedo", "roughness", "metallic", "height", "normal"];
            for filename in [
                "voxel_albedo.png", "voxel_roughness.png", "voxel_metallic.png",
                "voxel_height.png", "voxel_normal.png",
                "voxel_layout.json", "voxel.obj", "voxel.mtl",
            ] {
                if folder.join(filename).exists() {
                    return Err(format!("{filename} already exists — choose an empty folder"));
                }
            }
            let mut metadata = None;
            for (channel, name) in channels.iter().enumerate() {
                let (atlas, pixels) = render_atlas(&self.graph, channel, settings, size)?;
                save_png_rect(&folder.join(format!("voxel_{name}.png")),
                    atlas.width, atlas.height, &pixels)?;
                metadata = Some(atlas);
            }
            let atlas = metadata.ok_or("Could not create voxel atlas")?;
            std::fs::write(folder.join("voxel_layout.json"),
                serde_json::to_string_pretty(&atlas).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
            std::fs::write(folder.join("voxel.obj"), export_obj(&atlas, "voxel.mtl"))
                .map_err(|e| e.to_string())?;
            std::fs::write(folder.join("voxel.mtl"),
                "newmtl voxel_material\nKd 1.0 1.0 1.0\nmap_Kd voxel_albedo.png\n")
                .map_err(|e| e.to_string())?;
            Ok(())
        })();
        self.status = match result {
            Ok(()) => format!("Exported {} voxel with five PBR atlases and UV-mapped OBJ to {}",
                settings.shape.name(), folder.display()),
            Err(err) => format!("Voxel export failed: {err}"),
        };
    }

    fn demo_material(&mut self) {
        self.graph = Graph::demo_bricks();
        self.selected = Some(2);
        self.connecting = None;
        self.wire_gesture = None;
        self.pan = Vec2::ZERO;
        self.zoom = 1.0;
        self.project_path = None;
        self.dirty = true;
        self.status = "Loaded editable bricks material example".into();
    }

    fn spawn(&mut self, kind: NodeKind) {
        let offset = self.graph.nodes.len() as f32 * 24.0;
        let id = self.graph.add_node(kind, [80.0 + offset, 235.0 + offset]);
        self.selected = Some(id);
        self.dirty = true;
        self.status = format!("Added {} node", kind.name());
    }

    fn foliage_stage_list(&mut self, ui: &mut egui::Ui) {
        ui.label(egui::RichText::new("PROCEDURAL VEGETATION").strong());
        ui.small("Build real 3D cube/hex voxels for bark, branches and foliage.");
        ui.separator();
        for stage in FoliageStage::ALL {
            if stage == FoliageStage::PresetLoader ||
                (stage == FoliageStage::MeshBuilder) ||
                (stage == FoliageStage::FoliagePalette) ||
                (stage == FoliageStage::Output) {
                ui.small(stage.group());
            }
            if ui.selectable_label(self.foliage_stage == stage,stage.name()).clicked() {
                self.foliage_stage = stage;
            }
        }
        ui.separator();
        ui.small("Select a stage to edit parameters on the right.");
        ui.small("Stages fill an occupied block grid. Export blocks as JSON, plus OBJ/MTL.");
        ui.small("Not Unreal Nanite, skeleton animation or world scattering.");
    }

    fn save_foliage(&mut self) {
        let Some(path) = rfd::FileDialog::new().add_filter("Foliage project",&["json"])
            .set_file_name("tree.foliage.json").save_file() else { return; };
        let result = self.foliage.validate()
            .and_then(|_|serde_json::to_string_pretty(&self.foliage).map_err(|e|e.to_string()))
            .and_then(|json|std::fs::write(&path,json).map_err(|e|e.to_string()));
        self.status = match result {
            Ok(()) => format!("Saved foliage settings: {}",path.display()),
            Err(e) => format!("Save foliage failed: {e}"),
        };
    }

    fn open_foliage(&mut self) {
        let Some(path) = rfd::FileDialog::new().add_filter("Foliage project",&["json"])
            .pick_file() else { return; };
        let result = std::fs::read_to_string(&path).map_err(|e|e.to_string())
            .and_then(|text|serde_json::from_str::<FoliageProject>(&text).map_err(|e|e.to_string()))
            .and_then(|project| {project.validate()?; Ok(project)});
        match result {
            Ok(project) => {
                self.foliage = project;
                self.foliage_dirty = true;
                self.status = format!("Opened foliage: {}",path.display());
            }
            Err(e) => self.status = format!("Open foliage failed: {e}"),
        }
    }

    fn export_foliage(&mut self) {
        let Some(folder) = rfd::FileDialog::new().pick_folder() else { return; };
        let result: Result<(),String> = (||{
            let names=["foliage.voxels.json","foliage.obj","foliage.mtl","foliage.json"];
            for name in names {
                if folder.join(name).exists() {
                    return Err(format!("{name} already exists; choose an empty folder"));
                }
            }
            let mesh = self.foliage.generate()?;
            let project = serde_json::to_string_pretty(&self.foliage)
                .map_err(|e|e.to_string())?;
            let files=[
                ("foliage.voxels.json", mesh.voxel_json(&self.foliage)?),
                ("foliage.obj",mesh.obj()),
                ("foliage.mtl",texture_studio::foliage::mtl(&self.foliage)),
                ("foliage.json",project),
            ];
            for (name,contents) in files {
                std::fs::write(folder.join(name),contents).map_err(|e|e.to_string())?;
            }
            Ok(())
        })();
        self.status = match result {
            Ok(()) => format!("Exported voxel blocks, culled OBJ surface, materials and settings to {}",folder.display()),
            Err(err) => format!("Foliage export failed: {err}"),
        };
    }

    fn foliage_inspector(&mut self, ui: &mut egui::Ui) {
        ui.heading("TREE DESIGNER");
        ui.horizontal(|ui| {
            ui.label("Plant");
            egui::ComboBox::from_id_salt("foliage_species")
                .selected_text(self.foliage.preset.name())
                .show_ui(ui, |ui| {
                    for species in PlantPreset::ALL {
                        if ui.selectable_label(self.foliage.preset == species,species.name()).clicked() {
                            self.foliage = FoliageProject::from_preset(species);
                            self.foliage_dirty = true;
                        }
                    }
                });
        });
        ui.separator();
        ui.label(egui::RichText::new(self.foliage_stage.name()).strong());
        let tree=&mut self.foliage;
        let mut changed=false;
        match self.foliage_stage {
            FoliageStage::PresetLoader => {
                changed |= ui.add(egui::DragValue::new(&mut tree.seed).prefix("Seed: ")).changed();
                changed |= ui.add(egui::Slider::new(&mut tree.height,0.5..=30.0).text("Height")).changed();
                changed |= ui.add(egui::Slider::new(&mut tree.branch_count,1..=90).text("Branch count")).changed();
                changed |= ui.add(egui::Slider::new(&mut tree.branch_length,0.1..=1.5).text("Branch length")).changed();
                ui.small("Voxelize the selected species and branch layout at the grid resolution.");
            }
            FoliageStage::Carve => {
                changed |= ui.add(egui::Slider::new(&mut tree.carve_bottom,0.0..=0.85).text("Clear lower canopy")).changed();
                changed |= ui.add(egui::Slider::new(&mut tree.carve_top,0.0..=0.85).text("Trim upper canopy")).changed();
                ui.small("Carve removes branch growing zones from the top and bottom.");
            }
            FoliageStage::Gravity => {
                changed |= ui.add(egui::Slider::new(&mut tree.gravity,-1.0..=1.0).text("Gravity / droop")).changed();
                changed |= ui.add(egui::Slider::new(&mut tree.phototropism,0.0..=1.0).text("Phototropism")).changed();
                ui.small("Gravity bends branches down, phototropism raises them toward light.");
            }
            FoliageStage::Slope => {
                changed |= ui.add(egui::Slider::new(&mut tree.slope,-0.8..=0.8).text("Trunk lean")).changed();
                ui.small("Leans the trunk; useful for sloped terrain and windswept growth.");
            }
            FoliageStage::Scale => {
                changed |= ui.add(egui::Slider::new(&mut tree.overall_scale,0.1..=5.0).text("Overall scale")).changed();
            }
            FoliageStage::RemoveBranches => {
                changed |= ui.add(egui::Slider::new(&mut tree.prune,0.0..=1.0).text("Prune probability")).changed();
                ui.small("Deterministic branch removal using the current seed.");
            }
            FoliageStage::MeshBuilder => {
                ui.label("Voxel block shape");
                egui::ComboBox::from_id_salt("tree_voxel_shape")
                    .selected_text(tree.shape.name())
                    .show_ui(ui, |ui| {
                        changed |= ui.selectable_value(&mut tree.shape,
                            VoxelShape::Cube, "Cube voxels").changed();
                        changed |= ui.selectable_value(&mut tree.shape,
                            VoxelShape::HexPrism, "Hexagonal prism voxels").changed();
                    });
                changed |= ui.add(egui::Slider::new(&mut tree.voxel_size,
                    0.1..=1.5).text("Voxel cell size")).changed();
                changed |= ui.add(egui::Slider::new(&mut tree.trunk_radius,
                    0.02..=2.0).text("Trunk width")).changed();
                ui.small("Cells are discrete occupied cubes or axial hex prisms.");
                ui.small("Smaller cells = finer detail and more blocks; internal faces are culled.");
            }
            FoliageStage::FoliagePalette => {
                ui.label("Bark color");
                changed |= ui.color_edit_button_srgb(&mut tree.bark_color).changed();
                ui.label("Leaf color");
                changed |= ui.color_edit_button_srgb(&mut tree.leaf_color).changed();
                ui.small("Material slots: bark + dark, medium and light leaves.");
            }
            FoliageStage::FoliageDistributor => {
                changed |= ui.add(egui::Slider::new(&mut tree.leaf_density,0.0..=1.0).text("Density")).changed();
                changed |= ui.add(egui::Slider::new(&mut tree.leaf_scale,0.02..=1.5).text("Leaf size")).changed();
                changed |= ui.add(egui::Slider::new(&mut tree.leaf_angle,-1.0..=1.0).text("Leaf orientation")).changed();
                changed |= ui.add(egui::Slider::new(&mut tree.leaf_variation,0.0..=1.0).text("Size variation")).changed();
                ui.small("Grows volumetric leaf voxel clusters around branch tips.");
            }
            FoliageStage::Output => {
                if let Some(mesh)=&self.foliage_mesh {
                    ui.label(format!("{} occupied voxels",mesh.voxel_count()));
                    ui.label(format!("{} exposed-surface triangles",mesh.triangle_count()));
                    ui.label(format!("Voxel type: {}",mesh.shape.name()));
                    ui.label(format!("Cell size: {:.2}",mesh.voxel_size));
                    ui.label(format!("Nominal height: {:.1}",mesh.height));
                }
                ui.small("Exports each occupied block and material in foliage.voxels.json.");
                ui.small("Also exports the culled cube/hex surface as OBJ with MTL.");
                if ui.button("Export voxel tree").clicked() { self.export_foliage(); }
                if ui.button("Save foliage project").clicked() { self.save_foliage(); }
                if ui.button("Open foliage project").clicked() { self.open_foliage(); }
            }
        }
        if changed { self.foliage_dirty = true; }
        if let Err(err)=self.foliage.validate() {
            ui.colored_label(Color32::from_rgb(232,133,104),err);
        }
        ui.separator();
        ui.small("This voxel tree is separate from the procedural 2D material graph.");
    }

    fn foliage_canvas(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading("VOXEL TREE PREVIEW");
            ui.separator();
            ui.small("Occupied cube / hex cells • adjustable growth stages");
        });
        ui.add(egui::Slider::new(&mut self.foliage_yaw,
            0.0..=std::f32::consts::TAU).text("Orbit"));
        let available=ui.available_size();
        let desired=Vec2::new(available.x.max(50.0),available.y.max(250.0));
        let (rect,_) = ui.allocate_exact_size(desired,Sense::hover());
        let painter=ui.painter_at(rect);
        painter.rect_filled(rect,4.0,Color32::from_rgb(22,27,35));
        let Some(mesh)=&self.foliage_mesh else {
            painter.text(rect.center(),Align2::CENTER_CENTER,
                "Invalid foliage settings — adjust parameters",
                FontId::proportional(15.0),Color32::LIGHT_GRAY);
            return;
        };
        let yaw=self.foliage_yaw;
        let right=[yaw.cos(),yaw.sin(),0.0];
        let elevation=0.35_f32;
        let up=[-yaw.sin()*elevation.sin(),yaw.cos()*elevation.sin(),elevation.cos()];
        let camera=[-yaw.sin()*elevation.cos(),yaw.cos()*elevation.cos(),elevation.sin()];
        let dot=|p:[f32;3],axis:[f32;3]|p[0]*axis[0]+p[1]*axis[1]+p[2]*axis[2];
        let h=mesh.height.max(0.1);
        let scale=(rect.width()/(h*2.5)).min(rect.height()/(h*1.35));
        let project=|p:[f32;3]| Pos2::new(
            rect.center().x+dot(p,right)*scale,
            rect.bottom()-rect.height()*0.14-dot(p,up)*scale
        );
        let origin=project([0.0,0.0,0.0]);
        painter.line_segment([origin+Vec2::new(-rect.width()*0.2,0.0),
            origin+Vec2::new(rect.width()*0.2,0.0)],
            Stroke::new(1.0,Color32::from_gray(65)));
        let mut faces:Vec<(f32,usize)> = mesh.triangles.iter().enumerate()
            .map(|(i,t)|{
                let center=std::array::from_fn(|c|
                    (t.points[0][c]+t.points[1][c]+t.points[2][c])/3.0);
                (dot(center,camera),i)
            }).collect();
        faces.sort_by(|a,b|a.0.total_cmp(&b.0));
        let bark=self.foliage.bark_color;
        let leaf=self.foliage.leaf_color;
        // A bounded rasterized preview avoids flooding egui with tiny faces.
        // Exports always use the complete voxel surface, never this decimation.
        let stride = (faces.len() / 16000).max(1);
        for (_,index) in faces.into_iter().step_by(stride) {
            let tri=&mesh.triangles[index];
            let base=match tri.material {
                Material::Bark=>bark,
                Material::LeafDark=>leaf.map(|c|(c as f32*0.72) as u8),
                Material::Leaf=>leaf,
                Material::LeafLight=>leaf.map(|c|((c as f32*1.22).min(255.0)) as u8),
            };
            let shade=0.75+0.25*(index as f32*0.017).sin().abs();
            let color=Color32::from_rgb(
                (base[0] as f32*shade) as u8,
                (base[1] as f32*shade) as u8,
                (base[2] as f32*shade) as u8);
            painter.add(egui::Shape::convex_polygon(vec![
                project(tri.points[0]),project(tri.points[1]),project(tri.points[2])],
                color,Stroke::NONE));
        }
        painter.text(rect.left_top()+Vec2::new(12.0,12.0),Align2::LEFT_TOP,
            format!("{} • {} {} voxels • {} surface triangles",
                self.foliage.preset.name(),mesh.voxel_count(),mesh.shape.name(),
                mesh.triangle_count()),
            FontId::proportional(14.0),Color32::LIGHT_GRAY);
    }

    fn palette(&mut self, ui: &mut egui::Ui) {
        ui.heading("LIBRARY");
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.library_tab, 1, "Presets");
            ui.selectable_value(&mut self.library_tab, 0, "Nodes");
            ui.selectable_value(&mut self.library_tab, 2, "Foliage");
            ui.selectable_value(&mut self.library_tab, 3, "Assets");
        });
        ui.separator();
        if self.library_tab == 1 {
            self.preset_browser(ui);
            return;
        }
        if self.library_tab == 2 {
            self.foliage_stage_list(ui);
            return;
        }
        if self.library_tab == 3 {
            self.asset_outliner(ui);
            return;
        }
        ui.add(egui::TextEdit::singleline(&mut self.library_search).hint_text("Search nodes..."));
        ui.separator();
        let query = self.library_search.trim().to_lowercase();
        egui::ScrollArea::vertical().max_height(430.0).id_salt("node_library_scroll").show(ui, |ui| {
        for (section, entries) in [
            ("GENERATORS", vec![
                ("Noise", NodeKind::Noise { scale: 6, octaves: 4, seed: 42 }),
                ("Checker", NodeKind::Checker { tiles: 8 }),
                ("Bricks", NodeKind::Bricks { columns: 8, rows: 12, mortar: 0.12 }),
                ("Voronoi", NodeKind::Voronoi { cells: 7, seed: 17 }),
                ("Strata", NodeKind::Strata { bands: 14, warp: 0.25, seed: 11 }),
                ("Color", NodeKind::Color { rgb: [98, 152, 203] }),
            ]),
            ("FILTERS", vec![
                ("Invert", NodeKind::Invert),
                ("Colorize", NodeKind::Colorize { dark: [24, 37, 54], light: [204, 175, 130] }),
            ]),
            ("MIX", vec![
                ("Blend", NodeKind::Blend { factor: 0.5 }),
            ]),
        ] {
            if entries.iter().any(|(name, _)| name.to_lowercase().contains(&query)) {
                ui.label(egui::RichText::new(section).strong().color(Color32::from_rgb(165, 174, 193)));
                for (name, kind) in entries {
                    if name.to_lowercase().contains(&query) && ui.button(format!("＋ {name}")).clicked() {
                        self.spawn(kind);
                    }
                }
                ui.add_space(8.0);
            }
        }
        // The full Material Maker-inspired 2D node set. Each entry is a
        // functional Rust evaluator, not a decorative placeholder.
        for category in [
            "Shapes & Gradients", "Noise", "Patterns", "Color & Tones",
            "Filters", "Math & Mix", "Transforms",
        ] {
            let matching: Vec<LibraryNode> = LibraryNode::ALL.iter().copied()
                .filter(|operation| operation.category() == category &&
                    operation.name().to_lowercase().contains(&query))
                .collect();
            if matching.is_empty() { continue; }
            ui.label(egui::RichText::new(format!("{category} ({})", matching.len()))
                .strong().color(Color32::from_rgb(165, 174, 193)));
            for operation in matching {
                if ui.button(format!("＋ {}", operation.name())).clicked() {
                    self.spawn(NodeKind::Library { operation, amount: 0.5, seed: 42 });
                }
            }
            ui.add_space(8.0);
        }
        });
        ui.separator();
        if ui.button("Load bricks example").clicked() { self.demo_material(); }
        ui.add_space(12.0);
        ui.heading("CONTROLS");
        ui.small("Drag headers: move nodes");
        ui.small("Middle mouse drag: pan graph");
        ui.small("Mouse wheel: zoom graph");
        ui.small("Drag output → input: connect");
        ui.small("Drag input / cable: rewire");
        ui.small("Or click output then input");
        ui.small("Right-click input: disconnect");
        ui.small("Delete node in Inspector");
        if let Some(id) = self.connecting {
            ui.colored_label(Color32::LIGHT_GREEN, format!("Wiring from #{id}"));
            if ui.button("Cancel wire").clicked() {
                self.connecting = None;
                self.wire_gesture = None;
            }
        }
    }

    fn apply_preset(&mut self) {
        let Some(index) = self.preset_selected else { return; };
        let Some(preset) = self.presets.get(index) else { return; };
        let title = preset.title();
        // Shape/side mapping are project settings and must survive changing materials.
        self.graph = preset.build_graph(self.graph.voxel);
        self.selected = Some(4); // Colorize is the primary editable color stage.
        self.connecting = None;
        self.wire_gesture = None;
        self.project_path = None;
        self.pan = Vec2::ZERO;
        self.zoom = 0.7;
        self.dirty = true;
        self.status = format!("Applied '{title}' · editable procedural approximation. Save as a new project.");
    }

    fn preset_browser(&mut self, ui: &mut egui::Ui) {
        ui.label(egui::RichText::new(format!("{} natural presets", self.presets.len())).strong());
        ui.add(egui::TextEdit::singleline(&mut self.preset_search)
            .hint_text("Find basalt, willow, snow..."));
        let mut categories: Vec<&str> = self.presets.iter().map(|p| p.category.as_str()).collect();
        categories.sort_unstable();
        categories.dedup();
        egui::ComboBox::from_id_salt("preset_category")
            .selected_text(&self.preset_category)
            .width(ui.available_width())
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut self.preset_category,
                    "All categories".to_owned(), "All categories");
                for category in &categories {
                    ui.selectable_value(&mut self.preset_category,
                        (*category).to_owned(), *category);
                }
            });
        let indices: Vec<usize> = self.presets.iter().enumerate()
            .filter(|(_, p)| (self.preset_category == "All categories" ||
                self.preset_category == p.category) &&
                p.matches(&self.preset_search))
            .map(|(index, _)| index)
            .collect();
        ui.small(format!("{} matches · select for details", indices.len()));
        ui.separator();
        egui::ScrollArea::vertical().max_height(325.0)
            .id_salt("preset_list")
            .show_rows(ui, 26.0, indices.len(), |ui, visible| {
                for row in visible {
                    let idx = indices[row];
                    if ui.selectable_label(self.preset_selected == Some(idx),
                        self.presets[idx].title()).clicked() {
                        self.preset_selected = Some(idx);
                    }
                }
            });
        ui.separator();
        if let Some(index) = self.preset_selected {
            if let Some(preset) = self.presets.get(index) {
                ui.label(egui::RichText::new(preset.material.clone()).strong());
                ui.small(format!("{} / {}", preset.category, preset.family));
                ui.small(format!("Appearance: {}", preset.variant));
                let (dark, light) = preset.colors();
                ui.horizontal(|ui| {
                    ui.colored_label(Color32::from_rgb(dark[0],dark[1],dark[2]), "■ Dark");
                    ui.colored_label(Color32::from_rgb(light[0],light[1],light[2]), "■ Light");
                });
            }
            ui.add_space(6.0);
            if ui.button("Apply selected preset").clicked() {
                self.apply_preset();
            }
            ui.small("Replaces the current node graph. Save your project first.");
        } else {
            ui.small("Select any material to inspect it, then click Apply.");
        }
        ui.add_space(4.0);
        ui.small("Procedural approximations, not source-game PNGs or plant meshes.");
    }

    fn inspector(&mut self, ui: &mut egui::Ui) {
        ui.heading("Node settings");
        let Some(id) = self.selected else {
            ui.label("Select a node on the canvas.");
            return;
        };
        let Some(node) = self.graph.node_mut(id) else {
            self.selected = None;
            return;
        };
        ui.label(format!("{}  ·  #{id}", node.kind.name()));
        let mut changed = false;
        match &mut node.kind {
            NodeKind::Noise { scale, octaves, seed } => {
                changed |= ui.add(egui::Slider::new(scale, 1..=64).text("Frequency")).changed();
                changed |= ui.add(egui::Slider::new(octaves, 1..=6).text("Octaves")).changed();
                changed |= ui.add(egui::DragValue::new(seed).speed(1).prefix("Seed: ")).changed();
            }
            NodeKind::Checker { tiles } => {
                let mut half_tiles = *tiles / 2;
                if ui.add(egui::Slider::new(&mut half_tiles, 1..=32).text("Tile pairs")).changed() {
                    *tiles = half_tiles * 2;
                    changed = true;
                }
            }
            NodeKind::Color { rgb } => {
                changed |= ui.color_edit_button_srgb(rgb).changed();
            }
            NodeKind::Bricks { columns, rows, mortar } => {
                changed |= ui.add(egui::Slider::new(columns, 2..=64).text("Columns")).changed();
                let mut half_rows = *rows / 2;
                if ui.add(egui::Slider::new(&mut half_rows, 1..=32).text("Row pairs")).changed() {
                    *rows = half_rows * 2;
                    changed = true;
                }
                changed |= ui.add(egui::Slider::new(mortar, 0.0..=0.4).text("Mortar")).changed();
            }
            NodeKind::Voronoi { cells, seed } => {
                changed |= ui.add(egui::Slider::new(cells, 2..=32).text("Cells")).changed();
                changed |= ui.add(egui::DragValue::new(seed).speed(1).prefix("Seed: ")).changed();
            }
            NodeKind::Strata { bands, warp, seed } => {
                changed |= ui.add(egui::Slider::new(bands, 1..=64).text("Bands")).changed();
                changed |= ui.add(egui::Slider::new(warp, 0.0..=1.0).text("Warp")).changed();
                changed |= ui.add(egui::DragValue::new(seed).prefix("Seed: ")).changed();
            }
            NodeKind::Library { operation, amount, seed } => {
                ui.label(egui::RichText::new(operation.category()).strong());
                ui.small(format!("{} input socket(s)", operation.input_count()));
                changed |= ui.add(egui::Slider::new(amount, 0.0..=1.0)
                    .text("Amount / scale")).changed();
                changed |= ui.add(egui::DragValue::new(seed).speed(1).prefix("Seed: ")).changed();
                ui.small("Rendered by the Rust CPU engine. No shader or plugin needed.");
            }
            NodeKind::Colorize { dark, light } => {
                ui.label("Dark color");
                changed |= ui.color_edit_button_srgb(dark).changed();
                ui.label("Light color");
                changed |= ui.color_edit_button_srgb(light).changed();
            }
            NodeKind::Invert => { ui.label("Inverts the RGB channels of its input."); }
            NodeKind::Blend { factor } => {
                changed |= ui.add(egui::Slider::new(factor, 0.0..=1.0).text("Mix")).changed();
            }
            NodeKind::Output => {
                ui.label("Connect the final PBR channels here.");
                ui.small("Albedo · Roughness · Metallic · Height");
                ui.small("Normal map is derived from Height during export.");
            }
        }
        self.dirty |= changed;
        if !matches!(node.kind, NodeKind::Output) && ui.button("Delete selected node").clicked() {
            if let Err(err) = self.graph.remove_node(id) {
                self.status = err;
            } else {
                self.selected = None;
                if self.connecting == Some(id) { self.connecting = None; }
                self.wire_gesture = None;
                self.dirty = true;
                self.status = "Node deleted".into();
            }
        }
    }


    /// Geometric port picking avoids click-only widgets capturing the drag and
    /// works even when the pointer is released over another node's body.
    fn port_at(&self, canvas: Rect, pointer: Pos2) -> Option<Port> {
        if !canvas.contains(pointer) { return None; }
        let mut nearest: Option<(f32, Port)> = None;
        for node in self.graph.nodes.iter().rev() {
            let mut consider = |position: Pos2, port: Port| {
                let distance = position.distance(pointer);
                if distance <= 17.0 && nearest.map_or(true, |(d, _)| distance < d) {
                    nearest = Some((distance, port));
                }
            };
            if !matches!(node.kind, NodeKind::Output) {
                consider(socket_output(canvas.min, self.pan, self.zoom,
                    node.position, node.kind.input_count()), Port::Output(node.id));
            }
            for input in 0..node.kind.input_count() {
                consider(socket_input(canvas.min, self.pan, self.zoom,
                    node.position, input), Port::Input(node.id, input));
            }
        }
        nearest.map(|(_, port)| port)
    }

    /// The body of a wire is also grab-able, not just its small end socket.
    fn cable_at(&self, canvas: Rect, pointer: Pos2) -> Option<(u64, usize, u64)> {
        if !canvas.contains(pointer) { return None; }
        // Nodes should win over cables that happen to cross behind them.
        for node in &self.graph.nodes {
            let origin = screen_point(canvas.min, self.pan, self.zoom,
                node.position[0], node.position[1]);
            if Rect::from_min_size(origin,
                Vec2::new(NODE_W * self.zoom, node_height(node.kind) * self.zoom)).contains(pointer)
            {
                return None;
            }
        }
        let mut best: Option<(f32, (u64, usize, u64))> = None;
        for link in &self.graph.links {
            let (Some(src), Some(dst)) = (self.graph.node(link.from), self.graph.node(link.to)) else { continue; };
            let a = socket_output(canvas.min, self.pan, self.zoom, src.position, src.kind.input_count());
            let b = socket_input(canvas.min, self.pan, self.zoom, dst.position, link.input);
            let mut previous = a;
            for i in 1..=24 {
                let point = wire_bezier(a, b, i as f32 / 24.0);
                let d = distance_to_segment(pointer, previous, point);
                if d <= 11.0 && best.map_or(true, |(distance, _)| d < distance) {
                    best = Some((d, (link.to, link.input, link.from)));
                }
                previous = point;
            }
        }
        best.map(|(_, link)| link)
    }

    fn connect_dropped_wire(&mut self, gesture: WireGesture, target: Port) {
        let result = match (gesture.start, target) {
            (Port::Output(from), Port::Input(to, input)) =>
                self.graph.connect(from, to, input),
            (Port::Input(to, input), Port::Output(from)) => {
                if gesture.original.is_some() {
                    self.graph.rewire(to, input, from, to, input)
                } else {
                    self.graph.connect(from, to, input)
                }
            }
            (Port::Input(old_to, old_input), Port::Input(new_to, new_input)) => {
                if let Some((_, _, from)) = gesture.original {
                    self.graph.rewire(old_to, old_input, from, new_to, new_input)
                } else {
                    Err("To create a connection, drag this input to an output socket".into())
                }
            }
            _ => Err("Connect an output to an input, not two matching sockets".into()),
        };
        match result {
            Ok(()) => {
                self.connecting = None;
                self.dirty = true;
                self.status = "Connection updated".into();
            }
            Err(err) => self.status = format!("Connection unchanged: {err}"),
        }
    }

    fn handle_wire_input(&mut self, ui: &egui::Ui, canvas: Rect) {
        let (pointer, pressed, released, held, right_pressed, escape) = ui.input(|i| (
            i.pointer.latest_pos(),
            i.pointer.button_pressed(egui::PointerButton::Primary),
            i.pointer.button_released(egui::PointerButton::Primary),
            i.pointer.button_down(egui::PointerButton::Primary),
            i.pointer.button_pressed(egui::PointerButton::Secondary),
            i.key_pressed(egui::Key::Escape),
        ));

        if escape {
            self.wire_gesture = None;
            self.connecting = None;
            self.status = "Cable operation cancelled".into();
            return;
        }

        if let Some(pointer) = pointer {
            if right_pressed {
                let disconnected = match self.port_at(canvas, pointer) {
                    Some(Port::Input(to, slot)) => Some((to, slot)),
                    _ => self.cable_at(canvas, pointer).map(|(to, slot, _)| (to, slot)),
                };
                if let Some((to, slot)) = disconnected {
                    self.graph.disconnect(to, slot);
                    self.wire_gesture = None;
                    self.connecting = None;
                    self.dirty = true;
                    self.status = "Connection removed".into();
                    return;
                }
            }

            if pressed && canvas.contains(pointer) {
                let start = self.port_at(canvas, pointer).or_else(|| {
                    self.cable_at(canvas, pointer).map(|(to, slot, _)| Port::Input(to, slot))
                });
                if let Some(start) = start {
                    let original = match start {
                        Port::Input(to, slot) =>
                            self.graph.links.iter().find(|link| link.to == to && link.input == slot)
                                .map(|link| (to, slot, link.from)),
                        Port::Output(_) => None,
                    };
                    self.wire_gesture = Some(WireGesture {
                        start, original, pressed_at: pointer, dragging: false,
                    });
                } else {
                    self.connecting = None;
                }
            }

            if let Some(mut gesture) = self.wire_gesture {
                if held && pointer.distance(gesture.pressed_at) > 6.0 {
                    gesture.dragging = true;
                    self.wire_gesture = Some(gesture);
                }
                if released {
                    if gesture.dragging {
                        if let Some(target) = self.port_at(canvas, pointer) {
                            self.connect_dropped_wire(gesture, target);
                        } else {
                            self.status = "Cable drop cancelled; original connection preserved".into();
                        }
                    } else {
                        match gesture.start {
                            Port::Output(from) => {
                                self.connecting = Some(from);
                                self.selected = Some(from);
                                self.status = "Now click an input socket, or drag to it".into();
                            }
                            Port::Input(to, slot) => {
                                if let Some(from) = self.connecting {
                                    self.connect_dropped_wire(WireGesture {
                                        start: Port::Output(from), ..gesture
                                    }, Port::Input(to, slot));
                                } else {
                                    self.selected = Some(to);
                                    self.status = "Drag this socket or its cable to another port".into();
                                }
                            }
                        }
                    }
                    self.wire_gesture = None;
                }
            }
        }
    }

    fn canvas(&mut self, ui: &mut egui::Ui) {
        let (rect, _) = ui.allocate_exact_size(ui.available_size(), Sense::hover());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0, Color32::from_rgb(20, 22, 29));

        if let Some(pointer) = ui.ctx().pointer_hover_pos() {
            if rect.contains(pointer) {
                let (scroll, middle_down, delta) = ui.input(|i|
                    (i.smooth_scroll_delta.y, i.pointer.middle_down(), i.pointer.delta()));
                if middle_down { self.pan += delta; }
                if scroll.abs() > 0.1 {
                    let previous = self.zoom;
                    self.zoom = (self.zoom * (scroll * 0.0015).exp()).clamp(0.55, 1.65);
                    let anchor = pointer - rect.min;
                    self.pan = anchor - (anchor - self.pan) * (self.zoom / previous);
                }
            }
        }

        let grid_step = 36.0 * self.zoom;
        let grid_color = Color32::from_rgb(34, 37, 46);
        let mut x = rect.min.x + self.pan.x.rem_euclid(grid_step);
        while x < rect.max.x {
            painter.line_segment([Pos2::new(x, rect.min.y), Pos2::new(x, rect.max.y)],
                Stroke::new(1.0, grid_color));
            x += grid_step;
        }
        let mut y = rect.min.y + self.pan.y.rem_euclid(grid_step);
        while y < rect.max.y {
            painter.line_segment([Pos2::new(rect.min.x, y), Pos2::new(rect.max.x, y)],
                Stroke::new(1.0, grid_color));
            y += grid_step;
        }

        let moving_link = self.wire_gesture.filter(|gesture| gesture.dragging)
            .and_then(|gesture| gesture.original);
        for link in &self.graph.links {
            if moving_link == Some((link.to, link.input, link.from)) { continue; }
            let (Some(source), Some(target)) = (self.graph.node(link.from), self.graph.node(link.to)) else { continue; };
            let from = socket_output(rect.min, self.pan, self.zoom, source.position, source.kind.input_count());
            let to = socket_input(rect.min, self.pan, self.zoom, target.position, link.input);
            let color = match (target.kind, link.input) {
                (NodeKind::Output, 0) => Color32::from_rgb(239, 152, 95),
                (NodeKind::Output, 1) => Color32::from_rgb(175, 179, 190),
                (NodeKind::Output, 2) => Color32::from_rgb(177, 132, 207),
                (NodeKind::Output, 3) => Color32::from_rgb(100, 207, 222),
                _ => Color32::from_rgb(103, 170, 233),
            };
            draw_wire(&painter, from, to, color);
        }

        let nodes: Vec<_> = self.graph.nodes.iter()
            .map(|n| (n.id, n.position, n.kind)).collect();
        for (id, position, kind) in nodes {
            let z = self.zoom;
            let origin = screen_point(rect.min, self.pan, z, position[0], position[1]);
            let height = node_height(kind) * z;
            let node_rect = Rect::from_min_size(origin, Vec2::new(NODE_W * z, height));
            let active = self.selected == Some(id);
            painter.rect_filled(node_rect, 6, Color32::from_rgb(42, 45, 57));
            painter.rect_stroke(node_rect, 6,
                Stroke::new(if active { 2.0 } else { 1.0 },
                    if active { Color32::from_rgb(114, 198, 255) } else { Color32::from_rgb(74, 82, 100) }),
                StrokeKind::Inside);

            let body = Rect::from_min_max(origin + Vec2::new(0.0, 32.0 * z), node_rect.max);
            if ui.interact(body, ui.make_persistent_id(("node_body", id)), Sense::click()).clicked() {
                self.selected = Some(id);
            }
            let header = Rect::from_min_size(origin, Vec2::new(NODE_W * z, 31.0 * z));
            painter.rect_filled(header.shrink(1.0), 5, node_accent(kind));
            painter.text(origin + Vec2::new(9.0 * z, 15.0 * z), Align2::LEFT_CENTER,
                format!("{}  ·  {}", kind.name(), id),
                FontId::proportional(13.0 * z), Color32::WHITE);
            let response = ui.interact(header, ui.make_persistent_id(("node_header", id)), Sense::click_and_drag());
            if response.clicked() { self.selected = Some(id); }
            if response.dragged() {
                self.selected = Some(id);
                // pointer.delta() is per frame; drag_delta() is cumulative and caused runaway motion.
                let delta = ui.input(|i| i.pointer.delta()) / z;
                if let Some(node) = self.graph.node_mut(id) {
                    node.position[0] += delta.x;
                    node.position[1] += delta.y;
                }
            }

            if let Some(texture) = self.thumbs.get(&id) {
                let image = Rect::from_min_size(origin + Vec2::new(12.0 * z, 57.0 * z),
                    Vec2::splat(55.0 * z));
                painter.image(texture.id(), image,
                    Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
            }
            if matches!(kind, NodeKind::Output) {
                painter.text(origin + Vec2::new(12.0 * z, 47.0 * z), Align2::LEFT_CENTER,
                    "PBR CHANNELS", FontId::proportional(10.0 * z),
                    Color32::from_rgb(190, 200, 215));
            }

            if !matches!(kind, NodeKind::Output) {
                let output = socket_output(rect.min, self.pan, z, position, kind.input_count());
                painter.circle_filled(output, 6.0 * z,
                    if self.connecting == Some(id) { Color32::LIGHT_GREEN } else { Color32::from_rgb(103, 200, 232) });
            }
            for input in 0..kind.input_count() {
                let socket = socket_input(rect.min, self.pan, z, position, input);
                let color = if matches!(kind, NodeKind::Output) {
                    match input {
                        0 => Color32::from_rgb(239, 152, 95),
                        1 => Color32::from_rgb(175, 179, 190),
                        2 => Color32::from_rgb(177, 132, 207),
                        _ => Color32::from_rgb(100, 207, 222),
                    }
                } else { Color32::from_rgb(234, 185, 96) };
                painter.circle_filled(socket, 6.0 * z, color);
                painter.text(socket + Vec2::new(12.0 * z, 0.0), Align2::LEFT_CENTER,
                    kind.input_name(input), FontId::proportional(11.0 * z), Color32::WHITE);
            }
        }
        self.handle_wire_input(ui, rect);
        if let Some(pointer) = ui.ctx().pointer_hover_pos() {
            let active = self.wire_gesture.filter(|gesture| gesture.dragging);
            if let Some(gesture) = active {
                match gesture.start {
                    Port::Output(from) => {
                        if let Some(node) = self.graph.node(from) {
                            let start = socket_output(rect.min, self.pan, self.zoom,
                                node.position, node.kind.input_count());
                            draw_wire(&painter, start, pointer, Color32::LIGHT_GREEN);
                        }
                    }
                    Port::Input(to, slot) => {
                        if let Some((_, _, from)) = gesture.original {
                            if let Some(node) = self.graph.node(from) {
                                let start = socket_output(rect.min, self.pan, self.zoom,
                                    node.position, node.kind.input_count());
                                draw_wire(&painter, start, pointer, Color32::LIGHT_GREEN);
                            }
                        } else if let Some(node) = self.graph.node(to) {
                            let end = socket_input(rect.min, self.pan, self.zoom, node.position, slot);
                            draw_wire(&painter, pointer, end, Color32::LIGHT_GREEN);
                        }
                    }
                }
            } else if let Some(from) = self.connecting {
                if let Some(node) = self.graph.node(from) {
                    let start = socket_output(rect.min, self.pan, self.zoom,
                        node.position, node.kind.input_count());
                    draw_wire(&painter, start, pointer, Color32::LIGHT_GREEN);
                }
            }

            if let Some(port) = self.port_at(rect, pointer) {
                let position = match port {
                    Port::Output(id) => self.graph.node(id).map(|node|
                        socket_output(rect.min, self.pan, self.zoom, node.position, node.kind.input_count())),
                    Port::Input(id, slot) => self.graph.node(id).map(|node|
                        socket_input(rect.min, self.pan, self.zoom, node.position, slot)),
                };
                if let Some(center) = position {
                    painter.circle_stroke(center, 12.0,
                        Stroke::new(2.0, Color32::LIGHT_GREEN));
                }
            }
        }
    }

}

impl eframe::App for Studio {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if self.library_tab == 3 {
            if self.asset_dirty {
                match self.asset.mesh() {
                    Ok(mesh) => self.asset_mesh = Some(mesh),
                    Err(e) => { self.asset_mesh = None; self.status = format!("Asset error: {e}"); }
                }
                self.asset_dirty = false;
            }
        } else if self.library_tab == 2 {
            if self.foliage_dirty {
                match self.foliage.generate() {
                    Ok(mesh) => self.foliage_mesh = Some(mesh),
                    Err(err) => { self.foliage_mesh = None; self.status = format!("Foliage error: {err}"); }
                }
                self.foliage_dirty = false;
            }
        } else {
            self.refresh_preview(&ctx);
        }
        egui::Panel::top("toolbar").show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.heading("TEXTURE STUDIO");
                ui.separator();
                if self.library_tab == 3 {
                    if ui.button("New Asset").clicked() { self.new_asset(); }
                    if ui.button("Open Asset").clicked() { self.open_asset(); }
                    if ui.button("Save Asset").clicked() { self.save_asset(false); }
                    if ui.button("Save As").clicked() { self.save_asset(true); }
                    if ui.button("Export Asset").clicked() { self.export_asset(); }
                    ui.separator();
                    if ui.add_enabled(!self.asset_undo.is_empty(),
                        egui::Button::new("Undo")).clicked() { self.undo_asset(); }
                    if ui.add_enabled(!self.asset_redo.is_empty(),
                        egui::Button::new("Redo")).clicked() { self.redo_asset(); }
                } else if self.library_tab == 2 {
                    if ui.button("New Tree").clicked() {
                        self.foliage = FoliageProject::default();
                        self.foliage_dirty = true;
                        self.status = "New procedural tree created".into();
                    }
                    if ui.button("Open Tree").clicked() { self.open_foliage(); }
                    if ui.button("Save Tree").clicked() { self.save_foliage(); }
                    if ui.button("Export Voxel Tree").clicked() { self.export_foliage(); }
                    if ui.button("Edit Tree as Asset").clicked() { self.import_tree_as_asset(); }
                    ui.separator();
                } else {
                if ui.button("New").clicked() { self.new_project(); }
                if ui.button("Open").clicked() { self.open_project(); }
                if ui.button("Save").clicked() { self.save_project(false); }
                if ui.button("Save As").clicked() { self.save_project(true); }
                ui.separator();
                egui::ComboBox::from_id_salt("export_size")
                    .selected_text(format!("{}px", self.export_size))
                    .show_ui(ui, |ui| {
                        for size in [256, 512, 1024, 2048] {
                            ui.selectable_value(&mut self.export_size, size, format!("{size} × {size}"));
                        }
                    });
                if ui.button("Export image").clicked() { self.export_png(); }
                if ui.button("Export PBR maps").clicked() { self.export_material(); }
                if ui.add_enabled(!self.graph.voxel.geometryless,
                    egui::Button::new("Export voxel OBJ + atlases")).clicked() {
                    self.export_voxel();
                }
                ui.separator();
                if ui.button("Example").clicked() { self.demo_material(); }
                }
            });
        });
        egui::Panel::bottom("status").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.small(&self.status);
                if let Some(path) = &self.project_path {
                    ui.separator();
                    ui.small(path.display().to_string());
                }
            });
        });
        egui::Panel::left("library").resizable(true).default_size(195.0).min_size(160.0).show(ui, |ui| {
            ui.add_space(8.0);
            self.palette(ui);
        });
        egui::Panel::right("previews").resizable(true).default_size(300.0).min_size(240.0).show(ui, |ui| {
            ui.add_space(6.0);
            if self.library_tab == 2 {
                self.foliage_inspector(ui);
                return;
            }
            if self.library_tab == 3 {
                self.asset_inspector(ui);
                return;
            }
            ui.heading("MATERIAL GEOMETRY");
            ui.horizontal(|ui| {
                ui.label("Mode");
                egui::ComboBox::from_id_salt("material_geometry")
                    .selected_text(if self.graph.voxel.geometryless {
                        "Geometryless (2D)"
                    } else { self.graph.voxel.shape.name() })
                    .show_ui(ui, |ui| {
                        if ui.selectable_label(self.graph.voxel.geometryless,
                            "Geometryless (2D)").clicked() {
                            self.graph.voxel.geometryless = true;
                            self.view = 1;
                            self.dirty = true;
                        }
                        if ui.selectable_label(!self.graph.voxel.geometryless &&
                            self.graph.voxel.shape == VoxelShape::Cube, "Cube (6 faces)").clicked() {
                            self.graph.voxel.geometryless = false;
                            self.graph.voxel.shape = VoxelShape::Cube;
                            self.view = 0;
                            self.dirty = true;
                        }
                        if ui.selectable_label(!self.graph.voxel.geometryless &&
                            self.graph.voxel.shape == VoxelShape::HexPrism,
                            "Hexagonal prism (8 faces)").clicked() {
                            self.graph.voxel.geometryless = false;
                            self.graph.voxel.shape = VoxelShape::HexPrism;
                            self.view = 0;
                            self.dirty = true;
                        }
                    });
            });
            if !self.graph.voxel.geometryless {
                ui.horizontal(|ui| {
                    ui.label("Side UV");
                    egui::ComboBox::from_id_salt("voxel_uv_mapping")
                        .selected_text(match self.graph.voxel.side_mapping {
                            SideMapping::Wrap => "Wrap around perimeter",
                            SideMapping::Repeat => "Repeat per face",
                        })
                        .show_ui(ui, |ui| {
                            self.dirty |= ui.selectable_value(&mut self.graph.voxel.side_mapping,
                                SideMapping::Wrap, "Wrap around perimeter").changed();
                            self.dirty |= ui.selectable_value(&mut self.graph.voxel.side_mapping,
                                SideMapping::Repeat, "Repeat per face").changed();
                        });
                });
                ui.small("Shape and UV mode are saved in the project.");
            } else {
                ui.small("2D-only PBR textures. No mesh, UV atlas, or voxel preview.");
                ui.small("Export image or Export PBR maps for square PNG files.");
            }
            ui.separator();
            ui.heading("PREVIEW");
            ui.horizontal(|ui| {
                if !self.graph.voxel.geometryless {
                    self.dirty |= ui.selectable_value(&mut self.view, 0, "Voxel 3D").changed();
                }
                self.dirty |= ui.selectable_value(&mut self.view, 1, "2D").changed();
                self.dirty |= ui.selectable_value(&mut self.view, 2, "Node").changed();
                if !self.graph.voxel.geometryless {
                    self.dirty |= ui.selectable_value(&mut self.view, 3, "Atlas").changed();
                }
            });
            if self.graph.voxel.geometryless && (self.view == 0 || self.view == 3) {
                self.view = 1;
                self.dirty = true;
            }
            if self.view == 0 {
                self.dirty |= ui.add(egui::Slider::new(&mut self.voxel_yaw,
                    0.0..=std::f32::consts::TAU).text("Orbit")).changed();
            }
            if self.view == 1 || self.view == 3 {
                egui::ComboBox::from_id_salt("output_channel")
                    .selected_text(["Albedo", "Roughness", "Metallic", "Height", "Normal"][self.preview_channel])
                    .show_ui(ui, |ui| {
                        for (channel, name) in ["Albedo", "Roughness", "Metallic", "Height", "Normal"].iter().enumerate() {
                            if ui.selectable_value(&mut self.preview_channel, channel, *name).changed() {
                                self.dirty = true;
                            }
                        }
                    });
            }
            let preview = match self.view {
                0 => self.voxel_preview.as_ref(),
                1 => self.preview.as_ref(),
                2 => self.selected_preview.as_ref(),
                _ => self.atlas_preview.as_ref(),
            };
            if let Some(texture) = preview {
                let width = ui.available_width().min(272.0);
                let size = texture.size();
                let height = width * size[1] as f32 / size[0].max(1) as f32;
                ui.add(egui::Image::new(texture).fit_to_exact_size(Vec2::new(width, height)));
            } else {
                ui.label("No preview available.");
            }
            ui.small(match self.view {
                0 => "Cube / hex-prism faces with UV-mapped texture · approximate lighting",
                1 => "Tileable 2D material channel",
                2 => "Selected node image",
                _ => "Face tiles: sides first, then top and bottom caps",
            });
            ui.separator();
            self.inspector(ui);
        });
        egui::CentralPanel::default().show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("NODE GRAPH").strong().color(Color32::LIGHT_GRAY));
                ui.separator();
                ui.small("Wheel: zoom · Middle drag: pan · Click sockets: connect");
                if ui.button("Reset view").clicked() {
                    self.pan = Vec2::ZERO;
                    self.zoom = 1.0;
                }
                ui.small(format!("{:.0}%", self.zoom * 100.0));
            });
            if self.library_tab == 2 {
                self.foliage_canvas(ui);
            } else if self.library_tab == 3 {
                self.asset_canvas(ui);
            } else {
                self.canvas(ui);
            }
        });
        if self.dirty || self.foliage_dirty || self.asset_dirty { ctx.request_repaint(); }
    }
}

fn node_height(kind: NodeKind) -> f32 {
    node_height_for_inputs(kind.input_count())
}

fn node_height_for_inputs(inputs: usize) -> f32 {
    135.0 + inputs.saturating_sub(1) as f32 * 25.0
}

fn screen_point(canvas: Pos2, pan: Vec2, zoom: f32, x: f32, y: f32) -> Pos2 {
    canvas + pan + Vec2::new(x * zoom, y * zoom)
}

fn socket_output(canvas: Pos2, pan: Vec2, zoom: f32, position: [f32; 2], inputs: usize) -> Pos2 {
    screen_point(canvas, pan, zoom, position[0] + NODE_W, position[1] + node_height_for_inputs(inputs) - 20.0)
}

fn socket_input(canvas: Pos2, pan: Vec2, zoom: f32, position: [f32; 2], input: usize) -> Pos2 {
    screen_point(canvas, pan, zoom, position[0], position[1] + 84.0 + input as f32 * 25.0)
}

fn node_accent(kind: NodeKind) -> Color32 {
    match kind {
        NodeKind::Noise { .. } | NodeKind::Checker { .. } | NodeKind::Color { .. }
            | NodeKind::Bricks { .. } | NodeKind::Voronoi { .. } | NodeKind::Strata { .. } => Color32::from_rgb(43, 95, 131),
        NodeKind::Library { operation, .. } => match operation.category() {
            "Shapes & Gradients" | "Noise" | "Patterns" => Color32::from_rgb(43, 95, 131),
            "Transforms" => Color32::from_rgb(107, 80, 148),
            "Math & Mix" => Color32::from_rgb(143, 96, 58),
            _ => Color32::from_rgb(115, 64, 127),
        },
        NodeKind::Invert | NodeKind::Blend { .. } | NodeKind::Colorize { .. } => Color32::from_rgb(115, 64, 127),
        NodeKind::Output => Color32::from_rgb(43, 105, 77),
    }
}

/// Cubic cable path shared by painter and wire hit-testing.
fn wire_bezier(a: Pos2, b: Pos2, t: f32) -> Pos2 {
    let bend = ((b.x - a.x).abs() * 0.45).max(30.0);
    let c1 = a + Vec2::new(bend, 0.0);
    let c2 = b - Vec2::new(bend, 0.0);
    let inv = 1.0 - t;
    let point = a.to_vec2() * (inv * inv * inv)
        + c1.to_vec2() * (3.0 * inv * inv * t)
        + c2.to_vec2() * (3.0 * inv * t * t)
        + b.to_vec2() * (t * t * t);
    Pos2::new(point.x, point.y)
}

fn distance_to_segment(point: Pos2, a: Pos2, b: Pos2) -> f32 {
    let ab = b - a;
    let length_squared = ab.length_sq();
    if length_squared < 0.001 { return point.distance(a); }
    let t = ((point - a).dot(ab) / length_squared).clamp(0.0, 1.0);
    point.distance(a + ab * t)
}

fn draw_wire(painter: &egui::Painter, a: Pos2, b: Pos2, color: Color32) {
    let mut previous = a;
    for i in 1..=24 {
        let point = wire_bezier(a, b, i as f32 / 24.0);
        painter.line_segment([previous, point], Stroke::new(2.5, color));
        previous = point;
    }
}


fn put_texture(ctx: &egui::Context, slot: &mut Option<TextureHandle>, name: &str,
    rgba: Vec<u8>, size: u32)
{
    put_texture_rect(ctx, slot, name, rgba, [size as usize, size as usize]);
}

fn put_texture_rect(ctx: &egui::Context, slot: &mut Option<TextureHandle>, name: &str,
    rgba: Vec<u8>, dimensions: [usize;2])
{
    let image = egui::ColorImage::from_rgba_unmultiplied(dimensions, &rgba);
    if let Some(existing) = slot {
        existing.set(image, egui::TextureOptions::LINEAR);
    } else {
        *slot = Some(ctx.load_texture(name, image, egui::TextureOptions::LINEAR));
    }
}

fn save_png(path: &Path, size: u32, pixels: &[u8]) -> Result<(), String> {
    save_png_rect(path, size, size, pixels)
}

fn save_png_rect(path: &Path, width: u32, height: u32, pixels: &[u8]) -> Result<(), String> {
    if pixels.len() != (width as usize * height as usize * 4) {
        return Err("PNG buffer dimensions do not match".into());
    }
    let file = std::fs::File::create(path).map_err(|e| e.to_string())?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
    writer.write_image_data(pixels).map_err(|e| e.to_string())
}

#[cfg(test)]
mod wire_interaction_tests {
    use super::*;

    fn canvas() -> Rect {
        Rect::from_min_size(Pos2::ZERO, Vec2::new(1300.0, 900.0))
    }

    #[test]
    fn sockets_can_be_hit_after_zoom_and_pan() {
        let mut app = Studio::default();
        app.pan = Vec2::new(83.0, -19.0);
        app.zoom = 0.62;
        let node = app.graph.node(2).unwrap();
        let input = socket_input(canvas().min, app.pan, app.zoom, node.position, 0);
        assert_eq!(app.port_at(canvas(), input), Some(Port::Input(2, 0)));
        let source = app.graph.node(1).unwrap();
        let output = socket_output(canvas().min, app.pan, app.zoom, source.position,
            source.kind.input_count());
        assert_eq!(app.port_at(canvas(), output), Some(Port::Output(1)));
    }

    #[test]
    fn cables_can_be_selected_between_nodes() {
        let app = Studio::default();
        let source = app.graph.node(1).unwrap();
        let target = app.graph.node(2).unwrap();
        let start = socket_output(canvas().min, app.pan, app.zoom,
            source.position, source.kind.input_count());
        let end = socket_input(canvas().min, app.pan, app.zoom, target.position, 0);
        let midpoint = wire_bezier(start, end, 0.5);
        assert_eq!(app.cable_at(canvas(), midpoint), Some((2, 0, 1)));
    }
}

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Texture Studio — Procedural Textures")
            .with_inner_size([1330.0, 780.0])
            .with_min_inner_size([940.0, 550.0]),
        ..Default::default()
    };
    eframe::run_native("texture-studio", options, Box::new(|_| Ok(Box::new(Studio::default()))))
}
