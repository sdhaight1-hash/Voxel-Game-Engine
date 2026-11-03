mod gpu_renderer;
use std::sync::Arc;
use gpu_renderer::{GpuViewport, Scene as GpuScene,LeafAaMode};
use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, TextureHandle, Vec2};
use std::{collections::HashMap, path::{Path, PathBuf}, time::Instant};
use texture_studio::{Graph, NodeKind, LibraryNode, VoxelShape, SideMapping, FaceSurface, FaceAxis};
use texture_studio::node_definitions::{SocketType, ParameterControl};
use texture_studio::{ColorRamp,MAX_COLOR_STOPS,CurveRemap,MAX_CURVE_POINTS,SdfBoolean};
use texture_studio::render_cache::GraphRenderCache;
use texture_studio::foliage_bake_cache::FoliageBakeCache;
use texture_studio::presets::{builtin_presets, Preset};
use texture_studio::foliage::{FoliageProject, FoliageMesh, FoliageStage, PlantPreset, Material,
    BranchGrowth, LeafGeometry, sway_leaf_vertex};
use texture_studio::asset::VoxelAsset;
use texture_studio::flora_prefab::FloraPrefab;
use texture_studio::material_functions::{MaterialFunction,MaterialFunctionLibrary,
    MaterialFunctionInstance,MaterialFunctionCall,FunctionOverride,FunctionParameter,
    FunctionParameterKind};
use texture_studio::foliage_materials::{FoliageMaterials, MaterialRole,
    SeasonalPalettes, LeafAlphaSampling, MATERIAL_SIZE};
use texture_studio::leaf_vector::LeafSilhouetteSource;
use texture_studio::voxel::{render_atlas, render_voxel_preview, export_obj};

/// Use the function's authored Noise frequency as the initial override;
/// selecting or importing a template must not silently change its appearance.
fn authored_function_noise_scale(function:&MaterialFunction)->Option<u32> {
    let param=function.parameters.iter().find(|param|
        matches!(param.kind,FunctionParameterKind::NoiseScale
            | FunctionParameterKind::NestedNoiseScale))?;
    match param.kind {
        FunctionParameterKind::NoiseScale =>
            match function.graph.node(param.node_id)?.kind {
                NodeKind::Noise{scale,..}=>Some(scale),
                _=>None,
            },
        FunctionParameterKind::NestedNoiseScale => function.calls.iter()
            .find(|call|call.placeholder_node_id==param.node_id)?
            .reference.overrides.iter()
            .find(|item|item.key==param.key).map(|item|item.value),
    }
}

/// Keeps GPU edge-quality modes mutually exclusive and never touches the
/// material bake, seasonal palette or scene-upload invalidation state.
fn leaf_aa_editor(ui:&mut egui::Ui,mode:&mut LeafAaMode,msaa4_supported:bool) {
    ui.horizontal_wrapped(|ui|{
        ui.label("GPU leaf edges");
        ui.selectable_value(mode,LeafAaMode::Legacy,"Legacy");
        ui.selectable_value(mode,LeafAaMode::Ordered,"Ordered 4×4");
        if msaa4_supported {
            ui.selectable_value(mode,LeafAaMode::Msaa4,"4× MSAA");
        } else {
            ui.add_enabled(false,egui::Button::new("4× MSAA"))
                .on_disabled_hover_text("4× MSAA is unsupported on this GPU or the GPU viewport is unavailable.");
        }
    });
    ui.small(match mode {
        LeafAaMode::Legacy=>"Default: original 0.5 leaf alpha cutoff.",
        LeafAaMode::Ordered=>"Single-sample screen-door coverage; possible visible dither pattern.",
        LeafAaMode::Msaa4=>"Four depth/color samples, hardware leaf alpha-to-coverage, resolved to the same preview texture.",
    });
}

const NODE_W: f32 = 170.0;
const PREVIEW_SIZE: u32 = 256;

/// Editing a palette changes only the GPU's color table, never the generated
/// hexel occupancy or baked leaf texture. Each material owns four seasons.
fn seasonal_palette_editor(ui:&mut egui::Ui,materials:&mut FoliageMaterials,
                           base:[u8;3])->bool {
    let mut palette=materials.effective_palettes(base);
    let mut changed=false;
    ui.collapsing("Seasonal leaf gradients",|ui|{
        ui.small("Four stops: shadow, base, midtone, highlight.");
        for (season,name) in SeasonalPalettes::NAMES.iter().enumerate() {
            ui.horizontal(|ui|{
                ui.label(*name);
                for (stop,label) in SeasonalPalettes::STOPS.iter().enumerate() {
                    let c=&mut palette.seasons[season][stop];
                    changed |= ui.color_edit_button_srgb(c).on_hover_text(*label).changed();
                }
            });
        }
        if ui.button("Reset all seasons to species defaults").clicked() {
            palette=SeasonalPalettes::for_species(materials.leaf_species,base);
            changed=true;
        }
    });
    if changed {materials.seasonal_palettes=Some(palette);}
    changed
}

