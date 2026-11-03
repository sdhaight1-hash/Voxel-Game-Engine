//! Real wgpu 3D viewport: uploaded voxel triangles, depth-tested GPU rasterization,
//! WGSL camera, alpha-cutout fragments, GPU wind, and baked material textures.
//! Uses eframe's *same* wgpu device, not a second window or graphics context.
use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use eframe::{egui, egui_wgpu::{self, wgpu, wgpu::util::DeviceExt}};

use texture_studio::foliage::{FoliageMesh, Material};

const TARGET_SIZE: u32 = 1024;
#[cfg(test)]
const TEX_SIZE: u32 = 64;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Vertex {
    position: [f32; 3],
    uv: [f32; 2],
    tint: [f32; 4],
    kind: f32,
    sway_weight: f32,
    phase: f32,
    /// Mesh-ring perimeter in meters, or zero for legacy/leaf geometry.
    bark_circumference_m: f32,
    /// Geometric object-local surface normal (flat per triangle).
    normal: [f32;3],
    /// UV tangent xyz with bitangent handedness in w. Not MikkTSpace.
    tangent: [f32;4],
}
const ATTRIBUTES: [wgpu::VertexAttribute; 9] =
    wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x2,
        2 => Float32x4, 3 => Float32, 4 => Float32, 5 => Float32,
        6 => Float32, 7 => Float32x3, 8 => Float32x4];

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Uniforms {
    center: [f32; 4],
    right: [f32; 4],
    up: [f32; 4],
    view: [f32; 4],
    wind: [f32; 4],
    season: [f32; 4],
    leaf_base: [f32; 4],
    /// x = bark meters per repeat, zero disables physical UV mapping.
    /// y = opt-in roughness/normal dielectric BRDF.
    bark_mapping: [f32;4],
}

/// Preview-only render mode. Never persisted to tree materials or assets.
#[derive(Clone,Copy,Debug,Default,PartialEq,Eq)]
pub enum LeafAaMode {
    #[default]
    Legacy,
    Ordered,
    Msaa4,
}

fn leaf_coverage_uniform(mode:LeafAaMode)->f32 {
    match mode {
        LeafAaMode::Legacy=>0.0,
        LeafAaMode::Ordered=>1.0,
        LeafAaMode::Msaa4=>2.0,
    }
}

fn leaf_mipmap_uniform(enabled:bool)->f32 {
    if enabled {1.0} else {0.0}
}

/// The palette is uploaded only when the authored scene changes. Per-frame
/// season progress stays in the small camera uniform instead.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GpuPalette {
    stops: [[f32;4];16],
}

/// Immutable scene is rebuilt only when voxel geometry or material bake changes.
/// Orbit, zoom, time, and wind do not re-upload the vertex buffer.
pub struct Scene {
    vertices: Vec<Vertex>,
    pub center: [f32; 3],
    pub span: f32,
    bark_rgba: Vec<u8>,
    bark_repeat_m: f32,
    bark_roughness_rgba: Vec<u8>,
    bark_normal_rgba: Vec<u8>,
    /// Graph Metallic output, R8 linear factor carried in RGBA8 data.
    bark_metallic_rgba: Vec<u8>,
    bark_pbr_enabled: bool,
    leaf_rgba: Vec<u8>,
    palette: GpuPalette,
    leaf_base: [f32;4],
    wind_strength: f32,
    wind_speed: f32,
    voxel_size: f32,
    flower_kind: f32, // 0 none, 1 pink ornamental cherry, 2 white coffee
}

impl Scene {
    pub fn from_mesh(mesh: &FoliageMesh, bark: &[u8], leaf: &[u8],
                     leaf_tint: [u8; 3],palette:[[f32;4];16],
                     flower_kind:f32) -> Self {
        let mut min = [f32::INFINITY;3];
        let mut max = [f32::NEG_INFINITY;3];
        let mut vertices = Vec::with_capacity(mesh.triangles.len() * 3);
        let light = [0.46, 0.35, 0.81];
        for face in &mesh.triangles {
            let (surface_normal,tangent)=
                triangle_tangent_frame(face.points,face.uv);
            let p = &face.points;
            let a = [p[1][0]-p[0][0],p[1][1]-p[0][1],p[1][2]-p[0][2]];
            let b = [p[2][0]-p[0][0],p[2][1]-p[0][1],p[2][2]-p[0][2]];
            let normal = [
                a[1]*b[2]-a[2]*b[1],
                a[2]*b[0]-a[0]*b[2],
                a[0]*b[1]-a[1]*b[0],
            ];
            let length=(normal[0]*normal[0]+normal[1]*normal[1]+normal[2]*normal[2])
                .sqrt().max(1e-6);
            let diffuse=(normal[0]*light[0]+normal[1]*light[1]+normal[2]*light[2])
                .abs()/length;
            let shade=0.68+0.32*diffuse;
            let (kind,multiplier)=match face.material {
                Material::Bark=>(0.0,1.0),
                Material::LeafDark=>(if face.cutout {1.0} else {2.0},0.72),
                Material::Leaf=>(if face.cutout {1.0} else {2.0},1.0),
                Material::LeafLight=>(if face.cutout {1.0} else {2.0},1.20),
            };
            let tint=if kind>1.5 {
                [
                    leaf_tint[0] as f32/255.0*shade*multiplier,
                    leaf_tint[1] as f32/255.0*shade*multiplier,
                    leaf_tint[2] as f32/255.0*shade*multiplier,1.0
                ]
            } else {
                [shade*multiplier,shade*multiplier,shade*multiplier,1.0]
            };
            let phase=face.cell[0] as f32*0.37
                +face.cell[1] as f32*0.71+face.cell[2] as f32*0.19;
            for i in 0..3 {
                let pos=p[i];
                for j in 0..3 {
                    min[j]=min[j].min(pos[j]);max[j]=max[j].max(pos[j]);
                }
                vertices.push(Vertex {
                    position:pos,
                    uv:face.uv[i],
                    tint,
                    kind,
                    sway_weight:face.wind_weight[i],
                    phase,
                    bark_circumference_m:face.bark_circumference_m[i],
                    normal:surface_normal,
                    tangent,
                });
            }
        }
        if vertices.is_empty() {min=[-1.0;3];max=[1.0;3];}
        let center=std::array::from_fn(|axis| (min[axis]+max[axis])*0.5);
        let span=((max[0]-min[0]).max(max[1]-min[1])
            .max(max[2]-min[2])).max(mesh.voxel_size);
        Self {
            vertices,center,span,
            bark_rgba:bark.to_vec(),bark_repeat_m:0.0,
            bark_roughness_rgba:vec![204,204,204,255],
            bark_normal_rgba:vec![128,128,255,255],
            bark_metallic_rgba:vec![0,0,0,255],
            bark_pbr_enabled:false,
            leaf_rgba:leaf.to_vec(),
            palette:GpuPalette{stops:palette},
            leaf_base:[leaf_tint[0] as f32/255.0,leaf_tint[1] as f32/255.0,
                leaf_tint[2] as f32/255.0,1.0],
            wind_strength:mesh.leaf_render.wind_strength,
            wind_speed:mesh.leaf_render.wind_speed,
            voxel_size:mesh.voxel_size,
            flower_kind,
        }
    }

    /// Changing only this authoring parameter must not modify mesh vertices,
    /// bark RGBA or palette data; the next frame uploads its small uniform.
    pub fn with_bark_meters_per_repeat(mut self,meters:Option<f32>)->Self {
        self.bark_repeat_m=meters.filter(|m|
            m.is_finite() && (0.05..=8.0).contains(m)).unwrap_or(0.0);
        self
    }

