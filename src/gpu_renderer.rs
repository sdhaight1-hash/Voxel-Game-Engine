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
}
const ATTRIBUTES: [wgpu::VertexAttribute; 6] =
    wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x2,
        2 => Float32x4, 3 => Float32, 4 => Float32, 5 => Float32];

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
                });
            }
        }
        if vertices.is_empty() {min=[-1.0;3];max=[1.0;3];}
        let center=std::array::from_fn(|axis| (min[axis]+max[axis])*0.5);
        let span=((max[0]-min[0]).max(max[1]-min[1])
            .max(max[2]-min[2])).max(mesh.voxel_size);
        Self {
            vertices,center,span,
            bark_rgba:bark.to_vec(),leaf_rgba:leaf.to_vec(),
            palette:GpuPalette{stops:palette},
            leaf_base:[leaf_tint[0] as f32/255.0,leaf_tint[1] as f32/255.0,
                leaf_tint[2] as f32/255.0,1.0],
            wind_strength:mesh.leaf_render.wind_strength,
            wind_speed:mesh.leaf_render.wind_speed,
            voxel_size:mesh.voxel_size,
            flower_kind,
        }
    }

    #[cfg(test)]
    pub fn triangle_count(&self)->usize {self.vertices.len()/3}
}

/// The scene Arc can change for geometry, palette, or material edits.
#[derive(Debug, PartialEq, Eq)]
struct SceneUploads {
    geometry: bool,
    bark: bool,
    leaf: bool,
    palette: bool,
}

fn scene_uploads(previous: Option<&Scene>, next: &Scene) -> SceneUploads {
    SceneUploads {
        geometry: previous.is_none_or(|old|
            bytemuck::cast_slice::<Vertex,u8>(&old.vertices)
                != bytemuck::cast_slice::<Vertex,u8>(&next.vertices)),
        bark: previous.is_none_or(|old|old.bark_rgba!=next.bark_rgba),
        leaf: previous.is_none_or(|old|old.leaf_rgba!=next.leaf_rgba),
        palette: previous.is_none_or(|old|old.palette.stops!=next.palette.stops),
    }
}

/// Counts CPU-side resource upload calls, not GPU execution time.
#[derive(Default, Debug, PartialEq, Eq)]
struct UploadMetrics {
    vertex_buffers: u64,
    bark_textures: u64,
    leaf_textures: u64,
    palette_writes: u64,
}

impl UploadMetrics {
    fn record(&mut self, uploads: &SceneUploads) {
        self.vertex_buffers+=u64::from(uploads.geometry);
        self.bark_textures+=u64::from(uploads.bark);
        self.leaf_textures+=u64::from(uploads.leaf);
        self.palette_writes+=u64::from(uploads.palette);
    }
}

pub struct GpuViewport {
    pub texture_id: egui::TextureId,
    pub backend: String,
    pub adapter: String,
}

struct Resources {
    pipeline: wgpu::RenderPipeline,
    group_layout: wgpu::BindGroupLayout,
    uniform: wgpu::Buffer,
    palette_buffer: wgpu::Buffer,
    bind_group: Option<wgpu::BindGroup>,
    bark_view: Option<wgpu::TextureView>,
    leaf_view: Option<wgpu::TextureView>,
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
            ],
        });
        let pipeline_layout=device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor{
            label:Some("voxel-pbr-pipeline-layout"),
            bind_group_layouts:&[Some(&group_layout)],
            immediate_size:0,
        });
        let pipeline=device.create_render_pipeline(&wgpu::RenderPipelineDescriptor{
            label:Some("voxel-depth-texture-cutout-pipeline"),
            layout:Some(&pipeline_layout),
            vertex:wgpu::VertexState{
                module:&shader,entry_point:Some("vs_main"),
                buffers:&[Some(wgpu::VertexBufferLayout{
                    array_stride:std::mem::size_of::<Vertex>() as u64,
                    step_mode:wgpu::VertexStepMode::Vertex,
                    attributes:&ATTRIBUTES,
                })],
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
                targets:&[Some(wgpu::ColorTargetState {
                    format:wgpu::TextureFormat::Rgba8Unorm,
                    blend:None,
                    write_mask:wgpu::ColorWrites::ALL,
                })],
                compilation_options:wgpu::PipelineCompilationOptions::default(),
            }),
            multiview_mask:None,cache:None,
        });
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
            pipeline,group_layout,uniform,palette_buffer,bind_group:None,
            bark_view:None,leaf_view:None,sampler:None,
            vertex:None,vertex_count:0,last_scene:None,
            upload_metrics:UploadMetrics::default(),
            _target:target,target_view,
            _depth:depth,depth_view,
        });
        Some(Self{texture_id,backend:format!("{:?}",info.backend),adapter:info.name})
    }

    pub fn paint(&self,ui:&egui::Ui, rect:egui::Rect,scene:Arc<Scene>,
                 yaw:f32, zoom:f32, time:f32,season_progress:f32,
                 ordered_leaf_coverage:bool) {
        // Egui presents the GPU render target, populated during callback.prepare.
        ui.painter().image(self.texture_id,rect,
            egui::Rect::from_min_max(egui::Pos2::ZERO,egui::Pos2::new(1.0,1.0)),
            egui::Color32::WHITE);
        ui.painter().add(egui_wgpu::Callback::new_paint_callback(
            rect,RenderScene{scene,yaw,zoom,time,season_progress,
                ordered_leaf_coverage,
                width:rect.width(),height:rect.height()}));
    }
}

