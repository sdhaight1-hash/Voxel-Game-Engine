use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, TextureHandle, Vec2};
use std::path::PathBuf;
use texture_studio::{Graph, NodeKind};

const NODE_W: f32 = 170.0;
const PREVIEW_SIZE: u32 = 256;

struct Studio {
    graph: Graph,
    selected: Option<u64>,
    connecting: Option<u64>,
    preview: Option<TextureHandle>,
    dirty: bool,
    export_size: u32,
    project_path: Option<PathBuf>,
    status: String,
}

impl Default for Studio {
    fn default() -> Self {
        Self {
            graph: Graph::default(),
            selected: Some(1),
            connecting: None,
            preview: None,
            dirty: true,
            export_size: 1024,
            project_path: None,
            status: "Ready — click an output socket, then an input socket to connect nodes".to_owned(),
        }
    }
}

impl Studio {
    fn refresh_preview(&mut self, ctx: &egui::Context) {
        if !self.dirty { return; }
        self.dirty = false;
        match self.graph.render_rgba(PREVIEW_SIZE, PREVIEW_SIZE) {
            Ok(pixels) => {
                let image = egui::ColorImage::from_rgba_unmultiplied(
                    [PREVIEW_SIZE as usize, PREVIEW_SIZE as usize], &pixels,
                );
                if let Some(texture) = &mut self.preview {
                    texture.set(image, egui::TextureOptions::LINEAR);
                } else {
                    self.preview = Some(ctx.load_texture("preview", image, egui::TextureOptions::LINEAR));
                }
            }
            Err(err) => self.status = format!("Preview error: {err}"),
        }
    }