    /// PBR maps are graph-authored linear UNORM data, never sRGB colors:
    /// roughness and metallic are scalar R, normals are tangent XYZ in RGB.
    /// The legacy branch remains disabled by default for every species.
    pub fn with_bark_pbr_maps(mut self,roughness:Vec<u8>,normal:Vec<u8>,
                              metallic:Vec<u8>)->Self {
        assert!(!roughness.is_empty()&&roughness.len()%4==0);
        assert_eq!(roughness.len(),normal.len());
        assert_eq!(roughness.len(),metallic.len());
        self.bark_roughness_rgba=roughness;
        self.bark_normal_rgba=normal;
        self.bark_metallic_rgba=metallic;
        self.bark_pbr_enabled=true;
        self
    }

    #[cfg(test)]
    pub fn triangle_count(&self)->usize {self.vertices.len()/3}
}

/// One object-space TBN per authored triangle, retaining UV handedness on
/// branch rings, clipped hexels and ordinary block faces. Degenerate UVs
/// use a deterministic orthogonal tangent rather than NaN coordinates.
///
/// This is flat per-face tangent space, not MikkTSpace vertex smoothing.
fn triangle_tangent_frame(p:[[f32;3];3],uv:[[f32;2];3])
    ->([f32;3],[f32;4]) {
    let sub=|a:[f32;3],b:[f32;3]|->
        [f32;3] {[a[0]-b[0],a[1]-b[1],a[2]-b[2]]};
    let dot=|a:[f32;3],b:[f32;3]|->
        f32 {a[0]*b[0]+a[1]*b[1]+a[2]*b[2]};
    let cross=|a:[f32;3],b:[f32;3]|->
        [f32;3] {[a[1]*b[2]-a[2]*b[1],
                   a[2]*b[0]-a[0]*b[2],
                   a[0]*b[1]-a[1]*b[0]]};
    let normalize=|v:[f32;3]|->Option<[f32;3]> {
        let length=dot(v,v).sqrt();
        (length>1e-7 && length.is_finite())
            .then(||v.map(|a|a/length))
    };
    let e1=sub(p[1],p[0]);
    let e2=sub(p[2],p[0]);
    let n=normalize(cross(e1,e2)).unwrap_or([0.0,0.0,1.0]);
    let du1=uv[1][0]-uv[0][0];
    let dv1=uv[1][1]-uv[0][1];
    let du2=uv[2][0]-uv[0][0];
    let dv2=uv[2][1]-uv[0][1];
    let det=du1*dv2-du2*dv1;
    let fallback=|| {
        let axis=if n[2].abs()<0.9 {[0.0,0.0,1.0]}
            else {[0.0,1.0,0.0]};
        normalize(cross(axis,n)).unwrap_or([1.0,0.0,0.0])
    };
    if !det.is_finite() || det.abs()<1e-8 {
        let t=fallback();
        return (n,[t[0],t[1],t[2],1.0]);
    }
    let raw_t=std::array::from_fn(|i|(e1[i]*dv2-e2[i]*dv1)/det);
    let raw_b=std::array::from_fn(|i|(e2[i]*du1-e1[i]*du2)/det);
    let projected=std::array::from_fn(|i|raw_t[i]-n[i]*dot(n,raw_t));
    let t=normalize(projected).unwrap_or_else(fallback);
    let handedness=if dot(cross(n,t),raw_b)<0.0 {-1.0} else {1.0};
    (n,[t[0],t[1],t[2],handedness])
}

/// The scene Arc can change for geometry, palette, or material edits.
#[derive(Debug, PartialEq, Eq)]
struct SceneUploads {
    geometry: bool,
    bark: bool,
    leaf: bool,
    palette: bool,
    bark_roughness: bool,
    bark_normal: bool,
    bark_metallic: bool,
}

fn scene_uploads(previous: Option<&Scene>, next: &Scene) -> SceneUploads {
    SceneUploads {
        geometry: previous.is_none_or(|old|
            bytemuck::cast_slice::<Vertex,u8>(&old.vertices)
                != bytemuck::cast_slice::<Vertex,u8>(&next.vertices)),
        bark: previous.is_none_or(|old|old.bark_rgba!=next.bark_rgba),
        leaf: previous.is_none_or(|old|old.leaf_rgba!=next.leaf_rgba),
        palette: previous.is_none_or(|old|old.palette.stops!=next.palette.stops),
        bark_roughness: previous.is_none_or(|old|
            old.bark_roughness_rgba!=next.bark_roughness_rgba),
        bark_normal: previous.is_none_or(|old|
            old.bark_normal_rgba!=next.bark_normal_rgba),
        bark_metallic: previous.is_none_or(|old|
            old.bark_metallic_rgba!=next.bark_metallic_rgba),
    }
}

/// Counts CPU-side resource upload calls, not GPU execution time.
#[derive(Default, Debug, PartialEq, Eq)]
struct UploadMetrics {
    vertex_buffers: u64,
    bark_textures: u64,
    leaf_textures: u64,
    palette_writes: u64,
    roughness_textures: u64,
    normal_textures: u64,
    metallic_textures: u64,
}

impl UploadMetrics {
    fn record(&mut self, uploads: &SceneUploads) {
        self.vertex_buffers+=u64::from(uploads.geometry);
        self.bark_textures+=u64::from(uploads.bark);
        self.leaf_textures+=u64::from(uploads.leaf);
        self.palette_writes+=u64::from(uploads.palette);
        self.roughness_textures+=u64::from(uploads.bark_roughness);
        self.normal_textures+=u64::from(uploads.bark_normal);
        self.metallic_textures+=u64::from(uploads.bark_metallic);
    }
}

pub struct GpuViewport {
    pub texture_id: egui::TextureId,
    pub backend: String,
    pub adapter: String,
    pub msaa4_supported: bool,
}

struct Resources {
    pipeline: wgpu::RenderPipeline,
    pipeline_msaa4: Option<wgpu::RenderPipeline>,
    _msaa4_color: Option<wgpu::Texture>,
    msaa4_color_view: Option<wgpu::TextureView>,
    _msaa4_depth: Option<wgpu::Texture>,
    msaa4_depth_view: Option<wgpu::TextureView>,
    group_layout: wgpu::BindGroupLayout,
    uniform: wgpu::Buffer,
    palette_buffer: wgpu::Buffer,
    bind_group: Option<wgpu::BindGroup>,
    bark_view: Option<wgpu::TextureView>,
    leaf_view: Option<wgpu::TextureView>,
    bark_roughness_view: Option<wgpu::TextureView>,
    bark_normal_view: Option<wgpu::TextureView>,
    bark_metallic_view: Option<wgpu::TextureView>,
    sampler: Option<wgpu::Sampler>,
    vertex: Option<wgpu::Buffer>,
    vertex_count: u32,
    last_scene: Option<Arc<Scene>>,
    upload_metrics: UploadMetrics,
    _target: wgpu::Texture,
    target_view: wgpu::TextureView,
    _depth: wgpu::Texture,
    depth_view: wgpu::TextureView,
}

impl GpuViewport {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Option<Self> {
        let rs=cc.wgpu_render_state.as_ref()?;
        let device=&rs.device;
        let info=rs.adapter.get_info();
        // The legacy single-sample preview remains available on adapters
        // without 4x MSAA support for *both* color and depth formats.
        let color_flags=rs.adapter.get_texture_format_features(
            wgpu::TextureFormat::Rgba8Unorm).flags;
        let depth_flags=rs.adapter.get_texture_format_features(
            wgpu::TextureFormat::Depth24Plus).flags;
        let msaa4_supported=color_flags.contains(
            wgpu::TextureFormatFeatureFlags::MULTISAMPLE_X4 |
            wgpu::TextureFormatFeatureFlags::MULTISAMPLE_RESOLVE)
            && depth_flags.contains(wgpu::TextureFormatFeatureFlags::MULTISAMPLE_X4);