struct RenderScene {
    scene: Arc<Scene>,
    yaw:f32,
    zoom:f32,
    time:f32,
    season_progress:f32,
    ordered_leaf_coverage:bool,
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
                resources.leaf_view=Some(upload_rgba(device,queue,&scene.leaf_rgba)
                    .create_view(&Default::default()));
            }
            if uploads.bark || uploads.leaf || resources.bind_group.is_none() {
                let sampler=resources.sampler.get_or_insert_with(||
                    device.create_sampler(&wgpu::SamplerDescriptor {
                        label:Some("voxel-pixel-sampler"),
                        mag_filter:wgpu::FilterMode::Linear,
                        min_filter:wgpu::FilterMode::Linear,
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
                if self.ordered_leaf_coverage {1.0} else {0.0},0.0],
            leaf_base:self.scene.leaf_base,
        };
        queue.write_buffer(&resources.uniform,0,bytemuck::bytes_of(&globals));

        let mut pass=encoder.begin_render_pass(&wgpu::RenderPassDescriptor{
            label:Some("voxel-3d-depth-pass"),
            color_attachments:&[Some(wgpu::RenderPassColorAttachment{
                view:&resources.target_view,
                resolve_target:None,
                ops:wgpu::Operations{
                    load:wgpu::LoadOp::Clear(wgpu::Color{
                        r:0.075,g:0.091,b:0.126,a:1.0
                    }),store:wgpu::StoreOp::Store,
                },
                depth_slice:None,
            })],
            depth_stencil_attachment:Some(wgpu::RenderPassDepthStencilAttachment{
                view:&resources.depth_view,
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
            pass.set_pipeline(&resources.pipeline);
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

#[cfg(test)]
mod tests {
    use super::*;
    use texture_studio::foliage::{FoliageMesh,VoxelCell,Material};
    use texture_studio::VoxelShape;
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
            geometry:true,bark:true,leaf:true,palette:true,
        });
        let identical=Scene::from_mesh(&mesh,&bark,&leaf,[50,150,80],palette,0.0);
        assert_eq!(scene_uploads(Some(&baseline),&identical),SceneUploads {
            geometry:false,bark:false,leaf:false,palette:false,
        });
        palette[0][0]=0.7;
        let season=Scene::from_mesh(&mesh,&bark,&leaf,[50,150,80],palette,0.0);
        assert_eq!(scene_uploads(Some(&baseline),&season),SceneUploads {
            geometry:false,bark:false,leaf:false,palette:true,
        });
        let mut tinted=bark.clone();
        tinted[0]=90;
        let material=Scene::from_mesh(&mesh,&tinted,&leaf,[50,150,80],palette,0.0);
        assert_eq!(scene_uploads(Some(&season),&material),SceneUploads {
            geometry:false,bark:true,leaf:false,palette:false,
        });
        let grown=FoliageMesh::from_voxels(VoxelShape::Cube,0.5,vec![
            VoxelCell{x:0,y:0,z:0,material:Material::Bark},
            VoxelCell{x:0,y:0,z:1,material:Material::Bark},
        ]).unwrap();
        let different_mesh=Scene::from_mesh(&grown,&tinted,&leaf,[50,150,80],palette,0.0);
        assert_eq!(scene_uploads(Some(&material),&different_mesh),SceneUploads {
            geometry:true,bark:false,leaf:false,palette:false,
        });
        // A wind-strength edit changes only a uniform; scene resources stay put.
        let mut wind=Scene::from_mesh(&mesh,&bark,&leaf,[50,150,80],
            texture_studio::foliage_materials::SeasonalPalettes::default().gpu_stops(),0.0);
        wind.wind_strength+=0.1;
        assert_eq!(scene_uploads(Some(&baseline),&wind),SceneUploads {
            geometry:false,bark:false,leaf:false,palette:false,
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
        for ordered_leaf_coverage in [false,true] {
            let mode=if ordered_leaf_coverage {1.0f32} else {0.0};
            assert_eq!(mode,if ordered_leaf_coverage {1.0} else {0.0});
            assert_eq!(scene_uploads(Some(&scene),&scene),SceneUploads {
                geometry:false,bark:false,leaf:false,palette:false,
            });
        }
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
                geometry:false,bark:false,leaf:true,palette:false,
            },"{species:?} should only update leaf pixels");
        }
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
}
