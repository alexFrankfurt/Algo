use std::mem;

use anyhow::Result;
use wgpu::util::DeviceExt;
use winit::{dpi::PhysicalSize, window::Window, event::WindowEvent};

use glam::{Mat4, Vec3};

use crate::engine::{Bar, AnimationInfo, TempArrayState, SortMode, MultiTempArrayState, BarState};

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct BarVertex {
    position: [f32; 3],
    normal: [f32; 3],
    uv: [f32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct FullscreenVertex {
    position: [f32; 2],
    uv: [f32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct PlatformVertex {
    position: [f32; 3],
    uv: [f32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Instance {
    offset: f32,
    height: f32,
    z: f32,
    state: u32,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Globals {
    view_proj: [[f32; 4]; 4],
    bar_width: f32,
    max_value: f32,
    focus_distance: f32,
    focus_range: f32,
}

pub struct Renderer<'a> {
    surface: wgpu::Surface<'a>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: PhysicalSize<u32>,

    // Pipelines
    bar_pipeline: wgpu::RenderPipeline,
    floor_pipeline: wgpu::RenderPipeline,
    blur_pipeline_h: wgpu::RenderPipeline,
    blur_pipeline_v: wgpu::RenderPipeline,
    tonemap_pipeline: wgpu::RenderPipeline,

    // Geometry buffers
    bar_vertex_buffer: wgpu::Buffer,
    bar_index_buffer: wgpu::Buffer,
    bar_index_count: u32,
    platform_vertex_buffer: wgpu::Buffer,
    platform_index_buffer: wgpu::Buffer,
    platform_index_count: u32,
    instance_buffer: wgpu::Buffer,
    globals_buffer: wgpu::Buffer,
    globals_bind: wgpu::BindGroup,

    fullscreen_buffer: wgpu::Buffer,

    // Floor material
    floor_bind_group: wgpu::BindGroup,

    // Textures
    scene_floor_tex: wgpu::Texture,
    scene_floor_view: wgpu::TextureView,
    scene_full_tex: wgpu::Texture,
    scene_full_view: wgpu::TextureView,
    blur_a: wgpu::Texture,
    blur_a_view: wgpu::TextureView,
    blur_b: wgpu::Texture,
    blur_b_view: wgpu::TextureView,
    depth_tex: wgpu::Texture,
    depth_view: wgpu::TextureView,
    sampler_linear: wgpu::Sampler,

    // Post bind groups
    blur_from_scene_bind: wgpu::BindGroup,
    blur_from_a_bind: wgpu::BindGroup,
    tonemap_bind: wgpu::BindGroup,

    // Bars sampling floor scene
    bar_floor_bind: wgpu::BindGroup,

    // Layouts kept for resize
    tex_samp_layout: wgpu::BindGroupLayout,
    tex_samp_double_layout: wgpu::BindGroupLayout,

    // Egui
    egui_ctx: egui::Context,
    egui_state: egui_winit::State,
    egui_renderer: egui_wgpu::Renderer,

    // Animation state
    animated_heights: Vec<f32>,
    animated_offsets: Vec<f32>,  // X position offsets for animation
    flying_bar_progress: f32,    // 0.0 to 1.0 for flying bar animation
}

impl<'a> Renderer<'a> {
    pub async fn new(window: &'a Window) -> Result<Self> {
        let size = window.inner_size();
        let instance = wgpu::Instance::default();
        let surface = instance.create_surface(window)?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .ok_or_else(|| anyhow::anyhow!("No suitable GPU adapters found"))?;

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                },
                None,
            )
            .await?;

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        // Bar geometry (Cube)
        // X: -0.5..0.5, Y: 0.0..1.0, Z: -0.5..0.5
        let bar_vertices = [
            // Front face (Z+)
            BarVertex { position: [-0.5, 0.0, 0.5], normal: [0.0, 0.0, 1.0], uv: [0.0, 0.0] },
            BarVertex { position: [ 0.5, 0.0, 0.5], normal: [0.0, 0.0, 1.0], uv: [1.0, 0.0] },
            BarVertex { position: [ 0.5, 1.0, 0.5], normal: [0.0, 0.0, 1.0], uv: [1.0, 1.0] },
            BarVertex { position: [-0.5, 1.0, 0.5], normal: [0.0, 0.0, 1.0], uv: [0.0, 1.0] },
            // Back face (Z-)
            BarVertex { position: [ 0.5, 0.0, -0.5], normal: [0.0, 0.0, -1.0], uv: [0.0, 0.0] },
            BarVertex { position: [-0.5, 0.0, -0.5], normal: [0.0, 0.0, -1.0], uv: [1.0, 0.0] },
            BarVertex { position: [-0.5, 1.0, -0.5], normal: [0.0, 0.0, -1.0], uv: [1.0, 1.0] },
            BarVertex { position: [ 0.5, 1.0, -0.5], normal: [0.0, 0.0, -1.0], uv: [0.0, 1.0] },
            // Left face (X-)
            BarVertex { position: [-0.5, 0.0, -0.5], normal: [-1.0, 0.0, 0.0], uv: [0.0, 0.0] },
            BarVertex { position: [-0.5, 0.0,  0.5], normal: [-1.0, 0.0, 0.0], uv: [1.0, 0.0] },
            BarVertex { position: [-0.5, 1.0,  0.5], normal: [-1.0, 0.0, 0.0], uv: [1.0, 1.0] },
            BarVertex { position: [-0.5, 1.0, -0.5], normal: [-1.0, 0.0, 0.0], uv: [0.0, 1.0] },
            // Right face (X+)
            BarVertex { position: [ 0.5, 0.0,  0.5], normal: [1.0, 0.0, 0.0], uv: [0.0, 0.0] },
            BarVertex { position: [ 0.5, 0.0, -0.5], normal: [1.0, 0.0, 0.0], uv: [1.0, 0.0] },
            BarVertex { position: [ 0.5, 1.0, -0.5], normal: [1.0, 0.0, 0.0], uv: [1.0, 1.0] },
            BarVertex { position: [ 0.5, 1.0,  0.5], normal: [1.0, 0.0, 0.0], uv: [0.0, 1.0] },
            // Top face (Y+)
            BarVertex { position: [-0.5, 1.0,  0.5], normal: [0.0, 1.0, 0.0], uv: [0.0, 0.0] },
            BarVertex { position: [ 0.5, 1.0,  0.5], normal: [0.0, 1.0, 0.0], uv: [1.0, 0.0] },
            BarVertex { position: [ 0.5, 1.0, -0.5], normal: [0.0, 1.0, 0.0], uv: [1.0, 1.0] },
            BarVertex { position: [-0.5, 1.0, -0.5], normal: [0.0, 1.0, 0.0], uv: [0.0, 1.0] },
            // Bottom face (Y-)
            BarVertex { position: [-0.5, 0.0, -0.5], normal: [0.0, -1.0, 0.0], uv: [0.0, 0.0] },
            BarVertex { position: [ 0.5, 0.0, -0.5], normal: [0.0, -1.0, 0.0], uv: [1.0, 0.0] },
            BarVertex { position: [ 0.5, 0.0,  0.5], normal: [0.0, -1.0, 0.0], uv: [1.0, 1.0] },
            BarVertex { position: [-0.5, 0.0,  0.5], normal: [0.0, -1.0, 0.0], uv: [0.0, 1.0] },
        ];
        let bar_indices: [u16; 36] = [
            0, 1, 2, 0, 2, 3,       // Front
            4, 5, 6, 4, 6, 7,       // Back
            8, 9, 10, 8, 10, 11,    // Left
            12, 13, 14, 12, 14, 15, // Right
            16, 17, 18, 16, 18, 19, // Top
            20, 21, 22, 20, 22, 23, // Bottom
        ];
        let bar_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("BarVertexBuffer"),
            contents: bytemuck::cast_slice(&bar_vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let bar_index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("BarIndexBuffer"),
            contents: bytemuck::cast_slice(&bar_indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        // Platform geometry (world-space quad under the bars)
        let platform_vertices = [
            PlatformVertex { position: [-6.0, 0.0, -2.0], uv: [0.0, 0.0] },
            PlatformVertex { position: [6.0, 0.0, -2.0], uv: [8.0, 0.0] },
            PlatformVertex { position: [6.0, 0.0, 4.0], uv: [8.0, 6.0] },
            PlatformVertex { position: [-6.0, 0.0, 4.0], uv: [0.0, 6.0] },
        ];
        let platform_indices: [u16; 6] = [0, 1, 2, 0, 2, 3];
        let platform_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("PlatformVertexBuffer"),
            contents: bytemuck::cast_slice(&platform_vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let platform_index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("PlatformIndexBuffer"),
            contents: bytemuck::cast_slice(&platform_indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("InstanceBuffer"),
            size: 1024 * mem::size_of::<Instance>() as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let globals_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Globals"),
            size: mem::size_of::<Globals>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let globals_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("GlobalsLayout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let globals_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("GlobalsBindGroup"),
            layout: &globals_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: globals_buffer.as_entire_binding(),
            }],
        });

        // Fullscreen quad for post/floor
        let fullscreen_vertices: [FullscreenVertex; 6] = [
            FullscreenVertex { position: [-1.0, -1.0], uv: [0.0, 1.0] },
            FullscreenVertex { position: [1.0, -1.0], uv: [1.0, 1.0] },
            FullscreenVertex { position: [1.0, 1.0], uv: [1.0, 0.0] },
            FullscreenVertex { position: [-1.0, -1.0], uv: [0.0, 1.0] },
            FullscreenVertex { position: [1.0, 1.0], uv: [1.0, 0.0] },
            FullscreenVertex { position: [-1.0, 1.0], uv: [0.0, 0.0] },
        ];
        let fullscreen_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("FullscreenBuffer"),
            contents: bytemuck::cast_slice(&fullscreen_vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        // Textures for HDR + bloom
        let hdr_format = wgpu::TextureFormat::Rgba16Float;
        // Floor-only scene
        let (scene_floor_tex, scene_floor_view) =
            create_color_target(&device, size, hdr_format, "SceneFloor");
        // Full scene (floor + bars)
        let (scene_full_tex, scene_full_view) =
            create_color_target(&device, size, hdr_format, "SceneFull");
        let half_size = PhysicalSize::new((size.width / 2).max(1), (size.height / 2).max(1));
        let (blur_a, blur_a_view) = create_color_target(&device, half_size, hdr_format, "BlurA");
        let (blur_b, blur_b_view) = create_color_target(&device, half_size, hdr_format, "BlurB");
        let depth_format = wgpu::TextureFormat::Depth32Float;
        let (depth_tex, depth_view) = create_depth_target(&device, size, depth_format, "SceneDepth");

        let sampler_linear = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("LinearSampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });

        // Post layouts
        let tex_samp_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("TexSampLayout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let tex_samp_double_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("TexSampDoubleLayout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        // Floor material layout (albedo, normal, RMA, sampler)
        let floor_material_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("FloorMaterialLayout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let bar_floor_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("BarFloorBind"),
            layout: &tex_samp_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&scene_floor_view),
            }, wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&sampler_linear),
            }],
        });

        let blur_from_scene_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("BlurFromScene"),
            layout: &tex_samp_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&scene_full_view),
            }, wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&sampler_linear),
            }],
        });

        let blur_from_a_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("BlurFromA"),
            layout: &tex_samp_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&blur_a_view),
            }, wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&sampler_linear),
            }],
        });

        let tonemap_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("TonemapBind"),
            layout: &tex_samp_double_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&scene_full_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&blur_b_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&depth_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&sampler_linear),
                },
            ],
        });

        // Shaders
        let bar_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("BarShader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("bar.wgsl").into()),
        });
        let floor_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("FloorShader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("floor.wgsl").into()),
        });
        let post_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("PostShader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("post.wgsl").into()),
        });

        // Bar pipeline (globals + optional floor sampling in shader)
        let bar_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("BarPipelineLayout"),
            bind_group_layouts: &[&globals_layout, &tex_samp_layout],
            push_constant_ranges: &[],
        });
        let bar_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("BarPipeline"),
            layout: Some(&bar_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &bar_shader,
                entry_point: "vs_main",
                buffers: &[
                    wgpu::VertexBufferLayout {
                        array_stride: mem::size_of::<BarVertex>() as u64,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x2],
                    },
                    wgpu::VertexBufferLayout {
                        array_stride: mem::size_of::<Instance>() as u64,
                        step_mode: wgpu::VertexStepMode::Instance,
                        attributes: &wgpu::vertex_attr_array![3 => Float32, 4 => Float32, 5 => Float32, 6 => Uint32],
                    },
                ],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &bar_shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: hdr_format,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: depth_format,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        // Load floor textures
        let floor_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("FloorSampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let floor_albedo_bytes = include_bytes!("../assets/floor_albedo.png");
        let floor_normal_bytes = include_bytes!("../assets/floor_normal.png");
        let floor_rma_bytes = include_bytes!("../assets/floor_rma.png");

        let floor_albedo_image = image::load_from_memory(floor_albedo_bytes)?.to_rgba8();
        let floor_normal_image = image::load_from_memory(floor_normal_bytes)?.to_rgba8();
        let floor_rma_image = image::load_from_memory(floor_rma_bytes)?.to_rgba8();

        let floor_albedo_size = wgpu::Extent3d {
            width: floor_albedo_image.width(),
            height: floor_albedo_image.height(),
            depth_or_array_layers: 1,
        };
        let floor_normal_size = wgpu::Extent3d {
            width: floor_normal_image.width(),
            height: floor_normal_image.height(),
            depth_or_array_layers: 1,
        };
        let floor_rma_size = wgpu::Extent3d {
            width: floor_rma_image.width(),
            height: floor_rma_image.height(),
            depth_or_array_layers: 1,
        };

        let floor_albedo_tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("FloorAlbedoTex"),
            size: floor_albedo_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let floor_normal_tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("FloorNormalTex"),
            size: floor_normal_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let floor_rma_tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("FloorRmaTex"),
            size: floor_rma_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &floor_albedo_tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &floor_albedo_image,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4 * floor_albedo_image.width()),
                rows_per_image: Some(floor_albedo_image.height()),
            },
            floor_albedo_size,
        );
        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &floor_normal_tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &floor_normal_image,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4 * floor_normal_image.width()),
                rows_per_image: Some(floor_normal_image.height()),
            },
            floor_normal_size,
        );
        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &floor_rma_tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &floor_rma_image,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4 * floor_rma_image.width()),
                rows_per_image: Some(floor_rma_image.height()),
            },
            floor_rma_size,
        );

        let floor_albedo_view = floor_albedo_tex.create_view(&wgpu::TextureViewDescriptor::default());
        let floor_normal_view = floor_normal_tex.create_view(&wgpu::TextureViewDescriptor::default());
        let floor_rma_view = floor_rma_tex.create_view(&wgpu::TextureViewDescriptor::default());

        let floor_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("FloorMaterialBindGroup"),
            layout: &floor_material_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&floor_albedo_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&floor_normal_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&floor_rma_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&floor_sampler),
                },
            ],
        });

        // Floor pipeline (world-space platform quad under bars)
        let floor_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("FloorPipelineLayout"),
            bind_group_layouts: &[&globals_layout, &floor_material_layout],
            push_constant_ranges: &[],
        });
        let floor_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("FloorPipeline"),
            layout: Some(&floor_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &floor_shader,
                entry_point: "vs_main",
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: mem::size_of::<PlatformVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x2],
                }],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &floor_shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: hdr_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: depth_format,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        // Post pipelines
        let blur_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("BlurPipelineLayout"),
            bind_group_layouts: &[&tex_samp_layout],
            push_constant_ranges: &[],
        });
        let blur_pipeline_h = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("BlurPipelineH"),
            layout: Some(&blur_layout),
            vertex: wgpu::VertexState {
                module: &post_shader,
                entry_point: "vs_fullscreen",
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: mem::size_of::<FullscreenVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2],
                }],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &post_shader,
                entry_point: "fs_blur_h",
                targets: &[Some(wgpu::ColorTargetState {
                    format: hdr_format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        let blur_pipeline_v = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("BlurPipelineV"),
            layout: Some(&blur_layout),
            vertex: wgpu::VertexState {
                module: &post_shader,
                entry_point: "vs_fullscreen",
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: mem::size_of::<FullscreenVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2],
                }],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &post_shader,
                entry_point: "fs_blur_v",
                targets: &[Some(wgpu::ColorTargetState {
                    format: hdr_format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        let tonemap_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("TonemapPipelineLayout"),
            bind_group_layouts: &[&tex_samp_double_layout],
            push_constant_ranges: &[],
        });
        let tonemap_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("TonemapPipeline"),
            layout: Some(&tonemap_layout),
            vertex: wgpu::VertexState {
                module: &post_shader,
                entry_point: "vs_fullscreen",
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: mem::size_of::<FullscreenVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2],
                }],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &post_shader,
                entry_point: "fs_tonemap",
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        // Egui setup
        let egui_ctx = egui::Context::default();
        let egui_state = egui_winit::State::new(
            egui_ctx.clone(),
            egui::ViewportId::ROOT,
            &window,
            Some(window.scale_factor() as f32),
            None,
        );
        let egui_renderer = egui_wgpu::Renderer::new(&device, surface_format, None, 1);

        Ok(Self {
            surface,
            device,
            queue,
            config,
            size,
            bar_pipeline,
            floor_pipeline,
            blur_pipeline_h,
            blur_pipeline_v,
            tonemap_pipeline,
            bar_vertex_buffer,
            bar_index_buffer,
            bar_index_count: bar_indices.len() as u32,
            platform_vertex_buffer,
            platform_index_buffer,
            platform_index_count: platform_indices.len() as u32,
            instance_buffer,
            globals_buffer,
            globals_bind,
            fullscreen_buffer,
            floor_bind_group,
            scene_floor_tex,
            scene_floor_view,
            scene_full_tex,
            scene_full_view,
            blur_a,
            blur_a_view,
            blur_b,
            blur_b_view,
            depth_tex,
            depth_view,
            sampler_linear,
            blur_from_scene_bind,
            blur_from_a_bind,
            tonemap_bind,
            bar_floor_bind,
            tex_samp_layout,
            tex_samp_double_layout,
            egui_ctx,
            egui_state,
            egui_renderer,
            animated_heights: Vec::new(),
            animated_offsets: Vec::new(),
            flying_bar_progress: 0.0,
        })
    }

    pub fn handle_input(&mut self, window: &Window, event: &WindowEvent) {
        let _ = self.egui_state.on_window_event(window, event);
    }

    pub fn resize(&mut self, size: PhysicalSize<u32>) {
        if size.width == 0 || size.height == 0 {
            return;
        }
        self.size = size;
        self.config.width = size.width;
        self.config.height = size.height;
        self.surface.configure(&self.device, &self.config);

        // Recreate render targets and bind groups
        let hdr_format = wgpu::TextureFormat::Rgba16Float;
        let (scene_floor_tex, scene_floor_view) =
            create_color_target(&self.device, size, hdr_format, "SceneFloor");
        let (scene_full_tex, scene_full_view) =
            create_color_target(&self.device, size, hdr_format, "SceneFull");
        let half_size = PhysicalSize::new((size.width / 2).max(1), (size.height / 2).max(1));
        let (blur_a, blur_a_view) = create_color_target(&self.device, half_size, hdr_format, "BlurA");
        let (blur_b, blur_b_view) = create_color_target(&self.device, half_size, hdr_format, "BlurB");
        let depth_format = wgpu::TextureFormat::Depth32Float;
        let (depth_tex, depth_view) = create_depth_target(&self.device, size, depth_format, "SceneDepth");
        self.scene_floor_tex = scene_floor_tex;
        self.scene_floor_view = scene_floor_view;
        self.scene_full_tex = scene_full_tex;
        self.scene_full_view = scene_full_view;
        self.blur_a = blur_a;
        self.blur_a_view = blur_a_view;
        self.blur_b = blur_b;
        self.blur_b_view = blur_b_view;
        self.depth_tex = depth_tex;
        self.depth_view = depth_view;

        // Recreate blur bind groups with new views
        self.bar_floor_bind = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("BarFloorBind"),
            layout: &self.tex_samp_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&self.scene_floor_view),
            }, wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&self.sampler_linear),
            }],
        });

        self.blur_from_scene_bind = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("BlurFromScene"),
            layout: &self.tex_samp_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&self.scene_full_view),
            }, wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&self.sampler_linear),
            }],
        });
        self.blur_from_a_bind = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("BlurFromA"),
            layout: &self.tex_samp_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&self.blur_a_view),
            }, wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&self.sampler_linear),
            }],
        });
        self.tonemap_bind = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("TonemapBind"),
            layout: &self.tex_samp_double_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&self.scene_full_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&self.blur_b_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&self.depth_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&self.sampler_linear),
                },
            ],
        });
    }

    pub fn render(&mut self, bars: &[Bar], max_value: u32, comparisons: usize, operations: usize, time_elapsed: std::time::Duration, current_memory: usize, peak_memory: usize, animation: AnimationInfo, temp_array: &TempArrayState, multi_temp_arrays: &MultiTempArrayState, mode: SortMode, merge_level: usize, dt: std::time::Duration, window: &Window) -> Result<Option<SortMode>> {
        if bars.is_empty() {
            return Ok(None);
        }

        let array_size = bars.len();
        let base_memory = array_size * 4; // 4 bytes per u32 element
        let dt_secs = dt.as_secs_f32();
        
        let mut mode_changed: Option<SortMode> = None;

        // Initialize or resize animated heights/offsets if needed
        if self.animated_heights.len() != bars.len() {
            let max_val = max_value.max(1) as f32;
            self.animated_heights = bars.iter().map(|b| b.value as f32 / max_val).collect();
            self.animated_offsets = vec![0.0; bars.len()];
        }

        // Animate heights towards targets
        let max_val = max_value.max(1) as f32;
        let animation_speed = 8.0; // How fast bars animate (higher = faster)
        for (i, bar) in bars.iter().enumerate() {
            let target = bar.value as f32 / max_val;
            let current = self.animated_heights[i];
            // Smooth exponential interpolation
            self.animated_heights[i] = current + (target - current) * (1.0 - (-animation_speed * dt_secs).exp());
        }

        // Update flying bar progress
        if animation.active {
            // Progress the flying animation
            self.flying_bar_progress = (self.flying_bar_progress + dt_secs * 3.0).min(1.0);
        } else {
            self.flying_bar_progress = 0.0;
        }

        // Egui update
        let raw_input = self.egui_state.take_egui_input(window);
        let full_output = self.egui_ctx.run(raw_input, |ctx| {
            egui::Window::new("Algorithm Stats")
                .default_pos([10.0, 10.0])
                .show(ctx, |ui| {
                    ui.heading("Merge Sort");
                    ui.separator();
                    
                    // Mode toggle
                    ui.horizontal(|ui| {
                        ui.label("Mode:");
                        if ui.radio(matches!(mode, SortMode::Sequential), "Sequential").clicked() {
                            mode_changed = Some(SortMode::Sequential);
                        }
                        if ui.radio(matches!(mode, SortMode::Parallel), "Parallel (8 threads)").clicked() {
                            mode_changed = Some(SortMode::Parallel);
                        }
                    });
                    ui.separator();
                    
                    ui.label(format!("Time Elapsed: {:.2} s", time_elapsed.as_secs_f32()));
                    ui.label(format!("Comparisons: {}", comparisons));
                    ui.label(format!("Operations: {}", operations));
                    ui.separator();
                    ui.label("Memory Usage:");
                    ui.label(format!("Array: {} elements × 4 bytes = {} bytes", array_size, base_memory));
                    ui.label(format!("Temp Arrays: {} bytes", current_memory));
                    ui.label(format!("Current Total: {} bytes", base_memory + current_memory));
                    ui.label(format!("Peak Total: {} bytes", base_memory + peak_memory));
                    ui.separator();
                    ui.label("Complexity Analysis:");
                    ui.label("Time: O(n log n)");
                    ui.label("Space: O(n)");
                    
                    if matches!(mode, SortMode::Parallel) {
                        ui.separator();
                        ui.label("Thread Colors:");
                        ui.horizontal(|ui| {
                            ui.colored_label(egui::Color32::from_rgb(255, 102, 102), "T0");
                            ui.colored_label(egui::Color32::from_rgb(255, 178, 102), "T1");
                            ui.colored_label(egui::Color32::from_rgb(255, 255, 102), "T2");
                            ui.colored_label(egui::Color32::from_rgb(102, 255, 102), "T3");
                        });
                        ui.horizontal(|ui| {
                            ui.colored_label(egui::Color32::from_rgb(102, 255, 255), "T4");
                            ui.colored_label(egui::Color32::from_rgb(102, 178, 255), "T5");
                            ui.colored_label(egui::Color32::from_rgb(178, 102, 255), "T6");
                            ui.colored_label(egui::Color32::from_rgb(255, 102, 255), "T7");
                        });
                    }
                });
        });

        let count = bars.len() as f32;
        let bar_width = 2.0 / count;
        
        // Shift main array to the left to make room for temp array
        let main_array_offset = -1.2; // Shift everything left
        let z_span = 0.6;

        let mut instances: Vec<Instance> = bars
            .iter()
            .enumerate()
            .map(|(i, bar)| {
                let t = if count > 1.0 { i as f32 / (count - 1.0) } else { 0.5 };
                let offset = main_array_offset + (-1.0 + bar_width * (i as f32 + 0.5));
                
                // In parallel mode, spread thread-active bars horizontally by thread
                let z = match mode {
                    SortMode::Sequential => (t - 0.5) * z_span,
                    SortMode::Parallel => {
                        let state = bar.state as u32;
                        // States 6-13 are ThreadActive0-7, spread them by thread
                        if state >= 6 && state <= 13 {
                            let thread_id = state - 6;
                            let thread_t = thread_id as f32 / 7.0;
                            (thread_t - 0.5) * z_span
                        } else {
                            // Normal bars keep their index-based position
                            (t - 0.5) * z_span
                        }
                    }
                };
                
                // Use animated height for smooth transitions
                let h = self.animated_heights[i].clamp(0.0, 1.0);
                Instance {
                    offset,
                    height: h,
                    z,
                    state: bar.state as u32,
                }
            })
            .collect();

        // In parallel mode, add underline bars to show thread ownership
        if matches!(mode, SortMode::Parallel) {
            let n = bars.len();
            let base_chunk_size = (n + 7) / 8; // Initial segment size for 8 threads
            
            // Segment size doubles with each merge level
            // Level 0: base_chunk_size (8 threads each with 1 chunk)
            // Level 1: base_chunk_size * 2 (4 threads each with 2 chunks merged)
            // Level 2: base_chunk_size * 4 (2 threads each with 4 chunks merged)
            // Level 3: base_chunk_size * 8 (1 thread with all chunks merged)
            let segment_size = base_chunk_size * (1 << merge_level); // 2^merge_level
            let num_segments = (n + segment_size - 1) / segment_size; // Ceiling division
            
            for i in 0..n {
                // Calculate which segment this element belongs to
                let segment_id = i / segment_size;
                // Map segment to thread color (wrap around if fewer segments than threads)
                let thread_id = segment_id.min(7);
                
                let t = if count > 1.0 { i as f32 / (count - 1.0) } else { 0.5 };
                let offset = main_array_offset + (-1.0 + bar_width * (i as f32 + 0.5));
                // Move underline in front of bars (higher Z = closer to camera)
                let z = (t - 0.5) * z_span + 0.15;
                
                // Thin underline bar with saturated thread color (states 22-29)
                instances.push(Instance {
                    offset,
                    height: 0.03, // Slightly thicker for visibility
                    z,
                    state: 22 + thread_id as u32, // Saturated underline color
                });
            }
        }

        // Add temp array visualization (bars to the right, same height as main)
        // Position temp array on the right side with gap from main array
        let temp_x_offset = main_array_offset + 1.8; // To the right of shifted main array
        let temp_bar_width = bar_width * 0.6; // Smaller width bars for temp
        let temp_spacing = 1.8; // Extra spacing multiplier to prevent overlap
        
        match mode {
            SortMode::Sequential => {
                // Single temp array in center
                let temp_z = 0.0;
                for (i, &val) in temp_array.values.iter().enumerate() {
                    let h = (val as f32 / max_val).clamp(0.0, 1.0);
                    let temp_offset = temp_x_offset + temp_bar_width * (i as f32 + 0.5) * temp_spacing;
                    instances.push(Instance {
                        offset: temp_offset,
                        height: h,
                        z: temp_z,
                        state: 5, // Purple/violet for temp array
                    });
                }
            }
            SortMode::Parallel => {
                // 8 temp arrays spread horizontally (different Z positions with X offset)
                // Arrange in a 2x4 grid or spread along Z with slight X shifts
                for (thread_id, thread_temp) in multi_temp_arrays.arrays.iter().enumerate() {
                    // Spread threads along Z axis (horizontal from camera view)
                    let thread_t = thread_id as f32 / 7.0; // 0 to 1 for 8 threads
                    let temp_z = (thread_t - 0.5) * z_span * 6.0; // Wide spread in Z (3x more)
                    
                    // Also offset X per thread to create staggered layout
                    let thread_x_offset = (thread_id as f32 - 3.5) * 0.25; // X offset per thread (3x more)
                    
                    for (i, &val) in thread_temp.values.iter().enumerate() {
                        let h = (val as f32 / max_val).clamp(0.0, 1.0);
                        let temp_offset = temp_x_offset + thread_x_offset + temp_bar_width * (i as f32 + 0.5) * temp_spacing;
                        // Use thread-specific temp color (states 14-21)
                        let state = 14 + thread_id as u32;
                        instances.push(Instance {
                            offset: temp_offset,
                            height: h,
                            z: temp_z,
                            state,
                        });
                    }
                }
            }
        }

        // Add flying bar if animation is active
        if animation.active {
            let t = self.flying_bar_progress;
            let eased_t = t * t * (3.0 - 2.0 * t); // Smoothstep
            let z_span = 0.6;
            
            // Determine temp array Z position based on mode and thread
            let (temp_z, thread_x_offset) = match mode {
                SortMode::Sequential => (0.0, 0.0),
                SortMode::Parallel => {
                    let thread_t = animation.thread_id as f32 / 7.0;
                    let z = (thread_t - 0.5) * z_span * 6.0; // Wide spread matching temp arrays (3x more)
                    let x_off = (animation.thread_id as f32 - 3.5) * 0.25; // Match temp array X offset (3x more)
                    (z, x_off)
                }
            };
            
            // Get the appropriate temp array for this thread
            let active_temp = match mode {
                SortMode::Sequential => temp_array,
                SortMode::Parallel => &multi_temp_arrays.arrays[animation.thread_id],
            };
            
            if animation.is_temp_push {
                // Flying from main array to temp array (on the right)
                let source_offset = main_array_offset + (-1.0 + bar_width * (animation.source_idx as f32 + 0.5));
                let source_t = if count > 1.0 { animation.source_idx as f32 / (count - 1.0) } else { 0.5 };
                let source_z = (source_t - 0.5) * z_span;
                
                // Target is in temp array (right side) with thread-specific offset
                let temp_count = active_temp.values.len();
                let target_temp_idx = if temp_count > 0 { temp_count - 1 } else { 0 };
                let target_offset = temp_x_offset + thread_x_offset + temp_bar_width * (target_temp_idx as f32 + 0.5) * temp_spacing;
                let target_z = temp_z;
                
                let flying_offset = source_offset + (target_offset - source_offset) * eased_t;
                let flying_z = source_z + (target_z - source_z) * eased_t;
                
                // Arc motion - higher arc for longer distance
                let arc_height = 0.3 * (1.0 - (2.0 * t - 1.0).powi(2));
                
                // Keep height consistent - same as source/target
                let h = animation.source_height.clamp(0.0, 1.0);
                
                // Use thread-specific color in parallel mode
                let state = match mode {
                    SortMode::Sequential => 4, // Cyan for flying
                    SortMode::Parallel => 6 + animation.thread_id as u32, // Thread color
                };
                
                instances.push(Instance {
                    offset: flying_offset,
                    height: h + arc_height,
                    z: flying_z,
                    state,
                });
            } else if animation.target_idx < bars.len() {
                // Flying from temp array (right side) to main array
                let target_offset = main_array_offset + (-1.0 + bar_width * (animation.target_idx as f32 + 0.5));
                let target_t = if count > 1.0 { animation.target_idx as f32 / (count - 1.0) } else { 0.5 };
                let target_z = (target_t - 0.5) * z_span;
                
                // Source is front of temp array (index 0, on the right) with thread offset
                let source_offset = temp_x_offset + thread_x_offset + temp_bar_width * 0.5 * temp_spacing;
                let source_z = temp_z;
                
                let flying_offset = source_offset + (target_offset - source_offset) * eased_t;
                let flying_z = source_z + (target_z - source_z) * eased_t;
                
                // Arc motion
                let arc_height = 0.3 * (1.0 - (2.0 * t - 1.0).powi(2));
                
                // Keep height consistent - same as source/target
                let h = animation.source_height.clamp(0.0, 1.0);
                
                // Use thread-specific color in parallel mode
                let state = match mode {
                    SortMode::Sequential => 4, // Cyan for flying
                    SortMode::Parallel => 6 + animation.thread_id as u32, // Thread color
                };
                
                instances.push(Instance {
                    offset: flying_offset,
                    height: h + arc_height,
                    z: flying_z,
                    state,
                });
            }
        }

        let required_bytes = instances.len() as u64 * mem::size_of::<Instance>() as u64;
        if required_bytes > self.instance_buffer.size() {
            self.instance_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("InstanceBufferDynamic"),
                size: required_bytes.next_power_of_two(),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }
        self.queue
            .write_buffer(&self.instance_buffer, 0, bytemuck::cast_slice(&instances));
        let aspect = self.size.width as f32 / self.size.height as f32;
        let eye = Vec3::new(0.0, 1.2, 2.6);
        let target = Vec3::new(0.0, 0.6, 0.0);
        let view = Mat4::look_at_rh(eye, target, Vec3::Y);
        let fov_y = 50f32.to_radians();
        let near = 0.1;
        let far = 10.0;
        let proj = Mat4::perspective_rh(fov_y, aspect, near, far);
        let view_proj = proj * view;

        let globals = Globals {
            view_proj: view_proj.to_cols_array_2d(),
            bar_width,
            max_value: max_val,
            focus_distance: 2.3,
            focus_range: 2.5,
        };
        self.queue
            .write_buffer(&self.globals_buffer, 0, bytemuck::bytes_of(&globals));

        // Scene pass: floor into floor texture, then bars into full texture
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("SceneEncoder") });

        // Pass A: floor only into scene_floor_view
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("FloorPass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.scene_floor_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.02, g: 0.02, b: 0.04, a: 1.0 }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            render_pass.set_pipeline(&self.floor_pipeline);
            render_pass.set_bind_group(0, &self.globals_bind, &[]);
            render_pass.set_bind_group(1, &self.floor_bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.platform_vertex_buffer.slice(..));
            render_pass.set_index_buffer(self.platform_index_buffer.slice(..), wgpu::IndexFormat::Uint16);
            render_pass.draw_indexed(0..self.platform_index_count, 0, 0..1);
        }

        encoder.copy_texture_to_texture(
            self.scene_floor_tex.as_image_copy(),
            self.scene_full_tex.as_image_copy(),
            wgpu::Extent3d {
                width: self.size.width.max(1),
                height: self.size.height.max(1),
                depth_or_array_layers: 1,
            },
        );

        // Pass B: bars into scene_full_view, sampling floor (scene_floor_view)
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("BarsPass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.scene_full_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            render_pass.set_pipeline(&self.bar_pipeline);
            render_pass.set_bind_group(0, &self.globals_bind, &[]);
            render_pass.set_bind_group(1, &self.bar_floor_bind, &[]);
            render_pass.set_vertex_buffer(0, self.bar_vertex_buffer.slice(..));
            render_pass.set_vertex_buffer(1, self.instance_buffer.slice(..));
            render_pass.set_index_buffer(self.bar_index_buffer.slice(..), wgpu::IndexFormat::Uint16);
            render_pass.draw_indexed(0..self.bar_index_count, 0, 0..instances.len() as u32);
        }

        self.queue.submit(std::iter::once(encoder.finish()));

        let output = match self.surface.get_current_texture() {
            Ok(frame) => frame,
            Err(wgpu::SurfaceError::Lost) => {
                self.resize(self.size);
                return Ok(None);
            }
            Err(wgpu::SurfaceError::OutOfMemory) => {
                return Err(anyhow::anyhow!("Surface out of memory"));
            }
            Err(err) => {
                eprintln!("Surface error: {err:?}");
                return Ok(None);
            }
        };
        let swap_view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("PostEncoder") });

        // Blur passes (downsampled)
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("BlurH"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.blur_a_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.blur_pipeline_h);
            pass.set_bind_group(0, &self.blur_from_scene_bind, &[]);
            pass.set_vertex_buffer(0, self.fullscreen_buffer.slice(..));
            pass.draw(0..6, 0..1);
        }

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("BlurV"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.blur_b_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.blur_pipeline_v);
            pass.set_bind_group(0, &self.blur_from_a_bind, &[]);
            pass.set_vertex_buffer(0, self.fullscreen_buffer.slice(..));
            pass.draw(0..6, 0..1);
        }

        // Tonemap to swapchain
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Tonemap"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &swap_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.tonemap_pipeline);
            pass.set_bind_group(0, &self.tonemap_bind, &[]);
            pass.set_vertex_buffer(0, self.fullscreen_buffer.slice(..));
            pass.draw(0..6, 0..1);
        }

        // Egui render
        let screen_descriptor = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [self.config.width, self.config.height],
            pixels_per_point: window.scale_factor() as f32,
        };

        let paint_jobs = self.egui_ctx.tessellate(full_output.shapes, full_output.pixels_per_point);
        for (id, image_delta) in &full_output.textures_delta.set {
            self.egui_renderer.update_texture(&self.device, &self.queue, *id, image_delta);
        }
        
        self.egui_renderer.update_buffers(
            &self.device,
            &self.queue,
            &mut encoder,
            &paint_jobs,
            &screen_descriptor,
        );

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("EguiPass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &swap_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });
            self.egui_renderer.render(&mut pass, &paint_jobs, &screen_descriptor);
        }
        
        for id in &full_output.textures_delta.free {
            self.egui_renderer.free_texture(id);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();
        Ok(mode_changed)
    }
}

fn create_color_target(
    device: &wgpu::Device,
    size: PhysicalSize<u32>,
    format: wgpu::TextureFormat,
    label: &str,
) -> (wgpu::Texture, wgpu::TextureView) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width: size.width.max(1),
            height: size.height.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    (texture, view)
}

fn create_depth_target(
    device: &wgpu::Device,
    size: PhysicalSize<u32>,
    format: wgpu::TextureFormat,
    label: &str,
) -> (wgpu::Texture, wgpu::TextureView) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width: size.width.max(1),
            height: size.height.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    (texture, view)
}