        let shader=device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label:Some("voxel-game-engine-wgsl"),
            source:wgpu::ShaderSource::Wgsl(include_str!("voxel_gpu.wgsl").into()),
        });
        let group_layout=device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label:Some("voxel-material-uniforms"),
            entries:&[
                wgpu::BindGroupLayoutEntry {
                    binding:0,visibility:wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty:wgpu::BindingType::Buffer {
                        ty:wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset:false,min_binding_size:None,
                    },count:None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding:1,visibility:wgpu::ShaderStages::FRAGMENT,
                    ty:wgpu::BindingType::Texture {
                        multisampled:false,
                        view_dimension:wgpu::TextureViewDimension::D2,
                        sample_type:wgpu::TextureSampleType::Float {filterable:true},
                    },count:None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding:2,visibility:wgpu::ShaderStages::FRAGMENT,
                    ty:wgpu::BindingType::Texture {
                        multisampled:false,
                        view_dimension:wgpu::TextureViewDimension::D2,
                        sample_type:wgpu::TextureSampleType::Float {filterable:true},
                    },count:None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding:3,visibility:wgpu::ShaderStages::FRAGMENT,
                    ty:wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count:None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding:4,visibility:wgpu::ShaderStages::FRAGMENT,
                    ty:wgpu::BindingType::Buffer {
                        ty:wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset:false,min_binding_size:None,
                    },count:None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding:5,visibility:wgpu::ShaderStages::FRAGMENT,
                    ty:wgpu::BindingType::Texture {
                        multisampled:false,
                        view_dimension:wgpu::TextureViewDimension::D2,
                        sample_type:wgpu::TextureSampleType::Float{filterable:true},
                    },count:None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding:6,visibility:wgpu::ShaderStages::FRAGMENT,
                    ty:wgpu::BindingType::Texture {
                        multisampled:false,
                        view_dimension:wgpu::TextureViewDimension::D2,
                        sample_type:wgpu::TextureSampleType::Float{filterable:true},
                    },count:None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding:7,visibility:wgpu::ShaderStages::FRAGMENT,
                    ty:wgpu::BindingType::Texture {
                        multisampled:false,
                        view_dimension:wgpu::TextureViewDimension::D2,
                        sample_type:wgpu::TextureSampleType::Float{filterable:true},
                    },count:None,
                },
            ],
        });
        let pipeline_layout=device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor{
            label:Some("voxel-pbr-pipeline-layout"),
            bind_group_layouts:&[Some(&group_layout)],
            immediate_size:0,
        });
        let vertex_buffers=[Some(wgpu::VertexBufferLayout{
            array_stride:std::mem::size_of::<Vertex>() as u64,
            step_mode:wgpu::VertexStepMode::Vertex,
            attributes:&ATTRIBUTES,
        })];
        let fragment_targets=[Some(wgpu::ColorTargetState{
            format:wgpu::TextureFormat::Rgba8Unorm,
            blend:None,
            write_mask:wgpu::ColorWrites::ALL,
        })];
        let pipeline_descriptor=wgpu::RenderPipelineDescriptor{
            label:Some("voxel-depth-texture-cutout-pipeline"),
            layout:Some(&pipeline_layout),
            vertex:wgpu::VertexState{
                module:&shader,entry_point:Some("vs_main"),
                buffers:&vertex_buffers,
                compilation_options:wgpu::PipelineCompilationOptions::default(),
            },
            primitive:wgpu::PrimitiveState{
                topology:wgpu::PrimitiveTopology::TriangleList,
                cull_mode:None, // two-sided foliage
                ..Default::default()
            },
            depth_stencil:Some(wgpu::DepthStencilState{
                format:wgpu::TextureFormat::Depth24Plus,
                depth_write_enabled:Some(true),
                depth_compare:Some(wgpu::CompareFunction::Less),
                stencil:wgpu::StencilState::default(),
                bias:wgpu::DepthBiasState::default(),
            }),
            multisample:wgpu::MultisampleState::default(),
            fragment:Some(wgpu::FragmentState{
                module:&shader,entry_point:Some("fs_main"),
                targets:&fragment_targets,
                compilation_options:wgpu::PipelineCompilationOptions::default(),
            }),
            multiview_mask:None,cache:None,
        };
        let pipeline=device.create_render_pipeline(&pipeline_descriptor);
        let pipeline_msaa4=msaa4_supported.then(||device.create_render_pipeline(
            &wgpu::RenderPipelineDescriptor{
                label:Some("voxel-4x-msaa-alpha-to-coverage"),
                multisample:wgpu::MultisampleState{
                    count:4,mask:!0,alpha_to_coverage_enabled:true,
                },
                ..pipeline_descriptor
            }));
        let texture_descriptor=wgpu::TextureDescriptor {
            label:Some("voxel-viewport-color"),
            size:wgpu::Extent3d{width:TARGET_SIZE,height:TARGET_SIZE,depth_or_array_layers:1},
            mip_level_count:1,sample_count:1,
            dimension:wgpu::TextureDimension::D2,
            format:wgpu::TextureFormat::Rgba8Unorm,
            usage:wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats:&[],
        };
        let target=device.create_texture(&texture_descriptor);
        let target_view=target.create_view(&Default::default());
        let depth=device.create_texture(&wgpu::TextureDescriptor {
            label:Some("voxel-viewport-depth"),
            format:wgpu::TextureFormat::Depth24Plus,
            usage:wgpu::TextureUsages::RENDER_ATTACHMENT,
            ..texture_descriptor
        });
        let depth_view=depth.create_view(&Default::default());
        let msaa4_color=msaa4_supported.then(||
            device.create_texture(&wgpu::TextureDescriptor{
                label:Some("voxel-msaa4-color"),
                size:wgpu::Extent3d{
                    width:TARGET_SIZE,height:TARGET_SIZE,depth_or_array_layers:1,
                },
                mip_level_count:1,sample_count:4,
                dimension:wgpu::TextureDimension::D2,
                format:wgpu::TextureFormat::Rgba8Unorm,
                usage:wgpu::TextureUsages::RENDER_ATTACHMENT,view_formats:&[],
            }));
        let msaa4_color_view=msaa4_color.as_ref()
            .map(|t|t.create_view(&Default::default()));
        let msaa4_depth=msaa4_supported.then(||
            device.create_texture(&wgpu::TextureDescriptor{
                label:Some("voxel-msaa4-depth"),
                size:wgpu::Extent3d{
                    width:TARGET_SIZE,height:TARGET_SIZE,depth_or_array_layers:1,
                },
                mip_level_count:1,sample_count:4,
                dimension:wgpu::TextureDimension::D2,
                format:wgpu::TextureFormat::Depth24Plus,
                usage:wgpu::TextureUsages::RENDER_ATTACHMENT,view_formats:&[],
            }));
        let msaa4_depth_view=msaa4_depth.as_ref()
            .map(|t|t.create_view(&Default::default()));
        let uniform=device.create_buffer_init(&wgpu::util::BufferInitDescriptor{
            label:Some("voxel-camera-and-wind"),
            contents:bytemuck::bytes_of(&Uniforms::zeroed()),
            usage:wgpu::BufferUsages::COPY_DST|wgpu::BufferUsages::UNIFORM,
        });
        let palette_buffer=device.create_buffer_init(&wgpu::util::BufferInitDescriptor{
            label:Some("voxel-foliage-season-palettes"),
            contents:bytemuck::bytes_of(&GpuPalette::zeroed()),
            usage:wgpu::BufferUsages::COPY_DST|wgpu::BufferUsages::UNIFORM,
        });
        let texture_id=rs.renderer.write().register_native_texture(
            device,&target_view,wgpu::FilterMode::Linear);
        rs.renderer.write().callback_resources.insert(Resources {
            pipeline,pipeline_msaa4,
            _msaa4_color:msaa4_color,msaa4_color_view,
            _msaa4_depth:msaa4_depth,msaa4_depth_view,
            group_layout,uniform,palette_buffer,bind_group:None,
            bark_view:None,leaf_view:None,
            bark_roughness_view:None,bark_normal_view:None,
            bark_metallic_view:None,sampler:None,
            vertex:None,vertex_count:0,last_scene:None,
            upload_metrics:UploadMetrics::default(),
            _target:target,target_view,
            _depth:depth,depth_view,
        });
        Some(Self{texture_id,backend:format!("{:?}",info.backend),
            adapter:info.name,msaa4_supported})
    }

    pub fn paint(&self,ui:&egui::Ui, rect:egui::Rect,scene:Arc<Scene>,
                 yaw:f32, zoom:f32, time:f32,season_progress:f32,
                 leaf_aa_mode:LeafAaMode,leaf_mipmaps:bool) {
        // Egui presents the GPU render target, populated during callback.prepare.
        ui.painter().image(self.texture_id,rect,
            egui::Rect::from_min_max(egui::Pos2::ZERO,egui::Pos2::new(1.0,1.0)),
            egui::Color32::WHITE);
        ui.painter().add(egui_wgpu::Callback::new_paint_callback(
            rect,RenderScene{scene,yaw,zoom,time,season_progress,
                leaf_aa_mode:if leaf_aa_mode==LeafAaMode::Msaa4 && !self.msaa4_supported {
                    LeafAaMode::Legacy
                } else {leaf_aa_mode},leaf_mipmaps,
                width:rect.width(),height:rect.height()}));
    }
}