    fn new_project(&mut self) {
        self.graph = Graph::default();
        self.selected = Some(1);
        self.connecting = None;
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
        let result = self.graph.render_rgba(self.export_size, self.export_size)
            .and_then(|pixels| {
                let file = std::fs::File::create(&path).map_err(|e| e.to_string())?;
                let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), self.export_size, self.export_size);
                encoder.set_color(png::ColorType::Rgba);
                encoder.set_depth(png::BitDepth::Eight);
                let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
                writer.write_image_data(&pixels).map_err(|e| e.to_string())
            });
        self.status = match result {
            Ok(()) => format!("PNG exported: {}", path.display()),
            Err(err) => format!("Export failed: {err}"),
        };
    }

    fn spawn(&mut self, kind: NodeKind) {
        let offset = self.graph.nodes.len() as f32 * 24.0;
        let id = self.graph.add_node(kind, [80.0 + offset, 235.0 + offset]);
        self.selected = Some(id);
        self.dirty = true;
        self.status = format!("Added {} node", kind.name());
    }

    fn palette(&mut self, ui: &mut egui::Ui) {
        ui.heading("Add nodes");
        ui.label("Generators");
        if ui.button("+ Noise").clicked() { self.spawn(NodeKind::Noise { scale: 6, octaves: 4, seed: 42 }); }
        if ui.button("+ Checker").clicked() { self.spawn(NodeKind::Checker { tiles: 8 }); }
        if ui.button("+ Color").clicked() { self.spawn(NodeKind::Color { rgb: [91, 159, 226] }); }
        ui.separator();
        ui.label("Filters / mixers");
        if ui.button("+ Invert").clicked() { self.spawn(NodeKind::Invert); }
        if ui.button("+ Blend").clicked() { self.spawn(NodeKind::Blend { factor: 0.5 }); }
        ui.separator();
        ui.label("How to use");
        ui.small("1. Add a node");
        ui.small("2. Click its right socket");
        ui.small("3. Click a left input socket");
        ui.small("4. Select a node to edit it");
        ui.small("5. Drag the node's header to move it");
        ui.add_space(12.0);
        if let Some(id) = self.connecting {
            ui.colored_label(Color32::LIGHT_GREEN, format!("Wiring from node #{id}"));
            if ui.button("Cancel connection").clicked() { self.connecting = None; }
        }
        ui.add_space(12.0);
        ui.label("Tip: right-click an input socket to disconnect its cable.");
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
            NodeKind::Invert => { ui.label("Inverts the RGB channels of its input."); }
            NodeKind::Blend { factor } => {
                changed |= ui.add(egui::Slider::new(factor, 0.0..=1.0).text("Mix")).changed();
            }
            NodeKind::Output => { ui.label("Connect the final image here."); }
        }
        self.dirty |= changed;
        if !matches!(node.kind, NodeKind::Output) && ui.button("Delete selected node").clicked() {
            if let Err(err) = self.graph.remove_node(id) {
                self.status = err;
            } else {
                self.selected = None;
                if self.connecting == Some(id) { self.connecting = None; }
                self.dirty = true;
                self.status = "Node deleted".into();
            }
        }
    }

    fn canvas(&mut self, ui: &mut egui::Ui) {
        let (rect, _) = ui.allocate_exact_size(ui.available_size(), Sense::hover());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0, Color32::from_rgb(24, 27, 34));
        let grid_color = Color32::from_rgb(36, 40, 49);
        let mut x = rect.min.x;
        while x < rect.max.x {
            painter.line_segment([Pos2::new(x, rect.min.y), Pos2::new(x, rect.max.y)], Stroke::new(1.0, grid_color));
            x += 32.0;
        }
        let mut y = rect.min.y;
        while y < rect.max.y {
            painter.line_segment([Pos2::new(rect.min.x, y), Pos2::new(rect.max.x, y)], Stroke::new(1.0, grid_color));
            y += 32.0;
        }

        for link in &self.graph.links {
            let (Some(source), Some(target)) = (self.graph.node(link.from), self.graph.node(link.to)) else { continue; };
            let start = socket_output(rect.min, source.position, source.kind.input_count());
            let end = socket_input(rect.min, target.position, link.input);
            draw_wire(&painter, start, end, Color32::from_rgb(103, 200, 232));
        }

        // Snapshot IDs and geometry to avoid borrowing the graph across drag updates.
        let nodes: Vec<_> = self.graph.nodes.iter().map(|n| (n.id, n.position, n.kind)).collect();
        for (id, position, kind) in nodes {
            let origin = rect.min + Vec2::new(position[0], position[1]);
            let height = node_height(kind);
            let node_rect = Rect::from_min_size(origin, Vec2::new(NODE_W, height));
            let active = self.selected == Some(id);
            painter.rect_filled(node_rect, 8, Color32::from_rgb(43, 48, 60));
            painter.rect_stroke(node_rect, 8,
                Stroke::new(if active { 2.0 } else { 1.0 },
                    if active { Color32::from_rgb(114, 198, 255) } else { Color32::from_rgb(76, 86, 100) }),
                StrokeKind::Inside);
            let title = Rect::from_min_size(origin, Vec2::new(NODE_W, 30.0));
            painter.rect_filled(title.shrink(1.0), 6, node_accent(kind));
            painter.text(origin + Vec2::new(12.0, 15.0), Align2::LEFT_CENTER,
                format!("{}   #{id}", kind.name()), FontId::proportional(14.0), Color32::WHITE);
            let response = ui.interact(title, egui::Id::new(("node_header", id)), Sense::click_and_drag());
            if response.clicked() { self.selected = Some(id); }
            if response.dragged() {
                self.selected = Some(id);
                let delta = response.drag_delta();
                if let Some(node) = self.graph.node_mut(id) {
                    node.position[0] = (node.position[0] + delta.x).max(0.0);
                    node.position[1] = (node.position[1] + delta.y).max(0.0);
                }
            }
            painter.text(origin + Vec2::new(12.0, 48.0), Align2::LEFT_CENTER,
                if kind.input_count() == 0 { "Generator" } else { "Texture inputs" },
                FontId::proportional(11.0), Color32::LIGHT_GRAY);
            if !matches!(kind, NodeKind::Output) {
                let output = socket_output(rect.min, position, kind.input_count());
                painter.circle_filled(output, 6.0, if self.connecting == Some(id) { Color32::LIGHT_GREEN } else { Color32::from_rgb(103, 200, 232) });
                let hit = Rect::from_center_size(output, Vec2::splat(22.0));
                if ui.interact(hit, egui::Id::new(("output", id)), Sense::click()).clicked() {
                    self.connecting = Some(id);
                    self.selected = Some(id);
                    self.status = format!("Choose a left input socket to connect node #{id}");
                }
            }
            for input in 0..kind.input_count() {
                let socket = socket_input(rect.min, position, input);
                painter.circle_filled(socket, 6.0, Color32::from_rgb(234, 185, 96));
                painter.text(socket + Vec2::new(13.0, 0.0), Align2::LEFT_CENTER, kind.input_name(input),
                    FontId::proportional(12.0), Color32::WHITE);
                let hit = Rect::from_center_size(socket, Vec2::splat(22.0));
                let response = ui.interact(hit, egui::Id::new(("input", id, input)), Sense::click());
                if response.clicked() {
                    if let Some(source) = self.connecting {
                        match self.graph.connect(source, id, input) {
                            Ok(()) => {
                                self.connecting = None;
                                self.dirty = true;
                                self.status = "Nodes connected".into();
                            }
                            Err(err) => self.status = err,
                        }
                    } else {
                        self.selected = Some(id);
                        self.status = "Select an output socket first".into();
                    }
                }
                if response.secondary_clicked() {
                    self.graph.disconnect(id, input);
                    self.dirty = true;
                    self.status = "Connection removed".into();
                }
            }
        }
        if let Some(id) = self.connecting {
            if let Some(node) = self.graph.node(id) {
                if let Some(mouse) = ui.ctx().pointer_hover_pos() {
                    draw_wire(&painter, socket_output(rect.min, node.position, node.kind.input_count()),
                        mouse, Color32::LIGHT_GREEN);
                }
            }
        }
    }
}

