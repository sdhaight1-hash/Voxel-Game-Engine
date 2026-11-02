use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, TextureHandle, Vec2};
use std::{collections::HashMap, path::{Path, PathBuf}};
use std::f32::consts::PI;
use texture_studio::{Graph, NodeKind};

const NODE_W: f32 = 170.0;
const PREVIEW_SIZE: u32 = 256;

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
    selected: Option<u64>,
    connecting: Option<u64>,
    wire_gesture: Option<WireGesture>,
    preview: Option<TextureHandle>,
    selected_preview: Option<TextureHandle>,
    previewed_id: Option<u64>,
    sphere: Option<TextureHandle>,
    thumbs: HashMap<u64, TextureHandle>,
    preview_channel: usize,
    view: u8,
    pan: Vec2,
    zoom: f32,
    library_search: String,
    dirty: bool,
    export_size: u32,
    project_path: Option<PathBuf>,
    status: String,
}

impl Default for Studio {
    fn default() -> Self {
        Self {
            graph: Graph::demo_bricks(),
            selected: Some(2),
            connecting: None,
            wire_gesture: None,
            preview: None,
            selected_preview: None,
            previewed_id: None,
            sphere: None,
            thumbs: HashMap::new(),
            preview_channel: 0,
            view: 0,
            pan: Vec2::ZERO,
            zoom: 1.0,
            library_search: String::new(),
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
        let rendered = if self.preview_channel == 4 {
            self.graph.render_normal_rgba(PREVIEW_SIZE, PREVIEW_SIZE)
        } else {
            self.graph.render_output_channel_rgba(self.preview_channel, PREVIEW_SIZE, PREVIEW_SIZE)
        };
        match rendered {
            Ok(pixels) => put_texture(ctx, &mut self.preview, "output-preview", pixels, PREVIEW_SIZE),
            Err(err) => self.status = format!("Preview error: {err}"),
        }
        match make_sphere_preview(&self.graph, PREVIEW_SIZE) {
            Ok(pixels) => put_texture(ctx, &mut self.sphere, "sphere-preview", pixels, PREVIEW_SIZE),
            Err(err) => self.status = format!("Sphere preview error: {err}"),
        }
        if let Some(id) = self.selected {
            if let Ok(pixels) = self.graph.render_node_rgba(id, PREVIEW_SIZE, PREVIEW_SIZE) {
                put_texture(ctx, &mut self.selected_preview, "selected-preview", pixels, PREVIEW_SIZE);
            }
        } else {
            self.selected_preview = None;
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

    fn palette(&mut self, ui: &mut egui::Ui) {
        ui.heading("LIBRARY");
        ui.add(egui::TextEdit::singleline(&mut self.library_search).hint_text("Search nodes..."));
        ui.separator();
        let query = self.library_search.trim().to_lowercase();
        for (section, entries) in [
            ("GENERATORS", vec![
                ("Noise", NodeKind::Noise { scale: 6, octaves: 4, seed: 42 }),
                ("Checker", NodeKind::Checker { tiles: 8 }),
                ("Bricks", NodeKind::Bricks { columns: 8, rows: 12, mortar: 0.12 }),
                ("Voronoi", NodeKind::Voronoi { cells: 7, seed: 17 }),
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
        self.refresh_preview(&ctx);
        egui::Panel::top("toolbar").show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.heading("TEXTURE STUDIO");
                ui.separator();
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
                ui.separator();
                if ui.button("Example").clicked() { self.demo_material(); }
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
            ui.heading("PREVIEW");
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.view, 0, "3D");
                ui.selectable_value(&mut self.view, 1, "2D");
                ui.selectable_value(&mut self.view, 2, "Node");
            });
            if self.view == 1 {
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
                0 => self.sphere.as_ref(),
                1 => self.preview.as_ref(),
                _ => self.selected_preview.as_ref(),
            };
            if let Some(texture) = preview {
                let side = ui.available_width().min(272.0);
                ui.add(egui::Image::new(texture).fit_to_exact_size(Vec2::splat(side)));
            } else {
                ui.label("Select a node to preview.");
            }
            ui.small(if self.view == 0 {
                "Lit sphere simulation · not a full physically based renderer"
            } else {
                "Tileable 2D output"
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
            self.canvas(ui);
        });
        if self.dirty { ctx.request_repaint(); }
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
            | NodeKind::Bricks { .. } | NodeKind::Voronoi { .. } => Color32::from_rgb(43, 95, 131),
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
    let image = egui::ColorImage::from_rgba_unmultiplied([size as usize, size as usize], &rgba);
    if let Some(existing) = slot {
        existing.set(image, egui::TextureOptions::LINEAR);
    } else {
        *slot = Some(ctx.load_texture(name, image, egui::TextureOptions::LINEAR));
    }
}

fn save_png(path: &Path, size: u32, pixels: &[u8]) -> Result<(), String> {
    let file = std::fs::File::create(path).map_err(|e| e.to_string())?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), size, size);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
    writer.write_image_data(pixels).map_err(|e| e.to_string())
}

/// Approximate lit sphere: actual generated albedo, roughness and metallic textures are
/// sampled on spherical UVs. This is a preview, not a physically accurate PBR shader.
fn make_sphere_preview(graph: &Graph, size: u32) -> Result<Vec<u8>, String> {
    let albedo = graph.render_output_channel_rgba(0, size, size)?;
    let roughness = graph.render_output_channel_rgba(1, size, size)?;
    let metallic = graph.render_output_channel_rgba(2, size, size)?;
    let mut pixels = vec![0u8; (size * size * 4) as usize];
    for y in 0..size {
        for x in 0..size {
            let p = ((y * size + x) * 4) as usize;
            pixels[p..p + 4].copy_from_slice(&[21, 25, 33, 255]);
            let sx = (x as f32 + 0.5) * 2.0 / size as f32 - 1.0;
            let sy = 1.0 - (y as f32 + 0.5) * 2.0 / size as f32;
            let r2 = sx * sx + sy * sy;
            if r2 >= 0.88 { continue; }
            let nz = (1.0 - r2 / 0.88).sqrt();
            let nx = sx / 0.88_f32.sqrt();
            let ny = sy / 0.88_f32.sqrt();
            let u = (nz.atan2(nx) / (2.0 * PI) + 0.5).rem_euclid(1.0);
            let v = (ny.asin() / PI + 0.5).clamp(0.0, 0.9999);
            let tx = (u * size as f32) as u32;
            let ty = (v * size as f32) as u32;
            let tex = ((ty * size + tx) * 4) as usize;
            let rough = roughness[tex] as f32 / 255.0;
            let metal = metallic[tex] as f32 / 255.0;
            let diffuse = (nx * 0.41 + ny * 0.6 + nz * 0.68).max(0.0);
            let half = (nx * 0.22 + ny * 0.34 + nz * 0.91).max(0.0);
            let spec = half.powf(5.0 + (1.0 - rough) * 100.0);
            for channel in 0..3 {
                let base = albedo[tex + channel] as f32 / 255.0;
                let reflectance = 0.04 * (1.0 - metal) + base * metal;
                let lit = base * (0.18 + 0.82 * diffuse) * (1.0 - 0.65 * metal)
                    + reflectance * spec * (1.0 - rough * 0.65);
                pixels[p + channel] = (lit.clamp(0.0, 1.0) * 255.0).round() as u8;
            }
        }
    }
    Ok(pixels)
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