struct RenderScene {
    scene: Arc<Scene>,
    yaw:f32,
    zoom:f32,
    time:f32,
    season_progress:f32,
    leaf_aa_mode:LeafAaMode,
    leaf_mipmaps:bool,
    width:f32,
    height:f32,
}

impl egui_wgpu::CallbackTrait for RenderScene {
    fn prepare(
        &self,device:&wgpu::Device,queue:&wgpu::Queue,
        _screen:&egui_wgpu::ScreenDescriptor,
        encoder:&mut wgpu::CommandEncoder,
        resources:&mut egui_wgpu::CallbackResources,
    )->Vec<wgpu::CommandBuffer> {
        let Some(resources)=resources.get_mut::<Resources>() else {return Vec::new()};
        if !resources.last_scene.as_ref()
            .is_some_and(|old|Arc::ptr_eq(old,&self.scene)) {
            let started=std::time::Instant::now();
            let scene=&self.scene;
            let uploads=scene_uploads(resources.last_scene.as_deref(),scene);
            if uploads.geometry {
                let vertices=if scene.vertices.is_empty() {vec![Vertex::zeroed()]}
                    else {scene.vertices.clone()};
                resources.vertex=Some(device.create_buffer_init(
                    &wgpu::util::BufferInitDescriptor {
                        label:Some("voxel-geometry-vertices"),
                        contents:bytemuck::cast_slice(&vertices),
                        usage:wgpu::BufferUsages::VERTEX,
                    }));
                resources.vertex_count=scene.vertices.len() as u32;
            }
            if uploads.palette {
                queue.write_buffer(&resources.palette_buffer,0,
                    bytemuck::bytes_of(&scene.palette));
            }
            if uploads.bark {
                resources.bark_view=Some(upload_rgba(device,queue,&scene.bark_rgba)
                    .create_view(&Default::default()));
            }
            if uploads.leaf {
                resources.leaf_view=Some(upload_leaf_rgba_mips(device,queue,&scene.leaf_rgba)
                    .create_view(&Default::default()));
            }
            if uploads.bark_roughness {
                resources.bark_roughness_view=Some(
                    upload_rgba(device,queue,&scene.bark_roughness_rgba)
                        .create_view(&Default::default()));
            }
            if uploads.bark_normal {
                resources.bark_normal_view=Some(
                    upload_rgba(device,queue,&scene.bark_normal_rgba)
                        .create_view(&Default::default()));
            }
            if uploads.bark_metallic {
                resources.bark_metallic_view=Some(
                    upload_rgba(device,queue,&scene.bark_metallic_rgba)
                        .create_view(&Default::default()));
            }
            if uploads.bark || uploads.leaf || uploads.bark_roughness
                || uploads.bark_normal || uploads.bark_metallic
                || resources.bind_group.is_none() {
                let sampler=resources.sampler.get_or_insert_with(||
                    device.create_sampler(&wgpu::SamplerDescriptor {
                        label:Some("voxel-pixel-sampler"),
                        mag_filter:wgpu::FilterMode::Linear,
                        min_filter:wgpu::FilterMode::Linear,
                        mipmap_filter:wgpu::MipmapFilterMode::Linear,
                        address_mode_u:wgpu::AddressMode::Repeat,
                        address_mode_v:wgpu::AddressMode::Repeat,
                        ..Default::default()
                    }));
                resources.bind_group=Some(device.create_bind_group(&wgpu::BindGroupDescriptor{
                    label:Some("voxel-material-bindings"),
                    layout:&resources.group_layout,
                    entries:&[
                        wgpu::BindGroupEntry{binding:0,resource:resources.uniform.as_entire_binding()},
                        wgpu::BindGroupEntry{binding:1,resource:wgpu::BindingResource::TextureView(
                            resources.bark_view.as_ref().expect("bark uploaded"))},
                        wgpu::BindGroupEntry{binding:2,resource:wgpu::BindingResource::TextureView(
                            resources.leaf_view.as_ref().expect("leaf uploaded"))},
                        wgpu::BindGroupEntry{binding:3,resource:wgpu::BindingResource::Sampler(sampler)},
                        wgpu::BindGroupEntry{binding:4,
                            resource:resources.palette_buffer.as_entire_binding()},
                        wgpu::BindGroupEntry{binding:5,
                            resource:wgpu::BindingResource::TextureView(
                                resources.bark_roughness_view.as_ref()
                                    .expect("bark roughness uploaded"))},
                        wgpu::BindGroupEntry{binding:6,
                            resource:wgpu::BindingResource::TextureView(
                                resources.bark_normal_view.as_ref()
                                    .expect("bark normal uploaded"))},
                        wgpu::BindGroupEntry{binding:7,
                            resource:wgpu::BindingResource::TextureView(
                                resources.bark_metallic_view.as_ref()
                                    .expect("bark metallic uploaded"))},
                    ],
                }));
            }
            resources.upload_metrics.record(&uploads);
            if std::env::var_os("VOXEL_PROFILE_MATERIALS").is_some() {
                let m=&resources.upload_metrics;
                eprintln!(
                    "[material-profile] event=gpu_scene_upload cpu_ms={:.3} geometry={} bark={} leaf={} palette={} vertex_uploads={} bark_uploads={} leaf_uploads={} palette_writes={}",
                    started.elapsed().as_secs_f64()*1000.0,
                    uploads.geometry,uploads.bark,uploads.leaf,uploads.palette,
                    m.vertex_buffers,m.bark_textures,m.leaf_textures,m.palette_writes,
                );
            }
            resources.last_scene=Some(self.scene.clone());
        }

        let yaw=self.yaw;
        let elevation=0.4_f32;
        let right=[yaw.cos(),yaw.sin(),0.0];
        let up=[-yaw.sin()*elevation.sin(),yaw.cos()*elevation.sin(),elevation.cos()];
        let front=[-yaw.sin()*elevation.cos(),yaw.cos()*elevation.cos(),elevation.sin()];
        let span=self.scene.span.max(0.001);
        let pixels=self.width.min(self.height).max(1.0)*0.7*self.zoom/span;
        let globals=Uniforms{
            center:[self.scene.center[0],self.scene.center[1],self.scene.center[2],0.0],
            right:[right[0],right[1],right[2],2.0*pixels/self.width.max(1.0)],
            up:[up[0],up[1],up[2],2.0*pixels/self.height.max(1.0)],
            view:[front[0],front[1],front[2],0.4/span],
            wind:[self.time,self.scene.wind_strength,self.scene.wind_speed,self.scene.voxel_size],
            season:[self.season_progress.rem_euclid(4.0),
                self.scene.flower_kind,
                leaf_coverage_uniform(self.leaf_aa_mode),
                leaf_mipmap_uniform(self.leaf_mipmaps)],
            leaf_base:self.scene.leaf_base,
            bark_mapping:[self.scene.bark_repeat_m,
                if self.scene.bark_pbr_enabled {1.0} else {0.0},0.0,0.0],
        };
        queue.write_buffer(&resources.uniform,0,bytemuck::bytes_of(&globals));

        let msaa4=self.leaf_aa_mode==LeafAaMode::Msaa4 &&
            resources.pipeline_msaa4.is_some();
        let color_view=if msaa4 {
            resources.msaa4_color_view.as_ref().expect("MSAA4 color allocated")
        } else {&resources.target_view};
        let depth_view=if msaa4 {
            resources.msaa4_depth_view.as_ref().expect("MSAA4 depth allocated")
        } else {&resources.depth_view};
        let mut pass=encoder.begin_render_pass(&wgpu::RenderPassDescriptor{
            label:Some("voxel-3d-depth-pass"),
            color_attachments:&[Some(wgpu::RenderPassColorAttachment{
                view:color_view,
                resolve_target:if msaa4 {Some(&resources.target_view)} else {None},
                ops:wgpu::Operations{
                    load:wgpu::LoadOp::Clear(wgpu::Color{
                        r:0.075,g:0.091,b:0.126,a:1.0
                    }),store:wgpu::StoreOp::Store,
                },
                depth_slice:None,
            })],
            depth_stencil_attachment:Some(wgpu::RenderPassDepthStencilAttachment{
                view:depth_view,
                depth_ops:Some(wgpu::Operations {
                    load:wgpu::LoadOp::Clear(1.0),
                    store:wgpu::StoreOp::Store,
                }),
                stencil_ops:None,
            }),
            timestamp_writes:None,
            occlusion_query_set:None,
            multiview_mask:None,
        });
        if let (Some(vb),Some(group))=(&resources.vertex,&resources.bind_group) {
            pass.set_pipeline(if msaa4 {
                resources.pipeline_msaa4.as_ref().expect("MSAA4 pipeline allocated")
            } else {&resources.pipeline});
            pass.set_bind_group(0,group,&[]);
            pass.set_vertex_buffer(0,vb.slice(..));
            pass.draw(0..resources.vertex_count,0..1);
        }
        drop(pass);
        Vec::new()
    }