impl eframe::App for Studio {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.refresh_preview(&ctx);
        egui::TopBottomPanel::top("toolbar").show(&ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.heading("Texture Studio");
                ui.separator();
                if ui.button("New").clicked() { self.new_project(); }
                if ui.button("Open project").clicked() { self.open_project(); }
                if ui.button("Save").clicked() { self.save_project(false); }
                if ui.button("Save as...").clicked() { self.save_project(true); }
                ui.separator();
                egui::ComboBox::from_id_salt("export_size")
                    .selected_text(format!("{}px", self.export_size))
                    .show_ui(ui, |ui| {
                        for size in [256, 512, 1024, 2048] {
                            ui.selectable_value(&mut self.export_size, size, format!("{size} x {size}"));
                        }
                    });
                if ui.button("Export PNG").clicked() { self.export_png(); }
            });
        });
        egui::TopBottomPanel::bottom("status").show(&ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(&self.status);
                if let Some(path) = &self.project_path {
                    ui.separator();
                    ui.small(path.display().to_string());
                }
            });
        });
        egui::SidePanel::left("palette").resizable(false).default_width(170.0).show(&ctx, |ui| {
            ui.add_space(8.0);
            self.palette(ui);
        });
        egui::SidePanel::right("inspector").resizable(true).default_width(290.0).min_width(235.0).show(&ctx, |ui| {
            ui.add_space(8.0);
            ui.heading("Live preview");
            ui.label("256 x 256 · tileable");
            if let Some(texture) = &self.preview {
                let side = ui.available_width().min(256.0);
                ui.add(egui::Image::new(texture).fit_to_exact_size(Vec2::splat(side)));
            } else {
                ui.label("No preview available");
            }
            ui.separator();
            self.inspector(ui);
        });
        egui::CentralPanel::default().show(&ctx, |ui| {
            ui.label("NODE CANVAS  ·  Click sockets to wire  ·  Drag headers to move");
            self.canvas(ui);
        });
        if self.dirty { ctx.request_repaint(); }
    }
}

fn node_height(kind: NodeKind) -> f32 {
    78.0 + (kind.input_count().max(1) as f32) * 25.0
}

fn socket_output(canvas: Pos2, position: [f32; 2], inputs: usize) -> Pos2 {
    canvas + Vec2::new(position[0] + NODE_W, position[1] + node_height_for_inputs(inputs) - 20.0)
}

fn node_height_for_inputs(inputs: usize) -> f32 {
    78.0 + (inputs.max(1) as f32) * 25.0
}

fn socket_input(canvas: Pos2, position: [f32; 2], input: usize) -> Pos2 {
    canvas + Vec2::new(position[0], position[1] + 75.0 + input as f32 * 25.0)
}

fn node_accent(kind: NodeKind) -> Color32 {
    match kind {
        NodeKind::Noise { .. } | NodeKind::Checker { .. } | NodeKind::Color { .. } => Color32::from_rgb(52, 91, 137),
        NodeKind::Invert | NodeKind::Blend { .. } => Color32::from_rgb(103, 70, 142),
        NodeKind::Output => Color32::from_rgb(45, 114, 94),
    }
}

fn draw_wire(painter: &egui::Painter, a: Pos2, b: Pos2, color: Color32) {
    let bend = ((b.x - a.x).abs() * 0.45).max(30.0);
    let c1 = a + Vec2::new(bend, 0.0);
    let c2 = b - Vec2::new(bend, 0.0);
    let mut previous = a;
    for i in 1..=20 {
        let t = i as f32 / 20.0;
        let inv = 1.0 - t;
        let point = a.to_vec2() * (inv * inv * inv)
            + c1.to_vec2() * (3.0 * inv * inv * t)
            + c2.to_vec2() * (3.0 * inv * t * t)
            + b.to_vec2() * (t * t * t);
        let point = Pos2::new(point.x, point.y);
        painter.line_segment([previous, point], Stroke::new(2.5, color));
        previous = point;
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