/// Edit one tree's growth rules without replacing the selected species.
/// Geometry/morphology controls use the same bounds as FoliageProject::validate.
fn species_growth_editor(ui:&mut egui::Ui,tree:&mut FoliageProject)->bool {
    let base=tree.preset.growth_rules();
    let trunk_base=tree.preset.trunk_growth_rules();
    let mut rules=tree.effective_growth_rules();
    let mut trunk=tree.effective_trunk_rules();
    let mut edited=false;
    let mut reset=false;
    ui.collapsing(if tree.preset.is_palm() {
        "Palm growth rules (trunk, fronds, leaves)"
    } else {"Species growth rules (trunk, branches, twigs, leaves)"},|ui| {
        ui.small(format!("Growth pattern: {:?} • leaf arrangement: {:?}",
            base.phyllotaxy,base.foliage));
        ui.small("Growth patterns are locked to the species. Sliders stay inside its limits.");
        ui.separator();
        ui.strong("Trunk");
        edited |= ui.add(egui::Slider::new(&mut trunk.taper,
            (trunk_base.taper-0.12).max(0.0)..=
            (trunk_base.taper+0.12).min(0.85))
            .text("Taper")).changed();
        edited |= ui.add(egui::Slider::new(&mut trunk.base_flare,
            (trunk_base.base_flare-0.20).max(0.0)..=
            (trunk_base.base_flare+0.20).min(1.0))
            .text("Root flare")).changed();
        edited |= ui.add(egui::Slider::new(&mut trunk.height_seed_variation,
            0.0..=0.10).text("Seed height variation")).changed();
        edited |= ui.add(egui::Slider::new(&mut trunk.radius_seed_variation,
            0.0..=0.15).text("Seed width variation")).changed();
        edited |= ui.add(egui::Slider::new(&mut rules.lean_fraction,
            (base.lean_fraction-0.012).max(0.0)..=
            (base.lean_fraction+0.012).min(0.08))
            .text("Seed lean")).changed();
        ui.separator();
        ui.strong(if tree.preset.is_palm() {"Frond petioles"} else {"Primary branches"});
        edited |= ui.add(egui::Slider::new(&mut rules.primary_pitch[0],
            (base.primary_pitch[0]-0.20)..=(base.primary_pitch[0]+0.20))
            .text("Lowest pitch")).changed();
        edited |= ui.add(egui::Slider::new(&mut rules.primary_pitch[1],
            (base.primary_pitch[1]-0.20)..=(base.primary_pitch[1]+0.20))
            .text("Highest pitch")).changed();
        rules.primary_pitch[0]=rules.primary_pitch[0].min(rules.primary_pitch[1]);
        edited |= ui.add(egui::Slider::new(&mut rules.reach_factor,
            base.reach_factor*0.80..=base.reach_factor*1.20)
            .text("Crown reach")).changed();
        if !tree.preset.is_palm() {
        ui.separator();
        ui.strong("Twigs");
        edited |= ui.add(egui::Slider::new(&mut rules.secondary_pitch[0],
            (base.secondary_pitch[0]-0.20)..=(base.secondary_pitch[0]+0.20))
            .text("Lowest twig pitch")).changed();
        edited |= ui.add(egui::Slider::new(&mut rules.secondary_pitch[1],
            (base.secondary_pitch[1]-0.20)..=(base.secondary_pitch[1]+0.20))
            .text("Highest twig pitch")).changed();
        rules.secondary_pitch[0]=rules.secondary_pitch[0].min(rules.secondary_pitch[1]);
        edited |= ui.add(egui::Slider::new(&mut rules.extra_twig_chance,
            (base.extra_twig_chance-0.20).max(0.0)..=
            (base.extra_twig_chance+0.20).min(1.0))
            .text("Extra twig chance")).changed();
        } else {
            ui.small("Palms have no branching woody twigs; fronds emerge at the crown.");
        }
        ui.separator();
        ui.strong("Leaf clusters");
        edited |= ui.add(egui::Slider::new(&mut rules.foliage_factor,
            base.foliage_factor*0.80..=base.foliage_factor*1.20)
            .text("Cluster size multiplier")).changed();
        ui.small("Season colors and leaf cutouts stay in Foliage Palette.");
        if ui.button("Reset to species growth defaults").clicked() {
            reset=true;
        }
    });
    if reset {
        let changed=tree.growth_rules_override.is_some() ||
            tree.trunk_rules_override.is_some();
        tree.reset_growth_rules();
        return changed;
    }
    if edited {
        // The UI shares species bounds with JSON validation. Invalid values
        // cannot silently override a tree's canonical architecture.
        let growth_ok=tree.set_growth_rules(rules).is_ok();
        let trunk_ok=tree.set_trunk_rules(trunk).is_ok();
        return growth_ok && trunk_ok;
    }
    false
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum AssetTool { Select, Add, Erase, Paint }

fn cutout_hit(p:Pos2, screen:[Pos2;3], uv:[[f32;2];3],rgba:&[u8]) -> bool {
    let [a,b,c]=screen;
    let d=(b.y-c.y)*(a.x-c.x)+(c.x-b.x)*(a.y-c.y);
    if d.abs()<1e-5 {return false;}
    let w0=((b.y-c.y)*(p.x-c.x)+(c.x-b.x)*(p.y-c.y))/d;
    let w1=((c.y-a.y)*(p.x-c.x)+(a.x-c.x)*(p.y-c.y))/d;
    let w2=1.0-w0-w1;
    if w0<0.0 || w1<0.0 || w2<0.0 {return false;}
    let u=(w0*uv[0][0]+w1*uv[1][0]+w2*uv[2][0]).clamp(0.0,0.9999);
    let v=(w0*uv[0][1]+w1*uv[1][1]+w2*uv[2][1]).clamp(0.0,0.9999);
    let side=((rgba.len()/4) as f32).sqrt() as usize;
    if side==0 || side*side*4!=rgba.len() {return false;}
    let x=(u*side as f32) as usize;
    let y=(v*side as f32) as usize;
    rgba[(y*side+x)*4+3]>127
}

fn paint_voxel_triangle(
    painter:&egui::Painter,tri:&texture_studio::foliage::Triangle,
    points:[Pos2;3], color:Color32,texture:Option<&TextureHandle>,
    selected:bool,
) {
    if let Some(tex)=texture {
            let mut m=egui::Mesh::with_texture(tex.id());
            for (index,pos) in points.iter().enumerate() {
                m.vertices.push(egui::epaint::Vertex {
                    pos:*pos,uv:Pos2::new(tri.uv[index][0],tri.uv[index][1]),color,
                });
            }
            m.indices.extend_from_slice(&[0,1,2]);
            painter.add(egui::Shape::mesh(m));
    } else {
        painter.add(egui::Shape::convex_polygon(points.to_vec(),color,Stroke::NONE));
    }
    if selected {
        for edge in 0..3 {
            painter.line_segment([points[edge],points[(edge+1)%3]],
                Stroke::new(1.3,Color32::from_rgb(255,204,95)));
        }
    }
}

/// Render already-authored bark PBR outputs through the existing bounded
/// graph preview cache. Roughness and normal are UNORM data maps, not colors.
/// This runs only on GPU scene creation while opt-in preview is enabled.
fn preview_bark_pbr_maps(cache:&mut GraphRenderCache,materials:&FoliageMaterials)
    ->Result<(Vec<u8>,Vec<u8>),String> {
    let Some(graph)=&materials.bark else {
        let count=(MATERIAL_SIZE*MATERIAL_SIZE) as usize;
        return Ok((
            vec![204,204,204,255].repeat(count),
            vec![128,128,255,255].repeat(count),
        ));
    };
    let rough=cache.render_channel(graph,1,MATERIAL_SIZE,MATERIAL_SIZE)?;
    let normal=cache.render_normal(graph,MATERIAL_SIZE,MATERIAL_SIZE)?;
    Ok((rough.as_ref().to_vec(),normal.as_ref().to_vec()))
}

/// Bake the same material images used by the preview and the static OBJ.
fn write_foliage_textures(folder:&Path,materials:&FoliageMaterials,
                          bark:[u8;3], leaf:[u8;3], cutout:bool)->Result<(),String> {
    let size=MATERIAL_SIZE;
    if materials.bark.is_some() {
        save_png_rect(&folder.join("bark-albedo.png"),size,size,
            &materials.bake_bark(size,bark)?)?;
    }
    if cutout {
        save_png_rect(&folder.join("foliage-leaves.png"),size,size,
            &materials.bake_leaf(size,leaf)?)?;
        save_png_rect(&folder.join("foliage-leaves-alpha.png"),size,size,
            &materials.bake_leaf_opacity(size,leaf)?)?;
    }
    for (name,tex) in materials.bake_pbr(size)? {
        save_png_rect(&folder.join(name),size,size,&tex)?;
    }
    Ok(())
}

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

/// Counts actual editor preview operations, not export-time or background work.
#[derive(Default)]
struct AuthoringMetrics {
    leaf_bake_attempts: u64,
    bark_bake_attempts: u64,
    foliage_generations: u64,
    asset_mesh_builds: u64,
    foliage_scene_builds: u64,
    asset_scene_builds: u64,
}

impl AuthoringMetrics {
    fn report(&self, event: &str, elapsed: std::time::Duration, ok: bool) {
        if std::env::var_os("VOXEL_PROFILE_MATERIALS").is_some() {
            eprintln!(
                "[material-profile] event={event} ok={ok} cpu_ms={:.3} leaf_bakes={} bark_bakes={} foliage_generations={} asset_mesh_builds={} foliage_scene_builds={} asset_scene_builds={}",
                elapsed.as_secs_f64() * 1000.0,
                self.leaf_bake_attempts, self.bark_bake_attempts,
                self.foliage_generations, self.asset_mesh_builds,
                self.foliage_scene_builds, self.asset_scene_builds,
            );
        }
    }
}

struct Studio {
    metrics: AuthoringMetrics,
    gpu: Option<GpuViewport>,
    foliage_gpu_scene: Option<Arc<GpuScene>>,
    asset_gpu_scene: Option<Arc<GpuScene>>,
    graph: Graph,
    /// Bounded CPU RGBA cache for Nodes output, selected-node and thumbnail previews.
    preview_render_cache: GraphRenderCache,
    /// Shared across Foliage and Asset editors, keyed by authored bake inputs.
    foliage_bake_cache: FoliageBakeCache,
    foliage: FoliageProject,
    foliage_mesh: Option<FoliageMesh>,
    foliage_dirty: bool,
    foliage_stage: FoliageStage,
    foliage_yaw: f32,
    /// 0 Spring, 1 Summer, 2 Autumn, 3 Winter; 4 loops to Spring.
    season_preview: f32,
    /// GPU preview only; never changes saved foliage geometry or textures.
    leaf_aa_mode: LeafAaMode,
    /// Preview-only mip selection; legacy sample level zero remains the default.
    leaf_mipmaps: bool,
    /// Opt-in GPU dielectric PBR for bark; does not persist in saved trees.
    gpu_bark_pbr: bool,
    leaf_texture: Option<TextureHandle>,
    bark_texture: Option<TextureHandle>,
    asset_leaf_texture: Option<TextureHandle>,
    asset_bark_texture: Option<TextureHandle>,
    foliage_leaf_pixels: Vec<u8>,
    foliage_bark_pixels: Vec<u8>,
    asset_leaf_pixels: Vec<u8>,
    asset_bark_pixels: Vec<u8>,
    foliage_mats_dirty: bool,
    asset_mats_dirty: bool,
    material_role: MaterialRole,
    material_search: String,
    material_functions: MaterialFunctionLibrary,
    function_id_input: String,
    function_version_input: u32,
    selected_material_function: Option<usize>,
    function_placeholder_node: Option<u64>,
    function_noise_scale: u32,
    editing_material: Option<(bool,MaterialRole)>,
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
            metrics: AuthoringMetrics::default(),
            gpu: None,
            foliage_gpu_scene: None,
            asset_gpu_scene: None,
            graph: Graph::demo_bricks(),
            preview_render_cache: GraphRenderCache::new(64*1024*1024),
            foliage_bake_cache: FoliageBakeCache::new(32*1024*1024),
            foliage: FoliageProject::default(),
            foliage_mesh: None,
            foliage_dirty: true,
            foliage_stage: FoliageStage::PresetLoader,
            foliage_yaw: 0.8,
            season_preview: 1.0,
            leaf_aa_mode: LeafAaMode::Legacy,
            leaf_mipmaps: false,
            gpu_bark_pbr: false,
            leaf_texture: None,
            bark_texture: None,
            asset_leaf_texture: None,
            asset_bark_texture: None,
            foliage_leaf_pixels: Vec::new(),
            foliage_bark_pixels: Vec::new(),
            asset_leaf_pixels: Vec::new(),
            asset_bark_pixels: Vec::new(),
            foliage_mats_dirty: true,
            asset_mats_dirty: true,
            material_role: MaterialRole::Leaf,
            material_search: String::new(),
            material_functions: MaterialFunctionLibrary::default(),
            function_id_input:"shared_bark".into(),
            function_version_input:1,
            selected_material_function:None,
            function_placeholder_node:None,
            function_noise_scale:6,
            editing_material: None,
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
            library_tab: 3,
            preset_search: String::new(),
            preset_category: "All categories".into(),
            preset_selected: None,
            presets: builtin_presets().expect("bundled preset catalog is checked by tests"),
            dirty: true,
            export_size: 1024,
            project_path: None,
            status: "Voxel asset editor: select a tool, click blocks to edit, or import a procedural tree.".to_owned(),
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
                    self.preview_render_cache.render_normal(
                        &self.graph, PREVIEW_SIZE, PREVIEW_SIZE)
                } else {
                    self.preview_render_cache.render_channel(
                        &self.graph,self.preview_channel, PREVIEW_SIZE, PREVIEW_SIZE)
                };
                match rendered {
                    Ok(pixels) => put_texture(ctx, &mut self.preview,
                        "output-preview", pixels.as_ref().to_vec(), PREVIEW_SIZE),
                    Err(err) => self.status = format!("Preview error: {err}"),
                }
            }
            2 => {
                if let Some(id) = self.selected {
                    if let Ok(pixels) = self.preview_render_cache.render_node(
                        &self.graph,id,PREVIEW_SIZE,PREVIEW_SIZE) {
                        put_texture(ctx, &mut self.selected_preview,
                            "selected-preview", pixels.as_ref().to_vec(), PREVIEW_SIZE);
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
            if let Ok(pixels) = self.preview_render_cache.render_node(
                &self.graph,node.id,64,64) {
                let image = egui::ColorImage::from_rgba_unmultiplied([64, 64], pixels.as_ref());
                if let Some(texture) = self.thumbs.get_mut(&node.id) {
                    texture.set(image, egui::TextureOptions::LINEAR);
                } else {
                    self.thumbs.insert(node.id, ctx.load_texture(
                        format!("node-thumb-{}", node.id), image, egui::TextureOptions::LINEAR
                    ));
                }
            }
        }
        if std::env::var_os("VOXEL_PROFILE_MATERIALS").is_some() {
            let stats=self.preview_render_cache.stats();
            eprintln!(
                "[material-profile] event=node_render_cache hits={} misses={} evictions={} entries={} resident_bytes={} peak_bytes={}",
                stats.hits,stats.misses,stats.evictions,stats.entries,
                stats.resident_bytes,stats.peak_resident_bytes,
            );
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

    fn export_antialiased_png(&mut self) {
        if self.preview_channel>=4 || self.export_size>1024 {
            self.status="AA export supports Albedo/Roughness/Metallic/Height up to 1024px".into();
            return;
        }
        let Some(path)=rfd::FileDialog::new()
            .add_filter("PNG image",&["png"])
            .set_file_name("texture-aa.png").save_file() else {
            return;
        };
        let size=self.export_size;
        let result=self.graph.render_output_channel_rgba_ssaa2x(
            self.preview_channel,size,size)
            .and_then(|pixels|save_png(&path,size,&pixels));
        self.status=match result {
            Ok(())=>format!("2x2 antialiased PNG exported: {}",path.display()),
            Err(error)=>format!("AA export failed: {error}"),
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
            // Opt-in until hardware and preset-level export benchmarks justify
            // making shared-channel memoization the default path.
            if std::env::var_os("VOXEL_MEMOIZED_PBR_EXPORT").is_some() {
                let started=Instant::now();
                let (images,stats)=self.graph.render_channels_memoized_rgba(
                    &[0,1,2,3],size,size)?;
                for (name,pixels) in ["albedo","roughness","metallic","height"]
                    .iter().zip(images.iter()) {
                    save_png(&folder.join(format!("{name}.png")),size,pixels)?;
                }
                if std::env::var_os("VOXEL_PROFILE_MATERIALS").is_some() {
                    eprintln!("[material-profile] event=memo_pbr_export cpu_ms={:.3} sample_hits={} node_evaluations={} peak_entries_per_pixel={}",
                        started.elapsed().as_secs_f64()*1000.0,
                        stats.reused_samples,stats.evaluated_samples,stats.peak_entries_per_pixel);
                }
            } else {
                for (channel,name) in ["albedo","roughness","metallic","height"]
                    .iter().enumerate() {
                    let pixels=self.graph.render_output_channel_rgba(channel,size,size)?;
                    save_png(&folder.join(format!("{name}.png")),size,&pixels)?;
                }
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

    fn import_png_for_node(&mut self,node_id:u64) {
        let Some(path)=rfd::FileDialog::new()
            .add_filter("PNG image",&["png"]).pick_file() else {return;};
        let result=(||->Result<String,String>{
            let file_size=std::fs::metadata(&path)
                .map_err(|error|error.to_string())?.len();
            if file_size>texture_studio::bitmap_assets::MAX_PNG_BYTES as u64 {
                return Err("PNG source exceeds 4 MiB".into());
            }
            let bytes=std::fs::read(&path).map_err(|error|error.to_string())?;
            let filename=path.file_name().and_then(|name|name.to_str())
                .unwrap_or("bitmap.png");
            let image_id=self.graph.import_png_bytes(filename,&bytes)?;
            let node=self.graph.node_mut(node_id)
                .ok_or("Selected bitmap node no longer exists")?;
            match &mut node.kind {
                NodeKind::BitmapColor{asset_id,..} |
                NodeKind::BitmapAlpha{asset_id,..} => *asset_id=image_id,
                _=>return Err("Selected node is not a bitmap source".into()),
            }
            Ok(format!("Imported PNG as embedded bitmap #{image_id}; project is portable"))
        })();
        match result {
            Ok(status)=>{self.dirty=true;self.status=status;},
            Err(error)=>self.status=format!("PNG import failed: {error}"),
        }
    }

    fn spawn(&mut self, kind: NodeKind) {
        let offset = self.graph.nodes.len() as f32 * 24.0;
        let id = self.graph.add_node(kind, [80.0 + offset, 235.0 + offset]);
        self.selected = Some(id);
        self.dirty = true;
        self.status = format!("Added {} node", kind.name());
    }

    fn new(cc:&eframe::CreationContext<'_>)->Self {
        let mut app=Self::default();
        app.gpu=GpuViewport::new(cc);
        if let Some(gpu)=&app.gpu {
            app.status=format!("wgpu {:?} renderer on {}",gpu.backend,gpu.adapter);
        } else {
            app.status="wgpu initialization unavailable".into();
        }
        app
    }

    fn new_asset(&mut self) {
        self.asset = VoxelAsset::default();
        self.asset_path = None;
        self.asset_selected = Some([0, 0, 0]);
        self.asset_cursor = [0, 0, 1];
        self.asset_undo.clear();
        self.asset_redo.clear();
        self.asset_dirty = true;
        self.asset_mats_dirty = true;
        self.status = "New editable cube voxel asset".into();
    }

    fn import_tree_as_asset(&mut self) {
        match self.foliage.generate() {
            Ok(mesh) => {
                self.asset = VoxelAsset::from_tree(&self.foliage, &mesh);
                self.asset_selected = mesh.voxels.first().map(|v| [v.x, v.y, v.z]);
                self.asset_undo.clear();
                self.asset_redo.clear();
                self.asset_path = None;
                self.asset_dirty = true;
                self.asset_mats_dirty = true;
                self.library_tab = 3;
                self.status = format!("Created {}: {} editable standard 1m hexels",
                    self.foliage.preset.name(), mesh.voxel_count());
            }
            Err(e) => self.status = format!("Could not create hexel tree: {e}"),
        }
    }

    /// Each successful edit is an atomic undo step; invalid edits keep the old asset.
    fn edit_asset(&mut self, edit: impl FnOnce(&mut VoxelAsset) -> Result<(), String>) {
        let original = self.asset.clone();
        match edit(&mut self.asset) {
            Ok(()) => {
                // Reject invalid shapes/parameters before recording a change.
                if let Err(e) = self.asset.validate() {
                    self.asset = original;
                    self.status = format!("Asset edit rejected: {e}");
                    return;
                }
                if self.asset_undo.len() >= 20 { self.asset_undo.remove(0); }
                self.asset_undo.push(original);
                self.asset_redo.clear();
                self.asset_dirty = true;
            }
            Err(e) => {
                self.asset = original;
                self.status = format!("Edit rejected: {e}");
            }
        }
    }

    /// Undo/redo should rebake only when stored material inputs changed.
    fn asset_bake_inputs_changed(a:&VoxelAsset,b:&VoxelAsset)->bool {
        a.bark_color!=b.bark_color || a.leaf_color!=b.leaf_color
            || serde_json::to_value(&a.materials).ok()
                != serde_json::to_value(&b.materials).ok()
    }

    fn undo_asset(&mut self) {
        if let Some(previous) = self.asset_undo.pop() {
            let material_changed=Self::asset_bake_inputs_changed(&self.asset,&previous);
            self.asset_redo.push(std::mem::replace(&mut self.asset, previous));
            self.asset_dirty = true;
            self.asset_mats_dirty |= material_changed;
            self.status = "Undid voxel edit".into();
        }
    }

    fn redo_asset(&mut self) {
        if let Some(next) = self.asset_redo.pop() {
            let material_changed=Self::asset_bake_inputs_changed(&self.asset,&next);
            self.asset_undo.push(std::mem::replace(&mut self.asset, next));
            self.asset_dirty = true;
            self.asset_mats_dirty |= material_changed;
            self.status = "Redid voxel edit".into();
        }
    }

    fn save_asset(&mut self, save_as: bool) {
        let path = if !save_as { self.asset_path.clone() } else { None }
            .or_else(||rfd::FileDialog::new()
                .add_filter("Texture Studio voxel asset", &["json"])
                .set_file_name("asset.voxelasset.json").save_file());
        let Some(path) = path else { return; };
        let result = self.asset.to_json()
            .and_then(|json|std::fs::write(&path, json).map_err(|e|e.to_string()));
        self.status = match result {
            Ok(()) => {
                self.asset_path = Some(path.clone());
                format!("Saved editable voxel asset: {}",path.display())
            }
            Err(e) => format!("Could not save asset: {e}"),
        };
    }

    fn open_asset(&mut self) {
        let Some(path) = rfd::FileDialog::new().add_filter("Voxel asset", &["json"])
            .pick_file() else { return; };
        let result = std::fs::read_to_string(&path).map_err(|e|e.to_string())
            .and_then(|json|VoxelAsset::from_json(&json));
        match result {
            Ok(asset) => {
                self.asset_selected = asset.voxels.first().map(|v|[v.x,v.y,v.z]);
                self.asset = asset;
                self.asset_path = Some(path);
                self.asset_undo.clear();
                self.asset_redo.clear();
                self.asset_dirty = true;
                self.asset_mats_dirty = true;
                self.status = "Loaded editable voxel asset".into();
            }
            Err(e) => self.status = format!("Could not open asset: {e}"),
        }
    }

    fn export_asset(&mut self) {
        let Some(folder) = rfd::FileDialog::new().pick_folder() else { return; };
        let result: Result<(),String> = (|| {
            let names = ["asset.voxelasset.json", "asset.obj", "asset.mtl",
                "foliage-leaves.png", "foliage-leaves-alpha.png",
                "bark-albedo.png", "bark-roughness.png", "bark-normal.png",
                "foliage-leaves-roughness.png", "foliage-leaves-normal.png"];
            for name in names {
                if folder.join(name).exists() {
                    return Err(format!("{name} already exists — choose an empty folder"));
                }
            }
            let files = [
                ("asset.voxelasset.json", self.asset.to_json()?),
                ("asset.obj", self.asset.obj()?),
                ("asset.mtl", self.asset.mtl()),
            ];
            for (name, contents) in files {
                std::fs::write(folder.join(name), contents).map_err(|e|e.to_string())?;
            }
            write_foliage_textures(&folder,&self.asset.materials,
                self.asset.bark_color,self.asset.leaf_color,
                self.asset.leaf_render.geometry==LeafGeometry::CrossedCutout)?;
            Ok(())
        })();
        self.status = match result {
            Ok(()) => format!("Exported voxel data + OBJ/MTL to {}",folder.display()),
            Err(e) => format!("Export asset failed: {e}"),
        };
    }

    fn asset_outliner(&mut self, ui: &mut egui::Ui) {
        ui.heading("ASSET OUTLINER");
        ui.label(egui::RichText::new(&self.asset.name).strong());
        ui.small(format!("{} cells • {}",self.asset.voxels.len(),self.asset.shape.name()));
        ui.separator();
        if ui.button("Import Procedural Tree").clicked() {
            self.import_tree_as_asset();
        }
        ui.small("Tree generator → editable voxel cells. The copy can be edited independently.");
        ui.separator();
        for material in [Material::Bark,Material::Leaf,Material::LeafDark,Material::LeafLight] {
            let count = self.asset.voxels.iter().filter(|v|v.material==material).count();
            let label = match material {
                Material::Bark=>"Bark / body",Material::Leaf=>"Foliage / green",
                Material::LeafDark=>"Foliage / dark",Material::LeafLight=>"Foliage / light",
            };
            ui.small(format!("{label}: {count}"));
        }
        ui.separator();
        ui.label(egui::RichText::new("CELLS").strong());
        egui::ScrollArea::vertical().id_salt("asset_voxel_outliner")
            .show_rows(ui, 22.0, self.asset.voxels.len(), |ui, rows| {
                for index in rows {
                    let c = self.asset.voxels[index];
                    let pos = [c.x,c.y,c.z];
                    if ui.selectable_label(self.asset_selected==Some(pos),
                        format!("{:>3}, {:>3}, {:>3}   {:?}",c.x,c.y,c.z,c.material)).clicked() {
                        self.asset_selected = Some(pos);
                        self.asset_cursor = pos;
                        self.status = format!("Selected voxel ({}, {}, {})",c.x,c.y,c.z);
                    }
                }
            });
        ui.small("Click a cell to select; use the right inspector to move or paint it.");
    }

    fn material_controls(&mut self, ui:&mut egui::Ui, for_asset:bool) {
        ui.separator();
        ui.heading("PROCEDURAL MATERIAL SLOTS");
        ui.horizontal_wrapped(|ui| {
            for role in [MaterialRole::Bark,MaterialRole::Leaf,MaterialRole::Opacity] {
                ui.selectable_value(&mut self.material_role,role,role.name());
            }
        });
        let slot=self.material_role;
        let mat=if for_asset {&self.asset.materials} else {&self.foliage.materials};
        let assigned=match slot {
            MaterialRole::Bark=>mat.bark_preset.as_deref().unwrap_or("Custom / flat"),
            MaterialRole::Leaf=>mat.leaf_preset.as_deref().unwrap_or("Custom / flat"),
            MaterialRole::Opacity=>if mat.leaf_opacity.is_some() {"Custom mask graph"}
                else {"Original silhouette"},
        };
        ui.small(format!("Assigned: {assigned}"));
        ui.add(egui::TextEdit::singleline(&mut self.material_search)
            .hint_text("Find bark or leaf preset..."));
        let query=self.material_search.to_lowercase();
        let candidates:Vec<(usize,String)>=self.presets.iter().enumerate()
            .filter(|(_,p)| {
                if !query.is_empty() {p.matches(&query)}
                else {
                    let v=p.variant.to_lowercase();
                    (match slot {
                        MaterialRole::Bark=>v.contains("bark")||v.contains("trunk"),
                        MaterialRole::Leaf|MaterialRole::Opacity=>
                            v.contains("leaves")||v.contains("foliage")
                            ||v.contains("needle"),
                    }) && matches!(p.profile.as_str(),
                        "wood"|"foliage"|"fruit"|"grass"|"crop"|"flower")
                }
            }).take(40).map(|(i,p)|(i,p.title())).collect();
        let mut chosen=None;
        egui::ScrollArea::vertical().id_salt("slot_preset_choices")
            .max_height(112.0).show(ui,|ui| {
                for (index,label) in candidates {
                    if ui.selectable_label(false,label).clicked() {
                        chosen=Some(index);
                    }
                }
            });
        if let Some(i)=chosen {
            let preset=self.presets[i].clone();
            if for_asset {
                self.asset.materials.assign_preset(slot,&preset);
                self.asset_mats_dirty=true;
            } else {
                self.foliage.materials.assign_preset(slot,&preset);
                self.foliage_mats_dirty=true;
            }
            self.status=format!("Assigned {} to {}",preset.title(),slot.name());
        }
        let mat=if for_asset {&mut self.asset.materials}
            else {&mut self.foliage.materials};
        if ui.add(egui::Slider::new(&mut mat.cutout_threshold,0.0..=1.0)
            .text("Cutout threshold")).changed() {
            if for_asset {self.asset_mats_dirty=true;}
            else {self.foliage_mats_dirty=true;}
        }
        ui.small("Leaf Pattern controls luminance/detail; species and seasonal palettes control the base color.");
        ui.small("Transparency keeps the selected base leaf silhouette; an opacity graph only removes coverage.");
        let silhouette_ui=match mat.leaf_species {
            PlantPreset::Oak=>Some(("Oak leaf-card silhouette","Oak")),
            PlantPreset::Willow=>Some(("Willow leaf-card silhouette","Willow")),
            PlantPreset::EasternCottonwood|PlantPreset::FremontCottonwood
                |PlantPreset::BlackCottonwood=>
                Some(("Cottonwood leaf-card silhouette","Cottonwood")),
            _=>None,
        };
        if let Some((title,name))=silhouette_ui {
            ui.collapsing(title,|ui| {
                let mut vector=!matches!(mat.leaf_silhouette,
                    LeafSilhouetteSource::LegacySpecies);
                if ui.checkbox(&mut vector,format!("Use vector {name} leaf silhouette"))
                    .changed() {
                    mat.leaf_silhouette=if !vector {
                        mat.leaf_alpha_sampling=LeafAlphaSampling::Binary;
                        LeafSilhouetteSource::LegacySpecies
                    } else {
                        match mat.leaf_species {
                            PlantPreset::Oak=>LeafSilhouetteSource::VectorOak{
                                lobe_width:1.0,
                            },
                            PlantPreset::Willow=>LeafSilhouetteSource::VectorWillow{
                                blade_width:1.0,
                            },
                            _=>LeafSilhouetteSource::VectorCottonwood{
                                blade_width:1.0,
                            },
                        }
                    };
                    if for_asset {self.asset_mats_dirty=true;}
                    else {self.foliage_mats_dirty=true;}
                }
                let width=match &mut mat.leaf_silhouette {
                    LeafSilhouetteSource::VectorOak{lobe_width}=>
                        Some(("Lobe width",lobe_width)),
                    LeafSilhouetteSource::VectorWillow{blade_width}|
                        LeafSilhouetteSource::VectorCottonwood{blade_width}=>
                        Some(("Blade width",blade_width)),
                    LeafSilhouetteSource::LegacySpecies=>None,
                };
                if let Some((label,width))=width {
                    if ui.add(egui::Slider::new(width,0.65..=1.45)
                        .text(label)).changed() {
                        if for_asset {self.asset_mats_dirty=true;}
                        else {self.foliage_mats_dirty=true;}
                    }
                    let mut covered=mat.leaf_alpha_sampling==
                        LeafAlphaSampling::Coverage2x2;
                    if ui.checkbox(&mut covered,"2×2 vector silhouette coverage")
                        .on_hover_text("Bake four subpixel occupancy samples per 64px leaf texel. Fractional alpha is best viewed with Experimental ordered leaf edge coverage enabled in the 3D preview.").changed() {
                        mat.leaf_alpha_sampling=if covered {
                            LeafAlphaSampling::Coverage2x2
                        } else {
                            LeafAlphaSampling::Binary
                        };
                        if for_asset {self.asset_mats_dirty=true;}
                        else {self.foliage_mats_dirty=true;}
                    }
                    ui.small("Replaces the base silhouette; leaf tint, seasonal palettes and tree geometry stay independent.");
                }
            });
        }
        if slot==MaterialRole::Bark {
            // This is a GPU appearance uniform. Geometry, bark bake and
            // leaf/season resources remain untouched when only scale changes.
            let mut physical=mat.bark_meters_per_repeat.is_some();
            let mut mapping_changed=ui.checkbox(&mut physical,
                "Physical bark repeats (GPU preview)").changed();
            if mapping_changed {
                mat.bark_meters_per_repeat=if physical {Some(0.45)} else {None};
            }
            if let Some(meters)=&mut mat.bark_meters_per_repeat {
                mapping_changed|=ui.add(egui::Slider::new(meters,0.05..=8.0)
                    .text("Meters per bark repeat")).changed();
                ui.small("Physical tube circumference + branch arc length; seam-corrected.");
            }
            if mapping_changed {
                if for_asset {self.asset_gpu_scene=None;}
                else {self.foliage_gpu_scene=None;}
            }
            ui.small("Block-like wood and thin twig ribbons retain legacy UVs.");
        }
        if slot==MaterialRole::Bark && mat.bark.is_some() {
            if ui.add(egui::Slider::new(&mut mat.bark_tint_strength,0.0..=1.0)
                .text("Bark tint mix")).changed() {
                if for_asset {self.asset_mats_dirty=true;}
                else {self.foliage_mats_dirty=true;}
            }
            ui.small("0: original graph colors · 1: graph detail colored by tree bark color");
        }
        ui.horizontal(|ui| {
            if ui.button("Edit slot in Nodes").clicked() {
                let mats=if for_asset {&self.asset.materials}
                    else {&self.foliage.materials};
                self.graph=mats.graph(slot).cloned().unwrap_or_default();
                self.editing_material=Some((for_asset,slot));
                self.library_tab=0;
                self.selected=self.graph.nodes.first().map(|n|n.id);
                self.dirty=true;
                self.status=format!("Edit {} then click Apply in the top toolbar",slot.name());
            }
            if slot==MaterialRole::Opacity && ui.button("Clear opacity graph").clicked() {
                if for_asset {
                    self.asset.materials.leaf_opacity=None;
                    self.asset.materials.opacity_function=None;
                    self.asset_mats_dirty=true;
                } else {
                    self.foliage.materials.leaf_opacity=None;
                    self.foliage.materials.opacity_function=None;
                    self.foliage_mats_dirty=true;
                }
            }
        });
        if !for_asset && ui.button("Restore species bark + leaf presets").clicked() {
            self.foliage.materials=FoliageMaterials::for_species(self.foliage.preset);
            self.foliage_mats_dirty=true;
        }
        self.material_function_controls(ui,for_asset,slot);
    }

    fn material_function_controls(&mut self,ui:&mut egui::Ui,
        for_asset:bool,role:MaterialRole) {
        ui.collapsing("Reusable material functions",|ui| {
            ui.small("Save a graph template as JSON, then import and apply it to other species or assets.");
            ui.small("Each instance stores a resolved graph; edits to a template do not silently rewrite saved trees.");
            ui.horizontal(|ui| {
                ui.label("Function ID");
                ui.text_edit_singleline(&mut self.function_id_input);
            });
            ui.add(egui::DragValue::new(&mut self.function_version_input)
                .range(1..=10000).prefix("Version "));
            if ui.button("Export selected slot as function…").clicked() {
                let mat=if for_asset {&self.asset.materials} else {&self.foliage.materials};
                if let Some(graph)=mat.graph(role).cloned() {
                    match MaterialFunction::from_graph(self.function_id_input.clone(),
                        self.function_version_input,graph) {
                        Err(err)=>self.status=format!("Function error: {err}"),
                        Ok(function)=> {
                            if self.material_functions.functions.iter().any(|f|
                                f.id==function.id && f.version==function.version) {
                                self.status="That function ID/version is already loaded; increase its version".into();
                            } else if let Some(path)=rfd::FileDialog::new()
                                .add_filter("Material function", &["json"])
                                .set_file_name(format!("{}-v{}.json",
                                    function.id,function.version)).save_file() {
                                if path.exists() {
                                    self.status="Function file exists; choose another filename to preserve old versions".into();
                                } else {
                                    let result=serde_json::to_string_pretty(&function)
                                        .map_err(|e|e.to_string())
                                        .and_then(|json|std::fs::write(&path,json)
                                            .map_err(|e|e.to_string()));
                                    match result {
                                        Ok(())=>{
                                            let default_scale=authored_function_noise_scale(&function);
                                            let index=self.material_functions.insert(function)
                                                .expect("validated and unique export");
                                            self.selected_material_function=Some(index);
                                            if let Some(scale)=default_scale {
                                                self.function_noise_scale=scale;
                                            }
                                            self.status=format!("Saved material function {}",path.display());
                                        }
                                        Err(err)=>self.status=format!("Function save failed: {err}"),
                                    }
                                }
                            }
                        }
                    }
                } else {
                    self.status="Assign or edit a material graph before exporting a function".into();
                }
            }
            if ui.button("Import material function…").clicked() {
                if let Some(path)=rfd::FileDialog::new()
                    .add_filter("Material function", &["json"]).pick_file() {
                    let result=std::fs::read_to_string(&path).map_err(|e|e.to_string())
                        .and_then(|json|serde_json::from_str::<MaterialFunction>(&json)
                            .map_err(|e|e.to_string()))
                        .and_then(|function|self.material_functions.insert(function));
                    match result {
                        Ok(index)=>{
                            self.selected_material_function=Some(index);
                            if let Some(scale)=self.material_functions.functions.get(index)
                                .and_then(authored_function_noise_scale) {
                                self.function_noise_scale=scale;
                            }
                            self.status=format!("Imported material function {}",path.display());
                        }
                        Err(err)=>self.status=format!("Function import failed: {err}"),
                    }
                }
            }
            let mut function_selection_changed=false;
            egui::ComboBox::from_id_salt("material_function_selection")
                .selected_text(self.selected_material_function
                    .and_then(|i|self.material_functions.functions.get(i))
                    .map_or("Select imported function".into(),
                        |f|format!("{} v{}",f.name,f.version)))
                .show_ui(ui,|ui|{
                    for (index,f) in self.material_functions.functions.iter().enumerate() {
                        function_selection_changed |= ui.selectable_value(
                            &mut self.selected_material_function,
                            Some(index),format!("{} v{}",f.name,f.version)).changed();
                    }
                });
            if function_selection_changed {
                if let Some(scale)=self.selected_material_function
                    .and_then(|index|self.material_functions.functions.get(index))
                    .and_then(authored_function_noise_scale) {
                    self.function_noise_scale=scale;
                }
            }
            if let Some(index)=self.selected_material_function {
                if let Some(function)=self.material_functions.functions.get(index) {
                    let frequency=function.parameters.iter().find(|p|
                        matches!(p.kind,FunctionParameterKind::NoiseScale
                            |FunctionParameterKind::NestedNoiseScale));
                    if frequency.is_some() {
                        ui.add(egui::Slider::new(&mut self.function_noise_scale,1..=64)
                            .text("Instance noise frequency"));
                    }
                    if ui.button(format!("Apply to {}",role.name())).clicked() {
                        let overrides=frequency.map(|p|vec![FunctionOverride{
                            key:p.key.clone(),value:self.function_noise_scale,
                        }]).unwrap_or_default();
                        let reference=MaterialFunctionInstance{
                            function_id:function.id.clone(),version:function.version,overrides,
                        };
                        let materials=if for_asset {&mut self.asset.materials}
                            else {&mut self.foliage.materials};
                        match materials.apply_function(role,&self.material_functions,reference) {
                            Ok(())=>{
                                if for_asset {self.asset_mats_dirty=true;}
                                else {self.foliage_mats_dirty=true;}
                                self.status=format!("Applied versioned function to {}",role.name());
                            }
                            Err(err)=>self.status=format!("Function apply failed: {err}"),
                        }
                    }
                }
            }
            // Nested functions compile into the parent graph at authoring time.
            // Color placeholders are ordinary serializable nodes; no runtime
            // Group/Call enum variants or shader branch handling are required.
            let selected_child=self.selected_material_function
                .and_then(|index|self.material_functions.functions.get(index))
                .cloned();
            let slot_graph=if for_asset {&self.asset.materials}
                else {&self.foliage.materials};
            let slot_graph=slot_graph.graph(role).cloned();
            if let (Some(child),Some(parent_graph))=(selected_child,slot_graph) {
                let placeholders:Vec<_>=parent_graph.nodes.iter()
                    .filter(|node|matches!(node.kind,NodeKind::Color{..}))
                    .map(|node|node.id).collect();
                if !placeholders.is_empty() {
                    ui.separator();
                    ui.label("Nested material function");
                    ui.small("Replace a Color node in the assigned graph with the selected function's Albedo output.");
                    if !placeholders.contains(
                        &self.function_placeholder_node.unwrap_or(u64::MAX)) {
                        self.function_placeholder_node=placeholders.first().copied();
                    }
                    egui::ComboBox::from_id_salt("nested_material_placeholder")
                        .selected_text(format!("Color node #{}",
                            self.function_placeholder_node.unwrap_or_default()))
                        .show_ui(ui,|ui| {
                            for id in &placeholders {
                                ui.selectable_value(&mut self.function_placeholder_node,
                                    Some(*id),format!("Color node #{id}"));
                            }
                        });
                    if ui.button("Export slot as nested function…").clicked() {
                        let result=(|| ->Result<MaterialFunction,String> {
                            let mut parent=MaterialFunction::from_graph(
                                self.function_id_input.clone(),
                                self.function_version_input,parent_graph)?;
                            let overrides=child.parameters.iter().find(|param|
                                matches!(param.kind,FunctionParameterKind::NoiseScale
                                    |FunctionParameterKind::NestedNoiseScale))
                                .map(|param|vec![FunctionOverride{
                                    key:param.key.clone(),
                                    value:self.function_noise_scale,
                                }]).unwrap_or_default();
                            let reference=MaterialFunctionInstance{
                                function_id:child.id,version:child.version,overrides,
                            };
                            let placeholder_node_id=self.function_placeholder_node
                                .ok_or("Select a Color placeholder node")?;
                            let forwarded=child.parameters.iter()
                                .find(|param|matches!(param.kind,
                                    FunctionParameterKind::NoiseScale
                                    |FunctionParameterKind::NestedNoiseScale))
                                .map(|param|FunctionParameter{
                                    key:param.key.clone(),node_id:placeholder_node_id,
                                    kind:FunctionParameterKind::NestedNoiseScale,
                                });
                            parent.calls.push(MaterialFunctionCall{
                                placeholder_node_id,reference,
                            });
                            // A parent function's override must alter the
                            // nested child, never an unused local Noise node.
                            parent.parameters=forwarded.into_iter().collect();
                            parent.validate()?;
                            // Resolve and validate before showing the save dialog.
                            // Failed references/cycles never produce a file.
                            let mut candidate=self.material_functions.clone();
                            candidate.insert(parent.clone())?;
                            candidate.resolve(&MaterialFunctionInstance{
                                function_id:parent.id.clone(),
                                version:parent.version,overrides:Vec::new(),
                            })?;
                            Ok(parent)
                        })();
                        match result {
                            Err(err)=>self.status=format!("Nested function: {err}"),
                            Ok(parent)=>{
                                if let Some(path)=rfd::FileDialog::new()
                                    .add_filter("Material function",&["json"])
                                    .set_file_name(format!("{}-v{}.json",
                                        parent.id,parent.version)).save_file() {
                                    if path.exists() {
                                        self.status="Nested function file already exists; choose a new version".into();
                                    } else {
                                        let saved=serde_json::to_string_pretty(&parent)
                                            .map_err(|err|err.to_string())
                                            .and_then(|json|std::fs::write(&path,json)
                                                .map_err(|err|err.to_string()));
                                        match saved {
                                            Err(err)=>self.status=format!("Nested function save failed: {err}"),
                                            Ok(())=>{
                                                let index=self.material_functions.insert(parent)
                                                    .expect("prevalidated and unique nested function");
                                                self.selected_material_function=Some(index);
                                                self.status=format!(
                                                    "Saved nested material function {}",path.display());
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                } else {
                    ui.small("To author a nested function, connect a Color placeholder node to the slot graph and apply it first.");
                }
            }
        });
    }

    fn apply_edited_material(&mut self) {
        let Some((for_asset,role))=self.editing_material else {return;};
        if let Err(e)=self.graph.validate() {
            self.status=format!("Invalid material graph: {e}");return;
        }
        if for_asset {
            self.asset.materials.set_graph(role,self.graph.clone());
            self.asset_mats_dirty=true;
        } else {
            self.foliage.materials.set_graph(role,self.graph.clone());
            self.foliage_mats_dirty=true;
        }
        self.status=format!("Applied {} to {}",role.name(),
            if for_asset {"asset"} else {"tree"});
    }

    fn asset_inspector(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().id_salt("asset_inspector").show(ui, |ui| {
        ui.heading("VOXEL ASSET");
        ui.label("Asset name");
        ui.add(egui::TextEdit::singleline(&mut self.asset.name));
        ui.small(format!("{} blocks",self.asset.voxels.len()));
        if let Some(tree)=&self.asset.tree_structure {
            ui.small(format!("Structured tree: {} branches · {} support hexes",
                tree.branches.len(),tree.root_footprint.len()));
            if let Some(cell)=self.asset_selected {
                if let Some(part)=tree.cells.iter().find(|record|record.cell==cell) {
                    ui.small(format!("Selected cell: {:?} · Branch {:?}",
                        part.part,part.branch_id));
                }
            }
        }
        ui.separator();
        ui.heading("TOOLS");
        ui.horizontal_wrapped(|ui| {
            ui.selectable_value(&mut self.asset_tool,AssetTool::Select,"Select");
            ui.selectable_value(&mut self.asset_tool,AssetTool::Add,"Add");
            ui.selectable_value(&mut self.asset_tool,AssetTool::Erase,"Erase");
            ui.selectable_value(&mut self.asset_tool,AssetTool::Paint,"Paint");
        });
        ui.small("Click a visible voxel face: select, add adjacent, erase or recolor.");
        ui.separator();
        ui.label("Brush material");
        for (material,label) in [
            (Material::Bark,"Bark"),(Material::LeafDark,"Dark leaf"),
            (Material::Leaf,"Leaf"),(Material::LeafLight,"Light leaf"),
        ] {
            ui.selectable_value(&mut self.asset_material,material,label);
        }
        ui.separator();
        ui.label("Foliage cell mesh");
        let mut render=self.asset.leaf_render;
        let mut render_changed=false;
        egui::ComboBox::from_id_salt("asset_leaf_style")
            .selected_text(match render.geometry {
                LeafGeometry::CrossedCutout=>"Crossed cutout cards",
                LeafGeometry::SolidVoxel=>"Solid leaf blocks",
            }).show_ui(ui,|ui|{
                render_changed |= ui.selectable_value(&mut render.geometry,
                    LeafGeometry::CrossedCutout,"Crossed foliage cards").changed();
                render_changed |= ui.selectable_value(&mut render.geometry,
                    LeafGeometry::SolidVoxel,"Solid voxel leaves").changed();
            });
        render_changed |= ui.add(egui::Slider::new(&mut render.wind_strength,
            0.0..=0.5).text("Wind sway")).changed();
        render_changed |= ui.add(egui::Slider::new(&mut render.wind_speed,
            0.0..=5.0).text("Wind speed")).changed();
        if render_changed {
            self.edit_asset(|asset|{asset.leaf_render=render;Ok(())});
        }
        ui.small("Wind animates the preview; OBJ exports its static base pose.");
        ui.horizontal(|ui| {
            ui.label("Bark");
            if ui.color_edit_button_srgb(&mut self.asset.bark_color).changed() {
                self.asset_mats_dirty=true;
            }
        });
        ui.horizontal(|ui| {
            ui.label("Leaf");
            if ui.color_edit_button_srgb(&mut self.asset.leaf_color).changed() {
                self.asset_mats_dirty=true;
            }
        });
        if seasonal_palette_editor(ui,&mut self.asset.materials,self.asset.leaf_color) {
            self.asset_gpu_scene=None;
        }
        ui.separator();
        ui.heading("SELECTED CELL");
        if let Some(old) = self.asset_selected {
            if self.asset.contains(old) {
                let mut next=old;
                ui.horizontal(|ui| {
                    for (i,axis) in ["X","Y","Z"].iter().enumerate() {
                        ui.label(*axis);
                        ui.add(egui::DragValue::new(&mut next[i]).range(-128..=128));
                    }
                });
                if next != old {
                    self.edit_asset(|asset|asset.move_voxel(old,next));
                    if self.asset.contains(next) {
                        self.asset_selected=Some(next);
                        self.asset_cursor=next;
                    }
                }
                ui.horizontal(|ui| {
                    if ui.button("Paint selection").clicked() {
                        let material=self.asset_material;
                        self.edit_asset(|asset|if asset.paint(old,material) {Ok(())}
                            else {Err("Selected voxel no longer exists".into())});
                    }
                    if ui.button("Delete selection").clicked() {
                        self.edit_asset(|asset|if asset.erase(old) {Ok(())}
                            else {Err("Selected voxel no longer exists".into())});
                        self.asset_selected=None;
                    }
                });
            } else {
                ui.small("Selection no longer exists; choose another voxel.");
            }
        } else {
            ui.small("No voxel selected.");
        }
        ui.separator();
        ui.heading("ADD BY COORDINATE");
        ui.horizontal(|ui| {
            for (i,axis) in ["X","Y","Z"].iter().enumerate() {
                ui.label(*axis);
                ui.add(egui::DragValue::new(&mut self.asset_cursor[i]).range(-128..=128));
            }
        });
        if ui.button("Add block at coordinates").clicked() {
            let pos=self.asset_cursor;
            let material=self.asset_material;
            self.edit_asset(|asset|asset.add(pos,material));
            if self.asset.contains(pos) {self.asset_selected=Some(pos);}
        }
        ui.separator();
        ui.heading("ASSET GRID");
        ui.label("Voxel size");
        let mut size=self.asset.voxel_size;
        if ui.add(egui::Slider::new(&mut size,0.1..=1.5)).changed() {
            self.edit_asset(|asset|{asset.voxel_size=size;Ok(())});
        }
        ui.label("Grid shape");
        let mut shape=self.asset.shape;
        egui::ComboBox::from_id_salt("asset_grid_shape")
            .selected_text(shape.name())
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut shape,VoxelShape::Cube,"Cube voxel grid");
                ui.selectable_value(&mut shape,VoxelShape::HexPrism,"Hexagonal-prism voxel grid");
            });
        if shape != self.asset.shape {
            self.edit_asset(|asset|{asset.shape=shape;Ok(())});
        }
        ui.small("Switching grid shape reinterprets the same integer cell coordinates.");
        ui.separator();
        ui.heading("PROCEDURAL MATERIAL");
        ui.small(if self.asset.material_graph.is_some() {
            "A procedural material graph is attached."
        } else { "No material graph attached." });
        if ui.button("Capture current material nodes").clicked() {
            self.asset.material_graph=Some(self.graph.clone());
            self.status="Saved current material graph into voxel asset".into();
        }
        if ui.add_enabled(self.asset.material_graph.is_some(),
            egui::Button::new("Edit attached material in Nodes")).clicked() {
            if let Some(graph)=&self.asset.material_graph {
                self.graph=graph.clone();
                self.library_tab=0;
                self.dirty=true;
                self.status="Material graph loaded; capture it again to update the asset".into();
            }
        }
        ui.small("Cutout foliage is rendered with original alpha-masked cards.");
        ui.small("Wind sway is preview-only; OBJ is static, JSON retains wind settings.");
        ui.small("Procedural material nodes remain available in the Nodes tab.");
        self.material_controls(ui,true);
        });
    }

    fn asset_canvas(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading("VOXEL MODEL EDITOR");
            ui.separator();
            ui.small(format!("{} • {} blocks",self.asset.shape.name(),self.asset.voxels.len()));
        });
        ui.horizontal(|ui| {
            ui.label("Orbit");
            ui.add(egui::Slider::new(&mut self.asset_yaw,
                0.0..=std::f32::consts::TAU).show_value(false));
            ui.label("Zoom");
            ui.add(egui::Slider::new(&mut self.asset_zoom,0.5..=3.0).show_value(false));
        });
        ui.add(egui::Slider::new(&mut self.season_preview,0.0..=4.0)
            .text("Season preview").fixed_decimals(2));
        ui.small("0 Spring • 1 Summer • 2 Autumn • 3 Winter • 4 Spring");
        leaf_aa_editor(ui,&mut self.leaf_aa_mode,
            self.gpu.as_ref().is_some_and(|gpu|gpu.msaa4_supported));
        ui.checkbox(&mut self.leaf_mipmaps,"Preview foliage mipmapping")
            .on_hover_text("Off by default. Enable trilinear alpha-weighted 64/128px foliage mip levels for distant tree-card filtering. Does not modify materials or exported textures.");
        if ui.checkbox(&mut self.gpu_bark_pbr,
            "Preview PBR bark (roughness + normal)").on_hover_text(
            "Opt-in dielectric GGX shading with graph-authored linear roughness and tangent-space normal maps. Uses a fixed preview light; leaf appearance, saved assets and geometry are unchanged."
            ).changed() {
            self.asset_gpu_scene=None;
            self.foliage_gpu_scene=None;
        }
        ui.small("PBR bark uses direct + ambient preview lighting; metallic and environment IBL are not implemented.");
        let available=ui.available_size();
        let desired=Vec2::new(available.x.max(50.0),available.y.max(240.0));
        let (rect,response)=ui.allocate_exact_size(desired,Sense::click_and_drag());
        let painter=ui.painter_at(rect);
        painter.rect_filled(rect,4.0,Color32::from_rgb(23,27,35));
        if let (Some(gpu),Some(scene))=(&self.gpu,&self.asset_gpu_scene) {
            gpu.paint(ui,rect,Arc::clone(scene),self.asset_yaw,self.asset_zoom,
                ui.ctx().input(|i|i.time as f32),self.season_preview,
                self.leaf_aa_mode,self.leaf_mipmaps);
        }
        if response.dragged_by(egui::PointerButton::Secondary) {
            self.asset_yaw=(self.asset_yaw+
                response.drag_delta().x*0.008).rem_euclid(std::f32::consts::TAU);
        }
        let Some(mesh)=&self.asset_mesh else {
            painter.text(rect.center(),Align2::CENTER_CENTER,"Asset geometry is invalid",
                FontId::proportional(16.0),Color32::LIGHT_GRAY);
            return;
        };
        let voxel_count=mesh.voxel_count();
        let triangle_count=mesh.triangle_count();
        if mesh.triangles.is_empty() {
            painter.text(rect.center(),Align2::CENTER_CENTER,"Empty asset: add a block using the inspector",
                FontId::proportional(16.0),Color32::LIGHT_GRAY);
            return;
        }
        let mut lo=[f32::INFINITY;3];
        let mut hi=[f32::NEG_INFINITY;3];
        for tri in &mesh.triangles {
            for p in tri.points {
                for axis in 0..3 {
                    lo[axis]=lo[axis].min(p[axis]);
                    hi[axis]=hi[axis].max(p[axis]);
                }
            }
        }
        let center: [f32; 3] = std::array::from_fn(|i|(hi[i]+lo[i])*0.5);
        let delta=[hi[0]-lo[0],hi[1]-lo[1],hi[2]-lo[2]];
        let span=delta[0].max(delta[1]).max(delta[2]).max(self.asset.voxel_size);
        let yaw=self.asset_yaw;
        let right=[yaw.cos(),yaw.sin(),0.0];
        let elevation=0.4_f32;
        let up=[-yaw.sin()*elevation.sin(),yaw.cos()*elevation.sin(),elevation.cos()];
        let depth=[-yaw.sin()*elevation.cos(),yaw.cos()*elevation.cos(),elevation.sin()];
        let dot=|a:[f32;3],b:[f32;3]| -> f32 {
            a[0]*b[0]+a[1]*b[1]+a[2]*b[2]
        };
        let scale=rect.width().min(rect.height())*0.70*self.asset_zoom/span;
        let project=|p:[f32;3]| {
            let d=[p[0]-center[0],p[1]-center[1],p[2]-center[2]];
            Pos2::new(rect.center().x+dot(d,right)*scale,
                rect.center().y-dot(d,up)*scale)
        };
        let needs_cpu_picking=self.gpu.is_none() ||
            response.clicked_by(egui::PointerButton::Primary);
        let mut faces:Vec<(f32,usize)>=if needs_cpu_picking {
            mesh.triangles.iter().enumerate().map(|(i,tri)|{
                let p=std::array::from_fn(|axis|
                    (tri.points[0][axis]+tri.points[1][axis]+tri.points[2][axis])/3.0);
                (dot(p,depth),i)
            }).collect()
        } else {Vec::new()};
        faces.sort_by(|a,b|a.0.total_cmp(&b.0));
        let pointer=if response.clicked_by(egui::PointerButton::Primary) {
            response.interact_pointer_pos()
        } else {None};
        let leaf_alpha=pointer.and(if self.asset_leaf_pixels.is_empty() {None}
            else {Some(&self.asset_leaf_pixels)});
        let wind_time=ui.ctx().input(|i|i.time as f32);
        if mesh.leaf_render.geometry==LeafGeometry::CrossedCutout &&
            mesh.leaf_render.wind_strength>0.0 && mesh.leaf_render.wind_speed>0.0 {
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(70));
        }
        let mut picked=None;
        let stride=(faces.len()/18000).max(1);
        let mut drawn=0usize;
        for (_,index) in faces.into_iter().step_by(stride) {
            drawn+=1;
            let tri=&mesh.triangles[index];
            let projected=std::array::from_fn(|i|project(sway_leaf_vertex(
                tri.points[i],tri.cell,tri.wind_weight[i],wind_time,
                mesh.leaf_render,mesh.voxel_size)));
            if let Some(p)=pointer {
                if hit_triangle(p,projected[0],projected[1],projected[2]) &&
                    (!tri.cutout || leaf_alpha.is_some_and(|rgba|
                        cutout_hit(p,projected,tri.uv,rgba))) {
                    picked=Some((tri.cell,tri.outward));
                }
            }
            let base=match tri.material {
                Material::Bark=>self.asset.bark_color,
                Material::LeafDark=>self.asset.leaf_color.map(|c|(c as f32*0.72) as u8),
                Material::Leaf=>self.asset.leaf_color,
                Material::LeafLight=>self.asset.leaf_color.map(|c|
                    (c as f32*1.22).min(255.0) as u8),
            };
            let shade=(0.8+0.2*(index as f32*0.013).cos().abs()).min(1.0);
            let color=Color32::from_rgb(
                (base[0] as f32*shade) as u8,
                (base[1] as f32*shade) as u8,
                (base[2] as f32*shade) as u8);
            let selected=self.asset_selected==Some(tri.cell);
            if self.gpu.is_none() {
                paint_voxel_triangle(&painter,tri,projected,
                    if tri.cutout || tri.material==Material::Bark {Color32::WHITE} else {color},
                    if tri.cutout {self.asset_leaf_texture.as_ref()}
                        else if tri.material==Material::Bark {self.asset_bark_texture.as_ref()}
                        else {None},selected);
            } else if selected {
                for n in 0..3 {
                    painter.line_segment([projected[n],projected[(n+1)%3]],
                        Stroke::new(1.0,Color32::from_rgb(255,204,95)));
                }
            }
        }
        if self.gpu.is_some() {
            // The GPU owns all visible mesh triangles. Only highlight the handful
            // of faces belonging to the active selected cell on the CPU.
            if let Some(cell)=self.asset_selected {
                for tri in mesh.triangles.iter().filter(|t|t.cell==cell).take(36) {
                    let corners=tri.points.map(project);
                    for i in 0..3 {
                        painter.line_segment([corners[i],corners[(i+1)%3]],
                            Stroke::new(1.0,Color32::from_rgb(255,204,95)));
                    }
                }
            }
        }
        if let Some((cell,face))=picked {
            match self.asset_tool {
                AssetTool::Select => {
                    self.asset_selected=Some(cell);
                    self.asset_cursor=cell;
                }
                AssetTool::Add => {
                    let pos=[cell[0]+face[0],cell[1]+face[1],cell[2]+face[2]];
                    let mat=self.asset_material;
                    self.edit_asset(|asset|asset.add(pos,mat));
                    if self.asset.contains(pos) {self.asset_selected=Some(pos);}
                }
                AssetTool::Erase => {
                    self.edit_asset(|asset|if asset.erase(cell) {Ok(())}
                        else {Err("Cell is already empty".into())});
                    self.asset_selected=None;
                }
                AssetTool::Paint => {
                    let mat=self.asset_material;
                    self.edit_asset(|asset|if asset.paint(cell,mat) {Ok(())}
                        else {Err("Cell is already empty".into())});
                    self.asset_selected=Some(cell);
                }
            }
        }
        painter.text(rect.left_top()+Vec2::new(10.0,10.0),Align2::LEFT_TOP,
            format!("{} voxels • {} triangles • wgpu",
                voxel_count,if self.gpu.is_some(){triangle_count}else{drawn}),
            FontId::proportional(13.0),Color32::LIGHT_GRAY);
        ui.small("Left-click a voxel face to edit. Right-drag to orbit. Select a block in the outliner.");
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

    fn save_flora_prefab(&mut self) {
        let Some(path)=rfd::FileDialog::new().add_filter("Flora prefab",&["json"])
            .set_file_name("plant.flora.json").save_file() else {return;};
        let result=FloraPrefab::from_project(
                FloraPrefab::builtin_id(self.foliage.preset),self.foliage.clone())
            .and_then(|prefab|prefab.to_pretty_json())
            .and_then(|json|std::fs::write(&path,json).map_err(|e|e.to_string()));
        self.status=match result {
            Ok(())=>format!("Saved Flora prefab: {}",path.display()),
            Err(err)=>format!("Save Flora prefab failed: {err}"),
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
                self.foliage_mats_dirty = true;
                self.status = format!("Opened foliage: {}",path.display());
            }
            Err(e) => self.status = format!("Open foliage failed: {e}"),
        }
    }

    fn export_foliage(&mut self) {
        let Some(folder) = rfd::FileDialog::new().pick_folder() else { return; };
        let result: Result<(),String> = (||{
            let names=["foliage.voxels.json","foliage.obj","foliage.mtl","foliage.json",
                "foliage-leaves.png","foliage-leaves-alpha.png",
                "bark-albedo.png","bark-roughness.png","bark-normal.png",
                "foliage-leaves-roughness.png","foliage-leaves-normal.png"];
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
                ("foliage.obj",mesh.obj_with_bark_meters_per_repeat(
                    self.foliage.materials.bark_meters_per_repeat)?),
                ("foliage.mtl",texture_studio::foliage::mtl(&self.foliage)),
                ("foliage.json",project),
            ];
            for (name,contents) in files {
                std::fs::write(folder.join(name),contents).map_err(|e|e.to_string())?;
            }
            write_foliage_textures(&folder,&self.foliage.materials,
                self.foliage.bark_color,self.foliage.leaf_color,
                self.foliage.leaf_render.geometry==LeafGeometry::CrossedCutout)?;
            Ok(())
        })();
        self.status = match result {
            Ok(()) => format!("Exported voxel blocks, culled OBJ surface, materials and settings to {}",folder.display()),
            Err(err) => format!("Foliage export failed: {err}"),
        };
    }

    fn foliage_inspector(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().id_salt("foliage_inspector").show(ui,|ui|{
        ui.heading("TREE DESIGNER");
        ui.horizontal(|ui| {
            ui.label("Plant");
            egui::ComboBox::from_id_salt("foliage_species")
                .selected_text(self.foliage.preset.name())
                .show_ui(ui, |ui| {
                    let mut group = "";
                    for species in PlantPreset::ALL {
                        if group != species.group() {
                            if !group.is_empty() { ui.separator(); }
                            group = species.group();
                            ui.strong(group);
                        }
                        if ui.selectable_label(self.foliage.preset == species,species.name()).clicked() {
                            self.foliage = FoliageProject::from_preset(species);
                            if species==PlantPreset::CherryBlossom || species.is_coffee() {
                                // Show the signature pink flowers immediately.
                                self.season_preview=0.0;
                            }
                            self.foliage_dirty = true;
                            self.foliage_mats_dirty = true;
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
                ui.horizontal(|ui| {
                    changed |= ui.add(egui::DragValue::new(&mut tree.seed)
                        .prefix("Seed: ")).changed();
                    if ui.button("Next seed").clicked() {
                        tree.seed=tree.seed.wrapping_add(1);
                        changed=true;
                    }
                });
                changed |= ui.add(egui::Slider::new(&mut tree.height,0.5..=30.0).text("Height")).changed();
                changed |= ui.add(egui::Slider::new(&mut tree.branch_count,1..=90)
                    .text(if tree.preset.is_palm() {"Frond count"} else {"Branch count"})).changed();
                changed |= ui.add(egui::Slider::new(&mut tree.branch_length,0.1..=1.5)
                    .text(if tree.preset.is_palm() {"Frond reach"} else {"Branch length"})).changed();
                if !tree.preset.is_palm() {
                ui.label("Branch growth");
                egui::ComboBox::from_id_salt("foliage_branch_growth")
                    .selected_text(match tree.branch_growth {
                        BranchGrowth::Classic=>"Classic",
                        BranchGrowth::CanopyGuided=>"Canopy guided",
                    })
                    .show_ui(ui, |ui| {
                        changed |= ui.selectable_value(&mut tree.branch_growth,
                            BranchGrowth::Classic,"Classic").changed();
                        changed |= ui.selectable_value(&mut tree.branch_growth,
                            BranchGrowth::CanopyGuided,"Canopy guided").changed();
                    });
                }
                if !tree.preset.is_palm() && !matches!(tree.preset,
                    PlantPreset::GreenBamboo|PlantPreset::BrownBamboo) {
                    ui.collapsing("Structural wood thickness (pipe model)",|ui|{
                        changed |= ui.add(egui::Slider::new(
                            &mut tree.wood_pipe_blend,0.0..=1.0)
                            .text("Foliage-load influence")).changed();
                        changed |= ui.add(egui::Slider::new(
                            &mut tree.wood_pipe_exponent,1.5..=3.0)
                            .text("Branch area exponent")).changed();
                        ui.small("Calculates support from occupied leaf hexels, not GPU cards.");
                        ui.small("Radii and new wood occupancy remain bounded; original species habit preserved.");
                    });
                }
                let rules=tree.preset.growth_rules();
                ui.small(format!("Species architecture: {:?} branches, {:?} foliage",
                    rules.phyllotaxy,rules.foliage));
                ui.small(if tree.preset.is_palm() {
                    "Seed changes the trunk lean, crown petioles and frond foliage."
                } else {
                    "Seed changes trunk lean, branches, twigs and leaf locations within species rules."
                });
                if tree.preset==PlantPreset::CherryBlossom {
                    ui.small("Ornamental sakura: short pink spring bloom, green summer leaves.");
                    ui.small("Select Spring (0) in the season preview to see flowers.");
                }
                if tree.preset.is_coffee() {
                    ui.small("Evergreen Coffea: opposite glossy leaves, ripe coffee cherries and seasonal white flowers.");
                    ui.small("Cherries are non-harvestable texture accents; cultivars differ mainly in size and crown habit.");
                }
                if tree.preset==PlantPreset::KentuckyCoffeetree {
                    ui.small("Kentucky Coffeetree is NOT a coffee crop. Its pods and raw seeds are toxic.");
                    ui.small("Coarse branches and doubly compound deciduous leaves; seed pods not modeled.");
                }
                if tree.preset.is_cottonwood() {
                    ui.small("Riparian Populus: fast-growing crown, yellow autumn leaves, stronger default leaf sway.");
                    ui.small("Female cottony seed release and falling fluff are not yet simulated.");
                }
                if tree.preset.is_palm() {
                    ui.small("Pinnate palms have feather fronds; fan palms have radial blades.");
                }
                ui.collapsing("Climate / habitat (biome placement reference)",|ui| {
                    let c=tree.preset.climate_profile();
                    ui.label(format!("Preferred annual mean: {:.0} to {:.0} °C",
                        c.mean_annual_temp_c[0],c.mean_annual_temp_c[1]));
                    ui.label(format!("Preferred annual rainfall: {} to {} mm",
                        c.annual_precip_mm[0],c.annual_precip_mm[1]));
                    ui.label(format!("Indicative cold-event floor: {:.0} °C",
                        c.extreme_min_c));
                    ui.label(format!("Preferred dry months (<30 mm rain): {} to {}",
                        c.dry_months_per_year[0],c.dry_months_per_year[1]));
                    ui.label(format!("Root-zone moisture: {:?} • light: {:?}",
                        c.soil_moisture,c.light));
                    ui.label(format!("Groundwater/stream access needed: {}",
                        if c.requires_groundwater {"Yes"} else {"No"}));
                    ui.small("Approximate world-placement preferences, not survival limits.");
                    ui.small("World biome placement is not yet wired to this editor.");
                });
                changed |= species_growth_editor(ui,tree);
                if !tree.preset.is_palm() {
                    ui.small("Canopy guided steers branches toward seeded crown targets.");
                }
                ui.small("Voxelize the selected plant at the grid resolution.");
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
                ui.label("Game hexel geometry: 1 m across flats, 1 m high.");
                ui.small("Six horizontal faces; solid wood is generated as editable world cells.");
                changed |= ui.add(egui::Slider::new(&mut tree.trunk_radius,
                    0.02..=2.0).text("Trunk width (m)")).changed();
            }
            FoliageStage::FoliagePalette => {
                ui.label("Harvestable foliage hexels with crossed cutout leaf cards.");
                let mut wind_changed=false;
                wind_changed |= ui.add(egui::Slider::new(&mut tree.leaf_render.wind_strength,
                    0.0..=0.5).text("Wind sway")).changed();
                wind_changed |= ui.add(egui::Slider::new(&mut tree.leaf_render.wind_speed,
                    0.0..=5.0).text("Wind speed")).changed();
                if wind_changed {
                    // Wind affects the scene uniforms, not physical leaf cells.
                    if let Some(mesh)=&mut self.foliage_mesh {
                        mesh.leaf_render=tree.leaf_render;
                    }
                    self.foliage_gpu_scene=None;
                }
                ui.small("Leaf cell positions remain voxel-anchored; only their visual mesh bends.");
                ui.label("Bark color");
                let bark_changed=ui.color_edit_button_srgb(&mut tree.bark_color).changed();
                ui.label("Leaf color");
                let leaf_changed=ui.color_edit_button_srgb(&mut tree.leaf_color).changed();
                if bark_changed || leaf_changed {self.foliage_mats_dirty=true;}
                if seasonal_palette_editor(ui,&mut tree.materials,tree.leaf_color) {
                    self.foliage_gpu_scene=None;
                }
                ui.small("Seasonal gradients affect color, not occupancy or falling leaves.");
                if tree.preset==PlantPreset::CherryBlossom {
                    ui.small("Spring blossoms are a temporary GPU petal overlay; leaf palettes remain independent.");
                }
                if tree.preset.is_coffee() {
                    ui.small("Coffee's white spring-preview flowers are cosmetic WGSL overlays; ripeness is static.");
                }
                ui.small("Material slots: bark + dark, medium and light leaves.");
            }
            FoliageStage::FoliageDistributor => {
                changed |= ui.add(egui::Slider::new(&mut tree.leaf_density,0.0..=1.0).text("Density")).changed();
                changed |= ui.add(egui::Slider::new(&mut tree.leaf_scale,0.02..=1.5).text("Leaf size")).changed();
                changed |= ui.add(egui::Slider::new(&mut tree.leaf_angle,-1.0..=1.0).text("Leaf orientation")).changed();
                changed |= ui.add(egui::Slider::new(&mut tree.leaf_variation,0.0..=1.0).text("Size variation")).changed();
                ui.small("Distributes foliage cells; each cell may display crossed, cutout leaf cards.");
            }
            FoliageStage::Output => {
                if let Some(mesh)=&self.foliage_mesh {
                    ui.label(format!("{} occupied voxels",mesh.voxel_count()));
                    ui.label(format!("{} exposed-surface triangles",mesh.triangle_count()));
                    ui.label(format!("Voxel type: {}",mesh.shape.name()));
                    ui.label(format!("Cell size: {:.2}",mesh.voxel_size));
                    ui.label(format!("Nominal height: {:.1}",mesh.height));
                    if let Some(structure)=&mesh.structure {
                        let majors=structure.branches.iter()
                            .filter(|b|b.part==texture_studio::foliage::TreePart::MajorBranch).count();
                        let forks=structure.branches.iter()
                            .filter(|b|b.part==texture_studio::foliage::TreePart::SmallBranch).count();
                        if tree.preset.is_palm() {
                            ui.label(format!("Trunk + {majors} crown fronds (no woody forks)"));
                        } else {
                            ui.label(format!("Trunk + {majors} main branches + {forks} forks"));
                        }
                        ui.label(format!("Root footprint: {} support hexes",
                            structure.root_footprint.len()));
                        ui.small("Every leaf cell belongs to a branch; unsupported leaf islands are removed.");
                    }
                }
                ui.small("Exports occupied foliage cells plus cutout textures, UV OBJ and material masks.");
                ui.small("Also exports the culled hexel surface as OBJ with MTL.");
                ui.separator();
                ui.small("World tree: 1 m between opposing hex faces, 1 m high.");
                ui.small("Wood is solid; foliage cells render crossed cutout cards.");
                if ui.button("Edit tree as hexel asset").clicked() {
                    self.import_tree_as_asset();
                }
                ui.small("Opens in Assets for per-cell removal/painting and saving.");
                if ui.button("Export voxel tree").clicked() { self.export_foliage(); }
                if ui.button("Save foliage project").clicked() { self.save_foliage(); }
                if ui.button("Save Flora prefab JSON").clicked() {
                    self.save_flora_prefab();
                }
                ui.small("Flora prefab bundles editable tree settings and habitat.");
                ui.small("Give edited variants distinct asset_id values before publishing.");
                if ui.button("Open foliage project").clicked() { self.open_foliage(); }
            }
        }
        if changed { self.foliage_dirty = true; }
        if let Err(err)=self.foliage.validate() {
            ui.colored_label(Color32::from_rgb(232,133,104),err);
        }
        ui.separator();
        ui.small("Tree geometry and material graphs remain independently editable.");
        self.material_controls(ui,false);
        });
    }

    fn foliage_canvas(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading("VOXEL TREE PREVIEW");
            ui.separator();
            ui.small("Standard 1 m game hexels • adjustable growth stages");
        });
        ui.add(egui::Slider::new(&mut self.foliage_yaw,
            0.0..=std::f32::consts::TAU).text("Orbit"));
        ui.add(egui::Slider::new(&mut self.season_preview,0.0..=4.0)
            .text("Season preview").fixed_decimals(2));
        ui.small("0 Spring • 1 Summer • 2 Autumn • 3 Winter • 4 Spring");
        leaf_aa_editor(ui,&mut self.leaf_aa_mode,
            self.gpu.as_ref().is_some_and(|gpu|gpu.msaa4_supported));
        ui.checkbox(&mut self.leaf_mipmaps,"Preview foliage mipmapping")
            .on_hover_text("Off by default. Enable trilinear alpha-weighted 64/128px foliage mip levels for distant tree-card filtering. Does not modify materials or exported textures.");
        if ui.checkbox(&mut self.gpu_bark_pbr,
            "Preview PBR bark (roughness + normal)").on_hover_text(
            "Opt-in dielectric GGX shading with graph-authored linear roughness and tangent-space normal maps. Uses a fixed preview light; leaf appearance, saved assets and geometry are unchanged."
            ).changed() {
            self.asset_gpu_scene=None;
            self.foliage_gpu_scene=None;
        }
        ui.small("PBR bark uses direct + ambient preview lighting; metallic and environment IBL are not implemented.");
        let available=ui.available_size();
        let desired=Vec2::new(available.x.max(50.0),available.y.max(250.0));
        let (rect,_) = ui.allocate_exact_size(desired,Sense::hover());
        let painter=ui.painter_at(rect);
        painter.rect_filled(rect,4.0,Color32::from_rgb(22,27,35));
        if let (Some(gpu),Some(scene))=(&self.gpu,&self.foliage_gpu_scene) {
            gpu.paint(ui,rect,Arc::clone(scene),self.foliage_yaw,1.0,
                ui.ctx().input(|i|i.time as f32),self.season_preview,
                self.leaf_aa_mode,self.leaf_mipmaps);
        }
        let Some(mesh)=&self.foliage_mesh else {
            painter.text(rect.center(),Align2::CENTER_CENTER,
                "Invalid foliage settings — adjust parameters",
                FontId::proportional(15.0),Color32::LIGHT_GRAY);
            return;
        };
        if self.gpu.is_some() {
            if mesh.leaf_render.wind_strength>0.0 && mesh.leaf_render.wind_speed>0.0 {
                ui.ctx().request_repaint_after(std::time::Duration::from_millis(70));
            }
            painter.text(rect.left_top()+Vec2::new(12.0,12.0),Align2::LEFT_TOP,
                format!("wgpu • {} • {} voxels • {} GPU triangles",
                    self.foliage.preset.name(),mesh.voxel_count(),mesh.triangle_count()),
                FontId::proportional(14.0),Color32::LIGHT_GRAY);
            return;
        }
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
        let wind_time=ui.ctx().input(|i|i.time as f32);
        if mesh.leaf_render.geometry==LeafGeometry::CrossedCutout &&
            mesh.leaf_render.wind_strength>0.0 && mesh.leaf_render.wind_speed>0.0 {
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(70));
        }
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
            let points=std::array::from_fn(|i|project(sway_leaf_vertex(
                tri.points[i],tri.cell,tri.wind_weight[i],wind_time,
                mesh.leaf_render,mesh.voxel_size)));
            if self.gpu.is_none() {
                paint_voxel_triangle(&painter,tri,points,
                    if tri.cutout || tri.material==Material::Bark {Color32::WHITE} else {color},
                    if tri.cutout {self.leaf_texture.as_ref()}
                        else if tri.material==Material::Bark {self.bark_texture.as_ref()}
                        else {None},false);
            }
        }
        painter.text(rect.left_top()+Vec2::new(12.0,12.0),Align2::LEFT_TOP,
            format!("{} • {} {} voxels • {} surface triangles",
                self.foliage.preset.name(),mesh.voxel_count(),mesh.shape.name(),
                mesh.triangle_count()),
            FontId::proportional(14.0),Color32::LIGHT_GRAY);
    }

    fn palette(&mut self, ui: &mut egui::Ui) {
        ui.heading("LIBRARY");
        ui.horizontal_wrapped(|ui| {
            ui.selectable_value(&mut self.library_tab, 3, "Assets");
            ui.selectable_value(&mut self.library_tab, 2, "Foliage");
            ui.selectable_value(&mut self.library_tab, 1, "Presets");
            ui.selectable_value(&mut self.library_tab, 0, "Nodes");
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
                ("Scalar", NodeKind::Scalar { value: 0.5 }),
            ]),
            ("FILTERS", vec![
                ("Invert", NodeKind::Invert),
                ("Luminance", NodeKind::Luminance),
                ("Colorize", NodeKind::Colorize { dark: [24, 37, 54], light: [204, 175, 130] }),
                ("Color Ramp", NodeKind::ColorRamp { ramp: ColorRamp::default() }),
                ("Curve Remap", NodeKind::CurveRemap { curve: CurveRemap::default() }),
                ("Levels", NodeKind::Levels { black: 0.0, white: 1.0, gamma: 1.0 }),
                ("Threshold Mask", NodeKind::ThresholdMask {
                    threshold:0.5,softness:0.0
                }),
            ]),
            ("SIGNED DISTANCE", vec![
                ("SDF Circle", NodeKind::SdfCircle{
                    center:[0.5,0.5],radius:0.22}),
                ("SDF Ellipse", NodeKind::SdfEllipse{
                    center:[0.5,0.5],radii:[0.28,0.14]}),
                ("SDF Capsule", NodeKind::SdfCapsule{
                    start:[0.3,0.5],end:[0.7,0.5],radius:0.07}),
                ("SDF Bézier Stroke", NodeKind::SdfBezierStroke{
                    points:[[0.2,0.5],[0.35,0.2],[0.65,0.8],[0.8,0.5]],
                    start_radius:0.07,end_radius:0.02,segments:24,tiled:true}),
                ("SDF Boolean", NodeKind::SdfCombine{
                    operation:SdfBoolean::Union,smoothing:0.0}),
                ("SDF Contour Warp", NodeKind::SdfContourWarp{
                    frequency:8,amount:0.05,seed:57}),
                ("Distance to Mask", NodeKind::SdfToMask{edge_width:0.01}),
            ]),
            ("PATTERN SCATTER", vec![
                ("Shape Scatter", NodeKind::ShapeScatter{
                    cells:10,scale:0.8,jitter:0.2,rotation:1.0,
                    brightness:0.3,seed:55}),
            ]),
            ("GEOMETRY CONTEXT",vec![
                ("Face Surface Mask",NodeKind::FaceSurfaceMask{surface:FaceSurface::Top}),
                ("Face Facing Mask",NodeKind::FaceFacingMask{axis:FaceAxis::PosZ,sharpness:1.0}),
            ]),
            ("BITMAP INPUTS", vec![
                ("Bitmap Color",NodeKind::BitmapColor{asset_id:0,bilinear:true}),
                ("Bitmap Alpha",NodeKind::BitmapAlpha{asset_id:0,bilinear:true}),
            ]),
            ("REGION VARIATION", vec![
                ("Region Value", NodeKind::RegionValue{cells:7,seed:17}),
                ("Region Border Mask", NodeKind::RegionBorderMask{cells:7,seed:17,width:0.2}),
            ]),
            ("HEIGHT LAYERS", vec![
                ("Height Blend Mask", NodeKind::HeightBlendMask{influence:0.4}),
                ("Mask Mix", NodeKind::MaskMix),
            ]),
            ("MIX & ROUTING", vec![
                ("Blend", NodeKind::Blend { factor: 0.5 }),
                ("Color Reroute", NodeKind::Reroute {kind:SocketType::Color}),
                ("Scalar Reroute", NodeKind::Reroute {kind:SocketType::Scalar}),
                ("Mask Reroute", NodeKind::Reroute {kind:SocketType::Mask}),
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
        let bitmap_choices:Vec<(u64,String)>=self.graph.images.iter()
            .map(|image|(image.id,format!("{} ({}x{})",image.name,
                image.width,image.height))).collect();
        let mut import_bitmap=false;
        let mut prune_bitmaps=false;
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
            NodeKind::Scalar { value } => {
                changed |= ui.add(egui::Slider::new(value, 0.0..=1.0)
                    .text("Value")).changed();
                ui.small("Scalar values broadcast to grayscale on RGB inputs.");
            }
            NodeKind::Levels { black,white,gamma } => {
                changed |= ui.add(egui::Slider::new(black,0.0..=(*white-0.01).max(0.0))
                    .text("Black point")).changed();
                changed |= ui.add(egui::Slider::new(white,(*black+0.01).min(1.0)..=1.0)
                    .text("White point")).changed();
                changed |= ui.add(egui::Slider::new(gamma,0.1..=5.0)
                    .text("Gamma")).changed();
                ui.small("Typed scalar input; RGB to scalar requires an explicit conversion.");
            }
            NodeKind::ThresholdMask { threshold,softness } => {
                changed |= ui.add(egui::Slider::new(threshold,0.0..=1.0)
                    .text("Threshold")).changed();
                changed |= ui.add(egui::Slider::new(softness,0.0..=1.0)
                    .text("Edge softness")).changed();
                ui.small("Outputs a grayscale Mask; opacity is still authored in the Leaf Cutout slot.");
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
            NodeKind::ColorRamp { ramp } => {
                ui.small("Input: Scalar 0–1. Output: RGB color.");
                let (preview,_) = ui.allocate_exact_size(
                    Vec2::new(ui.available_width().min(240.0),18.0),
                    Sense::hover());
                for x in 0..64 {
                    let t=x as f32/63.0;
                    let rgb=ramp.sample(t).map(|component|
                        (component*255.0).round() as u8);
                    let start=preview.left()+preview.width()*x as f32/64.0;
                    let end=preview.left()+preview.width()*(x+1) as f32/64.0;
                    ui.painter().rect_filled(
                        Rect::from_min_max(
                            Pos2::new(start,preview.top()),
                            Pos2::new(end,preview.bottom())),
                        0.0,Color32::from_rgb(rgb[0],rgb[1],rgb[2]));
                }
                let count=ramp.count as usize;
                let mut remove=None;
                for index in 0..count {
                    let lower=if index>0 {
                        ramp.stops[index-1].position+0.001
                    } else {0.0};
                    let upper=if index+1<count {
                        ramp.stops[index+1].position-0.001
                    } else {1.0};
                    ui.horizontal(|ui| {
                        ui.label(format!("{}",index+1));
                        if index>0 && index+1<count {
                            changed |= ui.add(egui::Slider::new(
                                &mut ramp.stops[index].position,lower..=upper)
                                .fixed_decimals(3).text("Position")).changed();
                        } else {
                            ui.small(format!("{:.0}%",ramp.stops[index].position*100.0));
                        }
                        changed |= ui.color_edit_button_srgb(
                            &mut ramp.stops[index].rgb).changed();
                        if index>0 && index+1<count
                            && ui.small_button("Remove").clicked() {
                            remove=Some(index);
                        }
                    });
                }
                if let Some(index)=remove {changed |= ramp.remove_stop(index);}
                if ui.add_enabled(count<MAX_COLOR_STOPS,
                    egui::Button::new("Add color stop")).clicked() {
                    changed |= ramp.add_stop();
                }
                ui.small("Up to 8 ordered stops; endpoints stay at 0% and 100%.");
                ui.small("Static baked appearance — seasonal GPU palettes are edited separately.");
            }
            NodeKind::CurveRemap { curve } => {
                ui.small("Input/Output: Scalar 0–1. Smooth cubic remap.");
                let (preview,_) = ui.allocate_exact_size(
                    Vec2::new(ui.available_width().min(240.0),90.0),
                    Sense::hover());
                ui.painter().rect_stroke(preview,0.0,
                    Stroke::new(1.0,Color32::DARK_GRAY),StrokeKind::Inside);
                let mut previous=None;
                for x in 0..=64 {
                    let t=x as f32/64.0;
                    let sample=curve.sample(t);
                    let point=Pos2::new(preview.left()+preview.width()*t,
                        preview.bottom()-preview.height()*sample);
                    if let Some(prev)=previous {
                        ui.painter().line_segment([prev,point],
                            Stroke::new(2.0,Color32::from_rgb(116,212,190)));
                    }
                    previous=Some(point);
                }
                let count=curve.count as usize;
                let mut remove=None;
                for index in 0..count {
                    let lower=if index>0 {
                        curve.points[index-1].position+0.001
                    } else {0.0};
                    let upper=if index+1<count {
                        curve.points[index+1].position-0.001
                    } else {1.0};
                    ui.horizontal(|ui| {
                        ui.label(format!("{}",index+1));
                        if index>0 && index+1<count {
                            changed |= ui.add(egui::Slider::new(
                                &mut curve.points[index].position,lower..=upper)
                                .fixed_decimals(3).text("X")).changed();
                        } else {
                            ui.small(format!("{:.0}%",curve.points[index].position*100.0));
                        }
                        changed |= ui.add(egui::Slider::new(
                            &mut curve.points[index].value,0.0..=1.0)
                            .fixed_decimals(3).text("Y")).changed();
                        if index>0 && index+1<count
                            && ui.small_button("Remove").clicked() {
                            remove=Some(index);
                        }
                    });
                }
                if let Some(index)=remove {changed |= curve.remove_point(index);}
                if ui.add_enabled(count<MAX_CURVE_POINTS,
                    egui::Button::new("Add control point")).clicked() {
                    changed |= curve.add_point();
                }
                ui.small("2–8 points. The 0% and 100% input positions remain fixed.");
            }
            NodeKind::SdfCircle{center,radius} => {
                ui.small("Tileable signed distance: negative inside. Convert to Mask before PBR output.");
                for i in 0..2 {
                    changed |= ui.add(egui::Slider::new(&mut center[i],0.0..=1.0)
                        .text(if i==0 {"Center X"} else {"Center Y"})).changed();
                }
                changed |= ui.add(egui::Slider::new(radius,0.001..=0.5)
                    .text("Radius")).changed();
            }
            NodeKind::SdfEllipse{center,radii} => {
                ui.small("Periodic ellipse: exact contour, approximate distance away from the edge.");
                for i in 0..2 {
                    changed |= ui.add(egui::Slider::new(&mut center[i],0.0..=1.0)
                        .text(if i==0 {"Center X"} else {"Center Y"})).changed();
                    changed |= ui.add(egui::Slider::new(&mut radii[i],0.001..=0.5)
                        .text(if i==0 {"Radius X"} else {"Radius Y"})).changed();
                }
            }
            NodeKind::SdfCapsule{start,end,radius} => {
                ui.small("Tileable rounded stroke on the shortest wrapped endpoint path.");
                for i in 0..2 {
                    changed |= ui.add(egui::Slider::new(&mut start[i],0.0..=1.0)
                        .text(if i==0 {"Start X"} else {"Start Y"})).changed();
                    changed |= ui.add(egui::Slider::new(&mut end[i],0.0..=1.0)
                        .text(if i==0 {"End X"} else {"End Y"})).changed();
                }
                changed |= ui.add(egui::Slider::new(radius,0.001..=0.5)
                    .text("Radius")).changed();
            }
            NodeKind::SdfBezierStroke{points,start_radius,end_radius,
                segments,tiled} => {
                ui.small("Cubic Bézier stroke: four control points in normalized 0–1 UV.");
                for (index,point) in points.iter_mut().enumerate() {
                    ui.label(match index {
                        0=>"Start anchor",1=>"Handle A",2=>"Handle B",_=>"End anchor",
                    });
                    ui.horizontal(|ui| {
                        changed |= ui.add(egui::Slider::new(&mut point[0],0.0..=1.0)
                            .text("U")).changed();
                        changed |= ui.add(egui::Slider::new(&mut point[1],0.0..=1.0)
                            .text("V")).changed();
                    });
                }
                changed |= ui.add(egui::Slider::new(start_radius,0.001..=0.25)
                    .text("Start radius")).changed();
                changed |= ui.add(egui::Slider::new(end_radius,0.001..=0.25)
                    .text("End radius")).changed();
                changed |= ui.add(egui::Slider::new(segments,8..=64)
                    .text("Curve segments")).changed();
                changed |= ui.checkbox(tiled,"Repeat across U/V seams").changed();
                ui.small("Approximate signed distance: finer segment counts cost more CPU per pixel.");
                ui.small("Distance output must pass through Distance to Mask before PBR outputs.");
            }
            NodeKind::SdfCombine{operation,smoothing} => {
                ui.small("Combine two signed Distance inputs before converting to coverage.");
                egui::ComboBox::from_id_salt("sdf_operation")
                    .selected_text(format!("{operation:?}"))
                    .show_ui(ui,|ui|{
                        for option in [SdfBoolean::Union,SdfBoolean::Intersection,
                            SdfBoolean::Subtract] {
                            changed |= ui.selectable_value(operation,option,
                                format!("{option:?}")).changed();
                        }
                    });
                changed |= ui.add(egui::Slider::new(smoothing,0.0..=0.25)
                    .text("Smooth width")).changed();
            }
            NodeKind::SdfContourWarp{frequency,amount,seed} => {
                ui.small("Periodic noise warps a signed Distance field without clipping it to a mask.");
                changed |= ui.add(egui::Slider::new(frequency,1..=64)
                    .text("Frequency")).changed();
                changed |= ui.add(egui::Slider::new(amount,0.0..=0.25)
                    .text("UV offset")).changed();
                changed |= ui.add(egui::DragValue::new(seed)
                    .speed(1).prefix("Seed: ")).changed();
                ui.small("0 offset is exact pass-through; positive values distort the original contour.");
                ui.small("Feed an SDF shape into this node; convert to Mask after warping.");
            }
            NodeKind::ShapeScatter{cells,scale,jitter,rotation,brightness,seed} => {
                ui.small("Scatter a connected Pattern Mask across tileable UV; optional Density Mask defaults to full.");
                changed |= ui.add(egui::Slider::new(cells,2..=32)
                    .text("Cells per axis")).changed();
                changed |= ui.add(egui::Slider::new(scale,0.1..=0.85)
                    .text("Stamp scale / cell")).changed();
                changed |= ui.add(egui::Slider::new(jitter,0.0..=0.25)
                    .text("Position jitter / cell")).changed();
                changed |= ui.add(egui::Slider::new(rotation,0.0..=1.0)
                    .text("Rotation variation")).changed();
                changed |= ui.add(egui::Slider::new(brightness,0.0..=1.0)
                    .text("Brightness variation")).changed();
                changed |= ui.add(egui::DragValue::new(seed)
                    .speed(1).prefix("Seed: ")).changed();
                ui.small("At most 9 neighboring grid cells sampled per pixel. Output: Mask.");
            }
            NodeKind::SdfToMask{edge_width} => {
                ui.small("Explicit signed Distance to Mask conversion. Negative distance is inside.");
                changed |= ui.add(egui::Slider::new(edge_width,0.0..=0.25)
                    .text("Coverage width")).changed();
                ui.small("0: hard threshold; positive width softens edges in normalized UV units.");
            }
            NodeKind::Reroute { kind } => {
                ui.label(format!("{kind:?} pass-through"));
                ui.small("Organize cables without changing values, texture sampling or material ownership.");
                ui.small("Input and output types are fixed. Pick a different reroute variant to change types.");
            }
            NodeKind::Invert => { ui.label("Inverts the RGB channels of its input."); }
            NodeKind::Luminance => {
                ui.label("Convert RGB to a scalar using luminance weights.");
                ui.small("Use between legacy RGB generators and typed Scalar inputs.");
            }
            NodeKind::FaceFacingMask{axis,sharpness} => {
                ui.small("Object-local surface direction (not world compass or terrain slope).");
                egui::ComboBox::from_id_salt(("face_facing",id))
                    .selected_text(match axis {
                        FaceAxis::PosX=>"+X",FaceAxis::NegX=>"-X",
                        FaceAxis::PosY=>"+Y",FaceAxis::NegY=>"-Y",
                        FaceAxis::PosZ=>"+Z (up)",FaceAxis::NegZ=>"-Z (down)",
                    })
                    .show_ui(ui,|ui|{
                        for (direction,label) in [
                            (FaceAxis::PosX,"+X"),(FaceAxis::NegX,"-X"),
                            (FaceAxis::PosY,"+Y"),(FaceAxis::NegY,"-Y"),
                            (FaceAxis::PosZ,"+Z (up)"),(FaceAxis::NegZ,"-Z (down)")
                        ] {
                            changed|=ui.selectable_value(axis,direction,label).changed();
                        }
                    });
                changed|=ui.add(egui::Slider::new(sharpness,0.25..=8.0)
                    .text("Facing sharpness")).changed();
                ui.small("Mask = max(0, dot(face_normal, direction)) ^ sharpness.");
                ui.small("2D, selected-node and Flora bakes output 0 without face context.");
                ui.small("Use Mask Mix or Height Blend Mask for directional overlays.");
            }
            NodeKind::FaceSurfaceMask{surface} => {
                ui.small("Voxel-only surface context: sides, top cap or bottom cap.");
                egui::ComboBox::from_id_salt(("face_surface",id))
                    .selected_text(match surface {
                        FaceSurface::Side=>"Side walls",
                        FaceSurface::Top=>"Top cap",
                        FaceSurface::Bottom=>"Bottom cap",
                    })
                    .show_ui(ui,|ui|{
                        changed|=ui.selectable_value(surface,FaceSurface::Side,
                            "Side walls").changed();
                        changed|=ui.selectable_value(surface,FaceSurface::Top,
                            "Top cap").changed();
                        changed|=ui.selectable_value(surface,FaceSurface::Bottom,
                            "Bottom cap").changed();
                    });
                ui.small("Outputs 1 for matching voxel faces, 0 otherwise.");
                ui.small("2D, Node and Flora bakes have no face context: output 0.");
                ui.small("Use a Mask Mix per PBR channel to blend side and cap materials.");
            }
            NodeKind::BitmapColor{asset_id,bilinear} |
            NodeKind::BitmapAlpha{asset_id,bilinear} => {
                ui.small("Embedded PNG; the image path is never used after import.");
                let selected=bitmap_choices.iter().find(|(id,_)|id==asset_id)
                    .map(|(_,label)|label.as_str()).unwrap_or("Unassigned bitmap");
                egui::ComboBox::from_id_salt(("bitmap_asset",id))
                    .selected_text(selected).show_ui(ui,|ui|{
                        changed |= ui.selectable_value(asset_id,0,"Unassigned").changed();
                        for (choice,label) in &bitmap_choices {
                            changed |= ui.selectable_value(asset_id,*choice,label).changed();
                        }
                    });
                changed |= ui.checkbox(bilinear,"Bilinear (otherwise nearest)").changed();
                import_bitmap=ui.button("Import PNG into project").clicked();
                prune_bitmaps=ui.button("Remove unused embedded images").clicked();
                ui.small("Max 512x512 RGBA8; 16 images and 8 MiB pixels per graph.");
            }
            NodeKind::RegionValue{cells,seed} => {
                ui.small("One stable random Scalar per periodic Voronoi region. Threshold to Mask for material variation.");
                changed |= ui.add(egui::Slider::new(cells,2..=32)
                    .text("Cells per tile axis")).changed();
                changed |= ui.add(egui::DragValue::new(seed)
                    .speed(1).prefix("Seed: ")).changed();
                ui.small("Match cells/seed with Region Border Mask to keep regions aligned.");
            }
            NodeKind::RegionBorderMask{cells,seed,width} => {
                ui.small("Typed Mask strongest at Voronoi region borders, fading within each cell.");
                changed |= ui.add(egui::Slider::new(cells,2..=32)
                    .text("Cells per tile axis")).changed();
                changed |= ui.add(egui::DragValue::new(seed)
                    .speed(1).prefix("Seed: ")).changed();
                changed |= ui.add(egui::Slider::new(width,0.01..=0.5)
                    .text("Border width / cell")).changed();
                ui.small("No bitmap assets or new material ownership. Feed Mask into shared PBR blending.");
            }
            NodeKind::HeightBlendMask{influence} => {
                ui.small("Height A + Height B and Base Mask determine one reusable mask.");
                changed |= ui.add(egui::Slider::new(influence,0.0..=1.0)
                    .text("Height influence")).changed();
                ui.small("0: use the base mask exactly; higher height wins in transitions.");
                ui.small("Reuse this output for Albedo, Roughness, Metallic and Height Mask Mix nodes.");
            }
            NodeKind::MaskMix => {
                ui.small("Blend A and B by a typed Mask; connect one shared height blend mask to every PBR channel.");
                ui.small("Scalar and Mask sources can feed Color sockets. Color cannot implicitly feed Mask.");
            }
            NodeKind::Blend { factor } => {
                changed |= ui.add(egui::Slider::new(factor, 0.0..=1.0).text("Mix")).changed();
            }
            NodeKind::Output => {
                ui.label("Connect the final PBR channels here.");
                ui.small("Albedo · Roughness · Metallic · Height");
                ui.small("Normal map is derived from Height during export.");
            }
        }
        ui.collapsing("Socket types and parameter definitions",|ui| {
            if let Some(output)=node.kind.output_definition() {
                ui.small(format!("Output {}: {:?}", output.id, output.data_type));
            }
            for index in 0..node.kind.input_count() {
                if let Some(port)=node.kind.input_definition(index) {
                    ui.small(format!("{} ({}) — {:?}",
                        port.label, port.id, port.data_type));
                }
            }
            for param in node.kind.parameters() {
                let details=match param.control {
                    ParameterControl::Float{min,max,default,unit} =>
                        format!("{min}..{max}, default {default} {unit}"),
                    ParameterControl::Integer{min,max,step,default} =>
                        format!("{min}..{max} (step {step}), default {default}"),
                    ParameterControl::Seed{default} => format!("seed, default {default}"),
                    ParameterControl::Color{default} => format!("RGB, default {default:?}"),
                    ParameterControl::ColorStops{min,max,default} =>
                        format!("{min}..{max} color stops, default {default}"),
                    ParameterControl::CurvePoints{min,max,default} =>
                        format!("{min}..{max} curve points, default {default}"),
                    ParameterControl::Choice{options,default} =>
                        format!("{}; default {default}",options.join(" / ")),
                };
                ui.small(format!("{} [{}]: {details}",param.label,param.id));
            }
        });
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
        if import_bitmap {self.import_png_for_node(id);}
        if prune_bitmaps {
            let removed=self.graph.prune_unused_bitmaps();
            if removed>0 {self.dirty=true;}
            self.status=format!("Removed {removed} unused embedded image(s)");
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

    fn handle_wire_input(&mut self, ui: &egui::Ui, canvas: Rect,
        bezier_overlay:Option<Rect>) {
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
            // The Bézier editor is a draggable surface above the graph.
            // Cables drawn underneath it must not steal point gestures.
            if self.wire_gesture.is_none() && bezier_overlay
                .is_some_and(|bounds|bounds.contains(pointer)) {
                return;
            }
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
                _ => port_type_color(source.kind.output_type()),
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
                    if self.connecting == Some(id) { Color32::LIGHT_GREEN }
                    else { port_type_color(kind.output_type()) });
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
                } else { port_type_color(kind.input_type(input).unwrap_or(SocketType::Color)) };
                painter.circle_filled(socket, 6.0 * z, color);
                painter.text(socket + Vec2::new(12.0 * z, 0.0), Align2::LEFT_CENTER,
                    kind.input_name(input), FontId::proportional(11.0 * z), Color32::WHITE);
            }
        }
        let mut bezier_overlay=None;
        if let Some(id)=self.selected {
            let selection=self.graph.node(id).and_then(|node|
                if let NodeKind::SdfBezierStroke{points,tiled,..}=node.kind {
                    Some((node.position,points,tiled))
                } else {None});
            if let Some((position,points,tiled))=selection {
                let origin=screen_point(rect.min,self.pan,self.zoom,
                    position[0],position[1]);
                if let Some((panel,edited))=bezier_canvas_editor(
                    ui,&painter,rect,id,origin,self.zoom,points,tiled) {
                    bezier_overlay=Some(panel);
                    if let Some(points)=edited {
                        if let Some(node)=self.graph.node_mut(id) {
                            if let NodeKind::SdfBezierStroke{
                                points:actual,..}=&mut node.kind {
                                *actual=points;
                                self.dirty=true;
                            }
                        }
                    }
                }
            }
        }
        self.handle_wire_input(ui, rect, bezier_overlay);
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
        if self.library_tab==2 && self.foliage_mats_dirty {
            self.foliage_mats_dirty=false;
            self.foliage_gpu_scene=None;
            let size=64;
            let started=Instant::now();
            let before=self.foliage_bake_cache.stats().misses;
            let leaves=self.foliage_bake_cache.bake_leaf(
                &self.foliage.materials,size,self.foliage.leaf_color);
            self.metrics.leaf_bake_attempts+=self.foliage_bake_cache.stats().misses-before;
            let before=self.foliage_bake_cache.stats().misses;
            let bark=self.foliage_bake_cache.bake_bark(
                &self.foliage.materials,size,self.foliage.bark_color);
            self.metrics.bark_bake_attempts+=self.foliage_bake_cache.stats().misses-before;
            self.metrics.report("foliage_material_bake",started.elapsed(),
                leaves.is_ok() && bark.is_ok());
            if std::env::var_os("VOXEL_PROFILE_MATERIALS").is_some() {
                let stats=self.foliage_bake_cache.stats();
                eprintln!("[material-profile] event=foliage_bake_cache hits={} misses={} evictions={} resident_bytes={} peak_bytes={}",
                    stats.hits,stats.misses,stats.evictions,
                    stats.resident_bytes,stats.peak_resident_bytes);
            }
            match (leaves,bark) {
                (Ok(leaves),Ok(bark))=>{
                    self.foliage_leaf_pixels=leaves.as_ref().to_vec();
                    self.foliage_bark_pixels=bark.as_ref().to_vec();
                    put_texture_rect(&ctx,&mut self.leaf_texture,"foliage-leaf-material",
                        leaves.as_ref().to_vec(),[size as usize,size as usize]);
                    put_texture_rect(&ctx,&mut self.bark_texture,"foliage-bark-material",
                        bark.as_ref().to_vec(),[size as usize,size as usize]);
                }
                (Err(e),_)|(_,Err(e))=>self.status=format!("Foliage material: {e}"),
            }
        } else if self.library_tab==3 && self.asset_mats_dirty {
            self.asset_mats_dirty=false;
            self.asset_gpu_scene=None;
            let size=64;
            let started=Instant::now();
            let before=self.foliage_bake_cache.stats().misses;
            let leaves=self.foliage_bake_cache.bake_leaf(
                &self.asset.materials,size,self.asset.leaf_color);
            self.metrics.leaf_bake_attempts+=self.foliage_bake_cache.stats().misses-before;
            let before=self.foliage_bake_cache.stats().misses;
            let bark=self.foliage_bake_cache.bake_bark(
                &self.asset.materials,size,self.asset.bark_color);
            self.metrics.bark_bake_attempts+=self.foliage_bake_cache.stats().misses-before;
            self.metrics.report("asset_material_bake",started.elapsed(),
                leaves.is_ok() && bark.is_ok());
            if std::env::var_os("VOXEL_PROFILE_MATERIALS").is_some() {
                let stats=self.foliage_bake_cache.stats();
                eprintln!("[material-profile] event=foliage_bake_cache hits={} misses={} evictions={} resident_bytes={} peak_bytes={}",
                    stats.hits,stats.misses,stats.evictions,
                    stats.resident_bytes,stats.peak_resident_bytes);
            }
            match (leaves,bark) {
                (Ok(leaves),Ok(bark))=>{
                    self.asset_leaf_pixels=leaves.as_ref().to_vec();
                    self.asset_bark_pixels=bark.as_ref().to_vec();
                    put_texture_rect(&ctx,&mut self.asset_leaf_texture,"asset-leaf-material",
                        leaves.as_ref().to_vec(),[size as usize,size as usize]);
                    put_texture_rect(&ctx,&mut self.asset_bark_texture,"asset-bark-material",
                        bark.as_ref().to_vec(),[size as usize,size as usize]);
                }
                (Err(e),_)|(_,Err(e))=>self.status=format!("Asset material: {e}"),
            }
        }
        if self.library_tab == 3 {
            if self.asset_dirty {
                let started=Instant::now();
                let result=self.asset.mesh();
                self.metrics.asset_mesh_builds+=1;
                self.metrics.report("asset_mesh_build",started.elapsed(),result.is_ok());
                match result {
                    Ok(mesh) => self.asset_mesh = Some(mesh),
                    Err(e) => { self.asset_mesh = None; self.status = format!("Asset error: {e}"); }
                }
                self.asset_dirty = false;
                self.asset_gpu_scene=None;
            }
            if self.asset_gpu_scene.is_none() && self.gpu.is_some() {
                if let Some(mesh)=&self.asset_mesh {
                    // Geometry and palette changes reuse the already baked textures.
                    if self.asset_leaf_pixels.len()==64*64*4
                        && self.asset_bark_pixels.len()==64*64*4 {
                        let started=Instant::now();
                        let mut scene=GpuScene::from_mesh(
                            mesh,&self.asset_bark_pixels,&self.asset_leaf_pixels,
                            self.asset.leaf_color,
                            self.asset.materials.effective_palettes(self.asset.leaf_color)
                                .gpu_stops(),
                            if self.asset.materials.leaf_species.is_coffee() {2.0}
                            else if self.asset.materials.leaf_species==PlantPreset::CherryBlossom {1.0}
                            else {0.0}).with_bark_meters_per_repeat(
                                self.asset.materials.bark_meters_per_repeat);
                        if self.gpu_bark_pbr {
                            match preview_bark_pbr_maps(&mut self.preview_render_cache,
                                &self.asset.materials) {
                                Ok((roughness,normal)) =>
                                    scene=scene.with_bark_pbr_maps(roughness,normal),
                                Err(error)=>self.status=
                                    format!("Asset PBR preview unavailable: {error}"),
                            }
                        }
                        self.metrics.asset_scene_builds+=1;
                        self.metrics.report("asset_scene_build",started.elapsed(),true);
                        self.asset_gpu_scene=Some(Arc::new(scene));
                    }
                }
            }
        } else if self.library_tab == 2 {
            if self.foliage_dirty {
                let started=Instant::now();
                let result=self.foliage.generate();
                self.metrics.foliage_generations+=1;
                self.metrics.report("foliage_generate",started.elapsed(),result.is_ok());
                match result {
                    Ok(mesh) => self.foliage_mesh = Some(mesh),
                    Err(err) => { self.foliage_mesh = None; self.status = format!("Foliage error: {err}"); }
                }
                self.foliage_dirty = false;
                self.foliage_gpu_scene=None;
            }
            if self.foliage_gpu_scene.is_none() && self.gpu.is_some() {
                if let Some(mesh)=&self.foliage_mesh {
                    if self.foliage_leaf_pixels.len()==64*64*4
                        && self.foliage_bark_pixels.len()==64*64*4 {
                        let started=Instant::now();
                        let mut scene=GpuScene::from_mesh(
                            mesh,&self.foliage_bark_pixels,&self.foliage_leaf_pixels,
                            self.foliage.leaf_color,
                            self.foliage.materials.effective_palettes(self.foliage.leaf_color)
                                .gpu_stops(),
                            if self.foliage.materials.leaf_species.is_coffee() {2.0}
                            else if self.foliage.materials.leaf_species==PlantPreset::CherryBlossom {1.0}
                            else {0.0}).with_bark_meters_per_repeat(
                                self.foliage.materials.bark_meters_per_repeat);
                        if self.gpu_bark_pbr {
                            match preview_bark_pbr_maps(&mut self.preview_render_cache,
                                &self.foliage.materials) {
                                Ok((roughness,normal)) =>
                                    scene=scene.with_bark_pbr_maps(roughness,normal),
                                Err(error)=>self.status=
                                    format!("Tree PBR preview unavailable: {error}"),
                            }
                        }
                        self.metrics.foliage_scene_builds+=1;
                        self.metrics.report("foliage_scene_build",started.elapsed(),true);
                        self.foliage_gpu_scene=Some(Arc::new(scene));
                    }
                }
            }
        } else {
            self.refresh_preview(&ctx);
        }
        egui::Panel::top("toolbar").show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.heading("VOXEL GAME ENGINE");
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
                        self.foliage_mats_dirty = true;
                        self.status = "New procedural tree created".into();
                    }
                    if ui.button("Open Tree").clicked() { self.open_foliage(); }
                    if ui.button("Save Tree").clicked() { self.save_foliage(); }
                    if ui.button("Export Voxel Tree").clicked() { self.export_foliage(); }
                    if ui.button("Edit Tree as Asset").clicked() { self.import_tree_as_asset(); }
                    ui.separator();
                } else {
                if self.library_tab==0 {
                    if let Some((asset,role))=self.editing_material {
                        if ui.button(format!("Apply {} to {}",role.name(),
                            if asset {"Asset"} else {"Tree"})).clicked() {
                            self.apply_edited_material();
                        }
                    }
                }
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
                if ui.add_enabled(self.preview_channel<4 && self.export_size<=1024,
                    egui::Button::new("Export AA (2×2)")).on_hover_text(
                    "Supersample the selected PBR channel at four subpixel UVs. Slower, useful for SDF silhouettes. Does not change regular exports or normal maps."
                    ).clicked() { self.export_antialiased_png(); }
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

/// Keep the existing semantic colors for four PBR output channels,
/// while displaying actual typed internal graph sockets consistently.
fn port_type_color(kind: SocketType)->Color32 {
    match kind {
        SocketType::Color => Color32::from_rgb(103,170,233),
        SocketType::Scalar => Color32::from_rgb(116,212,190),
        SocketType::Mask => Color32::from_rgb(240,196,107),
        SocketType::Distance => Color32::from_rgb(222,142,222),
        SocketType::Vector2 | SocketType::Vector3 => Color32::from_rgb(178,156,233),
        SocketType::Gradient | SocketType::Curve => Color32::from_rgb(233,147,191),
    }
}

fn node_accent(kind: NodeKind) -> Color32 {
    match kind {
        NodeKind::Noise { .. } | NodeKind::Checker { .. } | NodeKind::Color { .. }
            | NodeKind::Scalar { .. } | NodeKind::Bricks { .. } | NodeKind::Voronoi { .. } | NodeKind::Strata { .. }
            | NodeKind::RegionValue{..} | NodeKind::RegionBorderMask{..}
            | NodeKind::BitmapColor{..} | NodeKind::BitmapAlpha{..}
            | NodeKind::FaceSurfaceMask{..} | NodeKind::FaceFacingMask{..}
            => Color32::from_rgb(43, 95, 131),
        NodeKind::Library { operation, .. } => match operation.category() {
            "Shapes & Gradients" | "Noise" | "Patterns" => Color32::from_rgb(43, 95, 131),
            "Transforms" => Color32::from_rgb(107, 80, 148),
            "Math & Mix" => Color32::from_rgb(143, 96, 58),
            _ => Color32::from_rgb(115, 64, 127),
        },
        NodeKind::Invert | NodeKind::Luminance | NodeKind::Blend { .. }
            | NodeKind::Colorize { .. } | NodeKind::Levels { .. }
            | NodeKind::ThresholdMask { .. } | NodeKind::ColorRamp { .. }
            | NodeKind::CurveRemap { .. } | NodeKind::Reroute { .. }
            | NodeKind::HeightBlendMask{..} | NodeKind::MaskMix
            => Color32::from_rgb(115, 64, 127),
        NodeKind::SdfCircle{..}|NodeKind::SdfEllipse{..}|NodeKind::SdfCapsule{..}
            |NodeKind::SdfBezierStroke{..}|NodeKind::SdfCombine{..}
            |NodeKind::SdfContourWarp{..}|NodeKind::SdfToMask{..}
            |NodeKind::ShapeScatter{..}
            => Color32::from_rgb(119,70,140),
        NodeKind::Output => Color32::from_rgb(43, 105, 77),
    }
}


/// Dragging a selected cubic SDF path edits four *authored* UV handles.
/// The preview uses exactly the same shortest-wrap centerline as baking.
/// This panel is editor-only; it adds no graph, JSON or GPU resource fields.
fn bezier_canvas_editor(ui:&mut egui::Ui,painter:&egui::Painter,
    viewport:Rect,id:u64,node_origin:Pos2,zoom:f32,
    points:[[f32;2];4],tiled:bool)
    ->Option<(Rect,Option<[[f32;2];4]>)> {
    const SIZE:f32=200.0;
    if viewport.width()<SIZE+20.0||viewport.height()<SIZE+20.0 {
        return None;
    }
    let right=node_origin.x+NODE_W*zoom+16.0;
    let desired_left=if right+SIZE+6.0<viewport.right() {
        right
    } else {node_origin.x-SIZE-16.0};
    let left=desired_left.clamp(viewport.left()+6.0,
        viewport.right()-SIZE-6.0);
    let top=node_origin.y.clamp(viewport.top()+6.0,
        viewport.bottom()-SIZE-6.0);
    let panel=Rect::from_min_size(Pos2::new(left,top),
        Vec2::splat(SIZE));
    let plot=Rect::from_min_max(
        panel.min+Vec2::new(18.0,32.0),
        panel.max-Vec2::new(18.0,18.0));
    painter.rect_filled(panel,5.0,Color32::from_rgb(30,34,43));
    painter.rect_stroke(panel,5.0,
        Stroke::new(1.0,Color32::from_rgb(115,125,145)),
        StrokeKind::Inside);
    painter.text(panel.min+Vec2::new(10.0,14.0),Align2::LEFT_CENTER,
        "BÉZIER UV HANDLES",FontId::proportional(11.0),
        Color32::from_rgb(210,221,237));
    painter.rect_stroke(plot,0.0,
        Stroke::new(1.0,Color32::from_rgb(68,77,93)),
        StrokeKind::Inside);
    let map=|point:[f32;2]|Pos2::new(
        plot.left()+point[0]*plot.width(),
        plot.top()+point[1]*plot.height());
    let mut previous:Option<Pos2>=None;
    for i in 0..=64 {
        let t=i as f32/64.0;
        let mut p=texture_studio::sdf_bezier::centerline(points,t,tiled);
        if tiled {
            p[0]=p[0].rem_euclid(1.0);
            p[1]=p[1].rem_euclid(1.0);
        }
        let screen=map(p);
        if let Some(before)=previous {
            // Never draw an artificial line across the square when a
            // periodic centerline crosses a UV tile boundary.
            if (screen.x-before.x).abs()<plot.width()*0.5 &&
                (screen.y-before.y).abs()<plot.height()*0.5 {
                painter.line_segment([before,screen],
                    Stroke::new(2.0,Color32::from_rgb(120,216,192)));
            }
        }
        previous=Some(screen);
    }
    let mut result=points;
    let mut changed=false;
    for (i,point) in points.iter().copied().enumerate() {
        let center=map(point);
        let color=if i==0||i==3 {
            Color32::from_rgb(247,184,103)
        } else {Color32::from_rgb(123,181,247)};
        painter.circle_filled(center,6.0,color);
        painter.circle_stroke(center,7.0,Stroke::new(1.0,Color32::BLACK));
        let response=ui.interact(
            Rect::from_center_size(center,Vec2::splat(18.0)),
            ui.make_persistent_id(("sdf_bezier_handle",id,i)),
            Sense::drag());
        if response.dragged() {
            let delta=ui.input(|i|i.pointer.delta());
            let next=bezier_uv_drag(point,delta,plot.size());
            if next!=point {
                result[i]=next;
                changed=true;
            }
        }
        if response.hovered()||response.dragged() {
            painter.circle_stroke(center,10.0,
                Stroke::new(2.0,Color32::WHITE));
        }
    }
    Some((panel,if changed {Some(result)} else {None}))
}

fn bezier_uv_drag(point:[f32;2],delta:Vec2,plot_size:Vec2)->[f32;2] {
    [
        (point[0]+delta.x/plot_size.x).clamp(0.0,1.0),
        (point[1]+delta.y/plot_size.y).clamp(0.0,1.0),
    ]
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
    fn imported_function_frequency_defaults_to_authored_value() {
        let mut graph=Graph::default();
        if let NodeKind::Noise{scale,..}=&mut graph.node_mut(1).unwrap().kind {
            *scale=23;
        } else {panic!("default graph must expose Noise frequency");}
        let function=MaterialFunction::from_graph("shared_bark".into(),1,graph)
            .unwrap();
        assert_eq!(authored_function_noise_scale(&function),Some(23));
        let instance=MaterialFunctionInstance {
            function_id:function.id.clone(),version:1,
            overrides:vec![FunctionOverride{key:"frequency".into(),value:23}],
        };
        let mut library=MaterialFunctionLibrary::default();
        library.insert(function.clone()).unwrap();
        let resolved=library.resolve(&instance).unwrap();
        assert_eq!(resolved.render_rgba(8,8).unwrap(),
            function.graph.render_rgba(8,8).unwrap());
    }

    #[test]
    fn nested_function_inherits_pinned_child_frequency_not_unused_parent_noise() {
        let mut graph=Graph::default();
        let placeholder=graph.add_node(NodeKind::Color{
            rgb:[240,120,50],
        },[100.0,100.0]);
        graph.connect(placeholder,2,0).unwrap();
        let mut function=MaterialFunction::from_graph(
            "nested_parent".into(),1,graph).unwrap();
        function.calls.push(MaterialFunctionCall{
            placeholder_node_id:placeholder,
            reference:MaterialFunctionInstance{
                function_id:"child".into(),version:1,
                overrides:vec![FunctionOverride{
                    key:"frequency".into(),value:17,
                }],
            },
        });
        function.parameters=vec![FunctionParameter{
            key:"frequency".into(),node_id:placeholder,
            kind:FunctionParameterKind::NestedNoiseScale,
        }];
        function.validate().unwrap();
        assert_eq!(authored_function_noise_scale(&function),Some(17));
    }

    #[test]
    fn dragging_cubic_uv_control_in_preview_is_normalized_and_clamped() {
        assert_eq!(bezier_uv_drag([0.5,0.5],
            Vec2::new(50.0,-25.0),Vec2::new(200.0,100.0)),
            [0.75,0.25]);
        assert_eq!(bezier_uv_drag([0.99,0.01],
            Vec2::new(100.0,-100.0),Vec2::splat(100.0)),
            [1.0,0.0]);
        assert_eq!(bezier_uv_drag([0.25,0.75],
            Vec2::ZERO,Vec2::splat(100.0)),[0.25,0.75]);
    }

    #[test]
    fn leaf_edge_preview_mode_defaults_to_original_cutout() {
        let app=Studio::default();
        assert_eq!(app.leaf_aa_mode,LeafAaMode::Legacy);
        assert!(!app.leaf_mipmaps);
    }

    #[test]
    fn voxel_face_hit_test_accepts_inside_and_rejects_outside() {
        let a=Pos2::new(10.0,10.0);
        let b=Pos2::new(30.0,10.0);
        let c=Pos2::new(10.0,30.0);
        assert!(hit_triangle(Pos2::new(16.0,16.0),a,b,c));
        assert!(!hit_triangle(Pos2::new(40.0,40.0),a,b,c));
    }

    #[test]
    fn voxel_asset_edits_have_undo_and_redo() {
        let mut app=Studio::default();
        assert_eq!(app.library_tab,3);
        let original=app.asset.voxels.len();
        app.asset_mats_dirty=false;
        app.edit_asset(|asset|asset.add([0,1,0],Material::Leaf));
        assert_eq!(app.asset.voxels.len(),original+1);
        app.undo_asset();
        assert!(!app.asset_mats_dirty,"geometry-only undo should reuse material bake");
        assert_eq!(app.asset.voxels.len(),original);
        app.redo_asset();
        assert!(!app.asset_mats_dirty,"geometry-only redo should reuse material bake");
        assert_eq!(app.asset.voxels.len(),original+1);
        app.edit_asset(|asset|asset.add([0,1,0],Material::Bark));
        assert_eq!(app.asset.voxels.len(),original+1);
    }

    #[test]
    fn material_change_restored_from_asset_history_requires_bake() {
        let mut app=Studio::default();
        let original=app.asset.clone();
        let mut changed=original.clone();
        changed.materials.bark_tint_strength=1.0;
        app.asset=changed;
        app.asset_undo.push(original);
        app.asset_mats_dirty=false;
        app.undo_asset();
        assert!(app.asset_mats_dirty);
    }

    #[test]
    fn foliage_material_graphs_transfer_to_asset_and_save() {
        let mut app=Studio::default();
        assert!(app.foliage.materials.bark.is_some());
        assert!(app.foliage.materials.leaf.is_some());
        app.import_tree_as_asset();
        assert!(app.asset.materials.bark.is_some());
        assert!(app.asset.materials.leaf.is_some());
        assert!(app.asset.material_graph.is_none(),
            "active Nodes graph must not overwrite imported tree material roles");
        let mut seasonal=app.asset.materials.effective_palettes(app.asset.leaf_color);
        seasonal.seasons[2][0]=[220,80,39];
        app.asset.materials.seasonal_palettes=Some(seasonal.clone());
        let json=app.asset.to_json().unwrap();
        let saved=VoxelAsset::from_json(&json).unwrap();
        assert!(saved.materials.bark.is_some());
        assert!(saved.materials.leaf.is_some());
        assert_eq!(saved.materials.seasonal_palettes,Some(seasonal));
        assert!(saved.mtl().contains("bark-albedo.png"));
        assert!(saved.mtl().contains("foliage-leaves-alpha.png"));
        app.editing_material=Some((true,MaterialRole::Opacity));
        let mut graph=Graph::default();
        graph.nodes[0].kind=NodeKind::Color{rgb:[0,0,0]};
        app.graph=graph;
        app.apply_edited_material();
        assert!(app.asset.materials.leaf_opacity.is_some());
        assert!(app.asset_mats_dirty);
    }

    #[test]
    fn generated_tree_converts_into_independently_editable_asset() {
        let mut app=Studio::default();
        app.library_tab=2;
        app.import_tree_as_asset();
        assert_eq!(app.library_tab,3);
        assert_eq!(app.asset.shape,VoxelShape::HexPrism);
        assert_eq!(app.asset.voxel_size,1.0);
        assert!(app.asset.voxels.len()>4);
        assert!(app.asset.material_graph.is_none());
        assert!(app.asset.mesh().is_ok());
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
    // wgpu owns window presentation AND the voxel editor's 3D renderer.
    // Prefer Vulkan on native systems, with Metal, DX12, and GL fallback.
    let mut wgpu_options=eframe::egui_wgpu::WgpuConfiguration::default();
    if let eframe::egui_wgpu::WgpuSetup::CreateNew(ref mut setup)
        =wgpu_options.wgpu_setup {
        setup.native_adapter_selector=Some(Arc::new(|adapters,surface| {
            let score=|adapter:&eframe::egui_wgpu::wgpu::Adapter| {
                use eframe::egui_wgpu::wgpu::Backend;
                match adapter.get_info().backend {
                    Backend::Vulkan=>5,
                    Backend::Metal=>4,
                    Backend::Dx12=>3,
                    Backend::Gl=>2,
                    _=>1,
                }
            };
            adapters.iter().filter(|adapter|
                surface.is_none_or(|surface|!surface.get_capabilities(adapter).formats.is_empty()))
                .max_by_key(|adapter|score(adapter)).cloned()
                .ok_or_else(||"No wgpu adapter can present to this window".to_owned())
        }));
    }
    let options = eframe::NativeOptions {
        renderer:eframe::Renderer::Wgpu,
        wgpu_options,
        viewport: egui::ViewportBuilder::default()
            .with_title("Voxel Game Engine — Voxel Asset Editor")
            .with_inner_size([1330.0, 780.0])
            .with_min_inner_size([940.0, 550.0]),
        ..Default::default()
    };
    eframe::run_native("voxel-game-engine", options, Box::new(|cc| Ok(Box::new(Studio::new(cc)))))

    #[test]
    fn bark_gpu_pbr_is_opt_in_and_uses_authored_graph_maps() {
        let app=Studio::default();
        assert!(!app.gpu_bark_pbr);
        let mut cache=GraphRenderCache::new(1024*1024);
        let mats=FoliageMaterials::for_species(PlantPreset::Oak);
        let (rough,normal)=preview_bark_pbr_maps(&mut cache,&mats).unwrap();
        assert_eq!(rough.len(),(MATERIAL_SIZE*MATERIAL_SIZE*4) as usize);
        assert_eq!(normal.len(),rough.len());
        let misses=cache.stats().misses;
        let again=preview_bark_pbr_maps(&mut cache,&mats).unwrap();
        assert_eq!((rough,normal),again);
        assert_eq!(cache.stats().misses,misses,
            "unchanged bark maps should be served from preview cache");
        let (fallback_r,fallback_n)=preview_bark_pbr_maps(
            &mut cache,&FoliageMaterials::default()).unwrap();
        assert_eq!(&fallback_r[..4],&[204,204,204,255]);
        assert_eq!(&fallback_n[..4],&[128,128,255,255]);
    }
}