    fn paint(&self,_info:egui::PaintCallbackInfo,
             _pass:&mut wgpu::RenderPass<'static>,
             _resources:&egui_wgpu::CallbackResources) {
        // The actual depth-tested pass ran in prepare, before egui samples it.
    }
}

fn upload_rgba(device:&wgpu::Device,queue:&wgpu::Queue,rgba:&[u8])->wgpu::Texture {
    let side=((rgba.len()/4) as f64).sqrt() as u32;
    let side=side.max(1);
    let tex=device.create_texture(&wgpu::TextureDescriptor {
        label:Some("procedural-material-atlas"),
        size:wgpu::Extent3d{width:side,height:side,depth_or_array_layers:1},
        mip_level_count:1,sample_count:1,
        dimension:wgpu::TextureDimension::D2,
        format:wgpu::TextureFormat::Rgba8Unorm,
        usage:wgpu::TextureUsages::COPY_DST|wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats:&[],
    });
    queue.write_texture(
        wgpu::TexelCopyTextureInfo{
            texture:&tex,mip_level:0,origin:wgpu::Origin3d::ZERO,
            aspect:wgpu::TextureAspect::All,
        },
        rgba,
        wgpu::TexelCopyBufferLayout{
            offset:0,bytes_per_row:Some(side*4),rows_per_image:Some(side),
        },
        wgpu::Extent3d{width:side,height:side,depth_or_array_layers:1},
    );
    tex
}

/// CPU mip chain for the 64/128px foliage card. Level 0 is copied
/// verbatim; downsampling uses alpha-weighted RGB to avoid dark edges and
/// average alpha to retain coverage of thin vector blades.
fn leaf_mip_chain(rgba:&[u8])->Vec<(u32,Vec<u8>)> {
    let side=((rgba.len()/4) as f64).sqrt() as u32;
    assert!(side>0 && side as usize*side as usize*4==rgba.len(),
        "foliage RGBA must be a nonempty square card");
    let mut levels=vec![(side,rgba.to_vec())];
    while levels.last().expect("initial level").0>1 {
        let (size,parent)=levels.last().expect("initial level");
        let next_size=(size/2).max(1);
        let mut pixels=Vec::with_capacity(next_size as usize*next_size as usize*4);
        for y in 0..next_size {
            for x in 0..next_size {
                let mut alpha_sum=0.0f32;
                let mut sum_rgb=[0.0f32;3];
                let mut fallback_rgb=[0.0f32;3];
                for oy in 0..2 {
                    for ox in 0..2 {
                        let sx=(2*x+ox).min(size-1);
                        let sy=(2*y+oy).min(size-1);
                        let i=((sy*size+sx)*4) as usize;
                        let alpha=parent[i+3] as f32/255.0;
                        alpha_sum+=alpha;
                        for c in 0..3 {
                            sum_rgb[c]+=parent[i+c] as f32*alpha;
                            fallback_rgb[c]+=parent[i+c] as f32;
                        }
                    }
                }
                for c in 0..3 {
                    pixels.push(if alpha_sum>0.0 {
                        (sum_rgb[c]/alpha_sum).round() as u8
                    } else {
                        (fallback_rgb[c]/4.0).round() as u8
                    });
                }
                pixels.push((alpha_sum/4.0*255.0).round() as u8);
            }
        }
        levels.push((next_size,pixels));
    }
    levels
}

/// Build the mip chain once when leaf content actually changes. Switching the
/// preview sampling mode only changes a per-frame uniform.
fn upload_leaf_rgba_mips(device:&wgpu::Device,queue:&wgpu::Queue,
    rgba:&[u8])->wgpu::Texture {
    let levels=leaf_mip_chain(rgba);
    let tex=device.create_texture(&wgpu::TextureDescriptor {
        label:Some("voxel-foliage-mip-chain"),
        size:wgpu::Extent3d {
            width:levels[0].0,height:levels[0].0,depth_or_array_layers:1,
        },
        mip_level_count:levels.len() as u32,sample_count:1,
        dimension:wgpu::TextureDimension::D2,
        format:wgpu::TextureFormat::Rgba8Unorm,
        usage:wgpu::TextureUsages::COPY_DST|wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats:&[],
    });
    for (level,(size,pixels)) in levels.iter().enumerate() {
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture:&tex,mip_level:level as u32,
                origin:wgpu::Origin3d::ZERO,aspect:wgpu::TextureAspect::All,
            },
            pixels,
            wgpu::TexelCopyBufferLayout {
                offset:0,bytes_per_row:Some(size*4),rows_per_image:Some(*size),
            },
            wgpu::Extent3d{
                width:*size,height:*size,depth_or_array_layers:1,
            },
        );
    }
    tex
}

#[cfg(test)]
mod tests {
    use super::*;
    use texture_studio::foliage::{FoliageMesh,VoxelCell,Material};
    use texture_studio::VoxelShape;
    #[test]
    fn mipmap_preview_flag_changes_uniform_without_resource_uploads() {
        assert_eq!(leaf_mipmap_uniform(false),0.0);
        assert_eq!(leaf_mipmap_uniform(true),1.0);
        let mesh=FoliageMesh::from_voxels(VoxelShape::Cube,0.5,vec![
            VoxelCell{x:0,y:0,z:0,material:Material::Bark},
            VoxelCell{x:0,y:0,z:1,material:Material::Leaf},
        ]).unwrap();
        let data=vec![120u8;TEX_SIZE as usize*TEX_SIZE as usize*4];
        let palette=texture_studio::foliage_materials::SeasonalPalettes::default()
            .gpu_stops();
        let scene=Scene::from_mesh(&mesh,&data,&data,[50,135,75],palette,0.0);
        assert_eq!(scene_uploads(Some(&scene),&scene),SceneUploads {
            geometry:false,bark:false,leaf:false,palette:false, bark_roughness:false,bark_normal:false,bark_metallic:false,
        });
    }

    #[test]
    fn leaf_mip_chain_has_expected_levels_and_unchanged_base_pixels() {
        let input=(0..64*64).flat_map(|i| {
            let alpha=if i%5==0 {128} else if i%7==0 {0} else {255};
            [52u8,140u8,81u8,alpha]
        }).collect::<Vec<_>>();
        let levels=leaf_mip_chain(&input);
        assert_eq!(levels.iter().map(|(s,_)|*s).collect::<Vec<_>>(),
            vec![64,32,16,8,4,2,1]);
        assert_eq!(levels[0].1,input);
        for (size,bytes) in &levels {
            assert_eq!(bytes.len(),(*size as usize)*(*size as usize)*4);
        }
        assert_eq!(levels.last().unwrap().1.len(),4);
    }

    #[test]
    fn alpha_weighted_mips_preserve_edge_color_and_coverage() {
        let mut input=vec![0u8;4*4*4];
        for i in 0..16 {
            let at=i*4;
            input[at..at+4].copy_from_slice(if i%2==0 {
                &[200,120,40,255]
            } else {
                &[0,0,0,0]
            });
        }
        let mips=leaf_mip_chain(&input);
        for pixel in mips[1].1.chunks_exact(4) {
            assert_eq!(&pixel[0..3],&[200,120,40],
                "transparent black cannot darken foliage edges");
            assert_eq!(pixel[3],128);
        }
        let last=mips.last().unwrap();
        assert_eq!(&last.1[0..3],&[200,120,40]);
        assert_eq!(last.1[3],128);
        let constant=vec![83u8,144,61,255].repeat(64);
        let chain=leaf_mip_chain(&constant);
        assert!(chain.iter().all(|(_,data)|
            data.chunks_exact(4).all(|px|px==[83,144,61,255])));
    }

    #[test]
    fn mipmap_shader_uses_gradients_only_when_selected() {
        let source=include_str!("voxel_gpu.wgsl");
        assert!(source.contains("globals.season.w > 0.5"));
        assert!(source.contains("textureSampleGrad(foliage_tex"));
        assert!(source.contains("textureSampleLevel(foliage_tex"));
    }

    #[test]
    fn wgsl_shader_parses_and_passes_naga_validation() {
        let source=include_str!("voxel_gpu.wgsl");
        let module=naga::front::wgsl::parse_str(source)
            .unwrap_or_else(|e|panic!("WGSL parse error: {}",e.emit_to_string(source)));
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::all(),
        ).validate(&module).expect("WGSL shader validation failed");
        assert!(module.entry_points.iter().any(|e|e.name=="vs_main"));
        assert!(module.entry_points.iter().any(|e|e.name=="fs_main"));
    }

    fn cpu_coverage_threshold(x:u32,y:u32)->f32 {
        const BAYER:[u8;16]=[
            0,8,2,10,12,4,14,6,3,11,1,9,15,7,13,5,
        ];
        (BAYER[((y&3)*4+(x&3)) as usize] as f32+0.5)/16.0
    }

    #[test]
    fn ordered_coverage_has_complete_4x4_levels_with_exact_endpoints() {
        let thresholds:Vec<f32>=(0..4).flat_map(|y|
            (0..4).map(move |x|cpu_coverage_threshold(x,y))).collect();
        let mut sorted=thresholds.clone();
        sorted.sort_by(|a,b|a.total_cmp(b));
        for (index,&value) in sorted.iter().enumerate() {
            assert_eq!(value,(index as f32+0.5)/16.0);
        }
        assert_eq!((0..16).filter(|index|0.0>=thresholds[*index]).count(),0);
        assert_eq!((0..16).filter(|index|1.0>=thresholds[*index]).count(),16);
        assert_eq!((0..16).filter(|index|0.5>=thresholds[*index]).count(),8);
        for y in 0..4 {
            for x in 0..4 {
                assert_eq!(cpu_coverage_threshold(x,y),
                    cpu_coverage_threshold(x+4,y+4));
            }
        }
        // The existing shader is tested with Naga, not GPU execution;
        // keep the WGSL's Bayer sequence in sync with this CPU reference.
        let shader=include_str!("voxel_gpu.wgsl");
        assert!(shader.contains("leaf_coverage_threshold"));
        assert!(shader.contains("globals.season.z > 0.5"));
        assert!(shader.contains("if (rgba.a < 0.5 && blossom < 0.55) { discard; }"));
    }

    #[test]
    fn msaa_shader_output_preserves_fractional_alpha_and_legacy_branches() {
        let shader=include_str!("voxel_gpu.wgsl");
        assert!(shader.contains("globals.season.z > 1.5"));
        assert!(shader.contains("select(1.0,coverage,globals.season.z > 1.5)"));
        assert!(shader.contains("return vec4<f32>(rgba.rgb * input.tint.rgb, rgba.a);"));
        assert!(shader.contains("if (rgba.a < 0.5 && blossom < 0.55) { discard; }"));
    }

    #[test]
    fn gpu_scene_uploads_only_changed_resources() {
        let mesh=FoliageMesh::from_voxels(VoxelShape::Cube,0.5,vec![
            VoxelCell{x:0,y:0,z:0,material:Material::Bark},
        ]).unwrap();
        let bark=vec![40;TEX_SIZE as usize*TEX_SIZE as usize*4];
        let leaf=vec![120;TEX_SIZE as usize*TEX_SIZE as usize*4];
        let mut palette=texture_studio::foliage_materials::SeasonalPalettes::default()
            .gpu_stops();
        let baseline=Scene::from_mesh(&mesh,&bark,&leaf,[50,150,80],palette,0.0);
        assert_eq!(scene_uploads(None,&baseline),SceneUploads {
            geometry:true,bark:true,leaf:true,palette:true, bark_roughness:true,bark_normal:true,bark_metallic:false,
        });
        let identical=Scene::from_mesh(&mesh,&bark,&leaf,[50,150,80],palette,0.0);
        assert_eq!(scene_uploads(Some(&baseline),&identical),SceneUploads {
            geometry:false,bark:false,leaf:false,palette:false, bark_roughness:false,bark_normal:false,bark_metallic:false,
        });
        palette[0][0]=0.7;
        let season=Scene::from_mesh(&mesh,&bark,&leaf,[50,150,80],palette,0.0);
        assert_eq!(scene_uploads(Some(&baseline),&season),SceneUploads {
            geometry:false,bark:false,leaf:false,palette:true, bark_roughness:false,bark_normal:false,bark_metallic:false,
        });
        let mut tinted=bark.clone();
        tinted[0]=90;
        let material=Scene::from_mesh(&mesh,&tinted,&leaf,[50,150,80],palette,0.0);
        assert_eq!(scene_uploads(Some(&season),&material),SceneUploads {
            geometry:false,bark:true,leaf:false,palette:false, bark_roughness:false,bark_normal:false,bark_metallic:false,
        });
        let grown=FoliageMesh::from_voxels(VoxelShape::Cube,0.5,vec![
            VoxelCell{x:0,y:0,z:0,material:Material::Bark},
            VoxelCell{x:0,y:0,z:1,material:Material::Bark},
        ]).unwrap();
        let different_mesh=Scene::from_mesh(&grown,&tinted,&leaf,[50,150,80],palette,0.0);
        assert_eq!(scene_uploads(Some(&material),&different_mesh),SceneUploads {
            geometry:true,bark:false,leaf:false,palette:false, bark_roughness:false,bark_normal:false,bark_metallic:false,
        });
        // A wind-strength edit changes only a uniform; scene resources stay put.
        let mut wind=Scene::from_mesh(&mesh,&bark,&leaf,[50,150,80],
            texture_studio::foliage_materials::SeasonalPalettes::default().gpu_stops(),0.0);
        wind.wind_strength+=0.1;
        assert_eq!(scene_uploads(Some(&baseline),&wind),SceneUploads {
            geometry:false,bark:false,leaf:false,palette:false, bark_roughness:false,bark_normal:false,bark_metallic:false,
        });
        let mut metrics=UploadMetrics::default();
        for changes in [
            scene_uploads(None,&baseline),
            scene_uploads(Some(&baseline),&identical),
            scene_uploads(Some(&baseline),&season),
            scene_uploads(Some(&season),&material),
            scene_uploads(Some(&material),&different_mesh),
            scene_uploads(Some(&baseline),&wind),
        ] {
            metrics.record(&changes);
        }
        assert_eq!(metrics,UploadMetrics {
            vertex_buffers:2,bark_textures:2,leaf_textures:1,palette_writes:2,
            roughness_textures:1,normal_textures:1,metallic_textures:1,
        });
    }

    #[test]
    fn ordered_leaf_preview_flag_changes_uniform_only_not_scene_uploads() {
        let mesh=FoliageMesh::from_voxels(VoxelShape::Cube,0.5,vec![
            VoxelCell{x:0,y:0,z:0,material:Material::Bark},
            VoxelCell{x:0,y:0,z:1,material:Material::Leaf},
        ]).unwrap();
        let bark=vec![100;TEX_SIZE as usize*TEX_SIZE as usize*4];
        let leaves=vec![128;TEX_SIZE as usize*TEX_SIZE as usize*4];
        let palette=texture_studio::foliage_materials::SeasonalPalettes::default()
            .gpu_stops();
        let scene=Scene::from_mesh(&mesh,&bark,&leaves,[40,140,50],palette,0.0);
        // RenderScene.ordered_leaf_coverage is not part of Scene or the
        // upload key. It only goes into globals.season.z each frame.
        assert_eq!(leaf_coverage_uniform(LeafAaMode::Legacy),0.0);
        assert_eq!(leaf_coverage_uniform(LeafAaMode::Ordered),1.0);
        assert_eq!(leaf_coverage_uniform(LeafAaMode::Msaa4),2.0);
        assert_eq!(scene_uploads(Some(&scene),&scene),SceneUploads {
            geometry:false,bark:false,leaf:false,palette:false, bark_roughness:false,bark_normal:false,bark_metallic:false,
        });
    }

    #[test]
    fn vector_leaf_silhouette_edits_upload_only_leaf_texture_not_geometry_or_palette() {
        use texture_studio::foliage::PlantPreset;
        use texture_studio::foliage_materials::FoliageMaterials;
        use texture_studio::leaf_vector::LeafSilhouetteSource;
        let mesh=FoliageMesh::from_voxels(VoxelShape::Cube,0.5,vec![
            VoxelCell{x:0,y:0,z:0,material:Material::Bark},
        ]).unwrap();
        let palette=texture_studio::foliage_materials::SeasonalPalettes::default()
            .gpu_stops();
        for (species,source) in [
            (PlantPreset::Willow,LeafSilhouetteSource::VectorWillow{
                blade_width:1.1,
            }),
            (PlantPreset::EasternCottonwood,
                LeafSilhouetteSource::VectorCottonwood{blade_width:1.1}),
            (PlantPreset::FremontCottonwood,
                LeafSilhouetteSource::VectorCottonwood{blade_width:1.1}),
            (PlantPreset::BlackCottonwood,
                LeafSilhouetteSource::VectorCottonwood{blade_width:1.1}),
        ] {
            let mut materials=FoliageMaterials::for_species(species);
            let tint=[75,135,65];
            let bark=materials.bake_bark(64,[91,76,57]).unwrap();
            let original=materials.bake_leaf(64,tint).unwrap();
            let before=Scene::from_mesh(&mesh,&bark,&original,tint,palette,0.0);
            materials.leaf_silhouette=source;
            let changed=materials.bake_leaf(64,tint).unwrap();
            assert_ne!(changed,original,"{species:?} needs a different leaf texture");
            let after=Scene::from_mesh(&mesh,&bark,&changed,tint,palette,0.0);
            assert_eq!(scene_uploads(Some(&before),&after),SceneUploads {
                geometry:false,bark:false,leaf:true,palette:false, bark_roughness:false,bark_normal:false,bark_metallic:false,
            },"{species:?} should only update leaf pixels");
        }
    }

    #[test]
    fn subpixel_leaf_alpha_updates_only_gpu_leaf_texture_not_palette_or_geometry() {
        use texture_studio::foliage::PlantPreset;
        use texture_studio::foliage_materials::{FoliageMaterials,LeafAlphaSampling};
        use texture_studio::leaf_vector::LeafSilhouetteSource;
        let mesh=FoliageMesh::from_voxels(VoxelShape::Cube,0.5,vec![
            VoxelCell{x:0,y:0,z:0,material:Material::Bark},
            VoxelCell{x:0,y:0,z:1,material:Material::Leaf},
        ]).unwrap();
        let palette=texture_studio::foliage_materials::SeasonalPalettes::default()
            .gpu_stops();
        let mut mat=FoliageMaterials::for_species(PlantPreset::Willow);
        mat.leaf_silhouette=LeafSilhouetteSource::VectorWillow{blade_width:1.0};
        let bark=mat.bake_bark(64,[90,73,54]).unwrap();
        let tint=[75,135,65];
        let binary=mat.bake_leaf(64,tint).unwrap();
        let before=Scene::from_mesh(&mesh,&bark,&binary,tint,palette,0.0);
        mat.leaf_alpha_sampling=LeafAlphaSampling::Coverage2x2;
        let covered=mat.bake_leaf(64,tint).unwrap();
        assert!(covered.chunks_exact(4).any(|px|px[3]>0&&px[3]<255));
        let after=Scene::from_mesh(&mesh,&bark,&covered,tint,palette,0.0);
        assert_eq!(scene_uploads(Some(&before),&after),SceneUploads {
            geometry:false,bark:false,leaf:true,palette:false, bark_roughness:false,bark_normal:false,bark_metallic:false,
        });
    }

    #[test]
    fn gpu_vertices_keep_uv_and_leaf_wind_weights() {
        let mesh=FoliageMesh::from_voxels(VoxelShape::Cube,0.5,vec![
            VoxelCell{x:0,y:0,z:0,material:Material::Bark},
            VoxelCell{x:0,y:0,z:1,material:Material::Leaf},
        ]).unwrap();
        let rgba=vec![255;TEX_SIZE as usize*TEX_SIZE as usize*4];
        let palette=texture_studio::foliage_materials::SeasonalPalettes::default();
        let scene=Scene::from_mesh(&mesh,&rgba,&rgba,[50,150,80],palette.gpu_stops(),0.0);
        assert_eq!(scene.triangle_count(),mesh.triangle_count());
        assert!(scene.vertices.iter().any(|v|v.kind==0.0));
        assert!(scene.vertices.iter().any(|v|v.kind==1.0&&v.sway_weight==1.0));
        assert!(scene.vertices.iter().all(|v|v.uv.iter().all(|x|x.is_finite())));
        assert!(scene.span>0.0);
        assert_eq!(scene.flower_kind,0.0);
    }

    #[test]
    fn bark_meter_scale_changes_only_uniform_not_mesh_texture_or_palette_uploads() {
        let mesh=FoliageMesh::from_voxels(VoxelShape::Cube,0.5,vec![
            VoxelCell{x:0,y:0,z:0,material:Material::Bark},
        ]).unwrap();
        let bark=vec![120;TEX_SIZE as usize*TEX_SIZE as usize*4];
        let leaf=vec![90;TEX_SIZE as usize*TEX_SIZE as usize*4];
        let palette=texture_studio::foliage_materials::SeasonalPalettes::default()
            .gpu_stops();
        let legacy=Scene::from_mesh(&mesh,&bark,&leaf,[70,132,58],palette,0.0);
        let metric=Scene::from_mesh(&mesh,&bark,&leaf,[70,132,58],palette,0.0)
            .with_bark_meters_per_repeat(Some(0.42));
        assert_eq!(legacy.bark_repeat_m,0.0);
        assert_eq!(metric.bark_repeat_m,0.42);
        assert_eq!(scene_uploads(Some(&legacy),&metric),SceneUploads {
            geometry:false,bark:false,leaf:false,palette:false, bark_roughness:false,bark_normal:false,bark_metallic:false,
        },"changing only the physical repeat must be a GPU uniform update");
        let altered=Scene::from_mesh(&mesh,&bark,&leaf,[70,132,58],palette,0.0)
            .with_bark_meters_per_repeat(Some(2.0));
        assert_eq!(scene_uploads(Some(&metric),&altered),SceneUploads {
            geometry:false,bark:false,leaf:false,palette:false, bark_roughness:false,bark_normal:false,bark_metallic:false,
        });
        assert_eq!(Scene::from_mesh(&mesh,&bark,&leaf,[70,132,58],palette,0.0)
            .with_bark_meters_per_repeat(Some(f32::NAN)).bark_repeat_m,0.0);
        let shader=include_str!("voxel_gpu.wgsl");
        assert!(shader.contains("input.bark_circumference_m > 0.0"));
        assert!(shader.contains("smoothstep(0.80,1.0,input.uv.x)"));
        assert!(shader.contains("(input.uv.x-1.0) * ring_tiles"));
    }

    #[test]
    fn tangent_frame_respects_uv_orientation_and_never_emits_nan() {
        let tri=[[0.0,0.0,0.0],[1.0,0.0,0.0],[0.0,1.0,0.0]];
        let (n,t)=triangle_tangent_frame(tri,[
            [0.0,0.0],[1.0,0.0],[0.0,1.0]]);
        assert_eq!(n,[0.0,0.0,1.0]);
        assert_eq!(t,[1.0,0.0,0.0,1.0]);
        let (_,mirrored)=triangle_tangent_frame(tri,[
            [0.0,0.0],[-1.0,0.0],[0.0,1.0]]);
        assert_eq!(&mirrored[..3],&[-1.0,0.0,0.0]);
        assert_eq!(mirrored[3],-1.0);
        let (_,fallback)=triangle_tangent_frame(tri,[[0.0,0.0];3]);
        assert!(fallback.iter().all(|v|v.is_finite()));
        let (zero_n,zero_t)=triangle_tangent_frame([[1.0,1.0,1.0];3],
            [[0.0,0.0];3]);
        assert!(zero_n.iter().all(|v|v.is_finite()));
        assert!(zero_t.iter().all(|v|v.is_finite()));
        assert_eq!(zero_t[3],1.0);
    }

    #[test]
    fn enabling_pbr_uploads_only_data_maps_not_mesh_or_legacy_bark() {
        let mesh=FoliageMesh::from_voxels(VoxelShape::Cube,0.5,vec![
            VoxelCell{x:0,y:0,z:0,material:Material::Bark}
        ]).unwrap();
        let pixels=vec![77;64*64*4];
        let palette=texture_studio::foliage_materials::SeasonalPalettes::default()
            .gpu_stops();
        let legacy=Scene::from_mesh(&mesh,&pixels,&pixels,[60,140,70],
            palette,0.0);
        assert!(!legacy.bark_pbr_enabled);
        let rough=vec![90;64*64*4];
        let normal=vec![128;64*64*4];
        let pbr=Scene::from_mesh(&mesh,&pixels,&pixels,[60,140,70],
            palette,0.0).with_bark_pbr_maps(rough.clone(),normal.clone());
        assert!(pbr.bark_pbr_enabled);
        assert_eq!(scene_uploads(Some(&legacy),&pbr),SceneUploads {
            geometry:false,bark:false,leaf:false,palette:false,
            bark_roughness:true,bark_normal:true,bark_metallic:false,
        });
        let next=Scene::from_mesh(&mesh,&pixels,&pixels,[60,140,70],
            palette,0.0).with_bark_pbr_maps(
                vec![110;64*64*4],normal.clone());
        assert_eq!(scene_uploads(Some(&pbr),&next),SceneUploads {
            geometry:false,bark:false,leaf:false,palette:false,
            bark_roughness:true,bark_normal:false,bark_metallic:false,
        });
        let retiling=Scene::from_mesh(&mesh,&pixels,&pixels,[60,140,70],
            palette,0.0).with_bark_pbr_maps(rough,normal)
            .with_bark_meters_per_repeat(Some(0.35));
        assert_eq!(scene_uploads(Some(&pbr),&retiling),SceneUploads {
            geometry:false,bark:false,leaf:false,palette:false,
            bark_roughness:false,bark_normal:false,bark_metallic:false,
        });
        let shader=include_str!("voxel_gpu.wgsl");
        assert!(shader.contains("globals.bark_mapping.y > 0.5"));
        assert!(shader.contains("shade_bark_dielectric"));
        assert!(shader.contains("srgb_to_linear"));
        assert!(shader.contains("bark_roughness_tex"));
        assert!(shader.contains("bark_normal_tex"));
        assert!(shader.contains("Legacy bark and ALL foliage modes"));
    }
}
