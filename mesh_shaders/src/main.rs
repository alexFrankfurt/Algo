//! Mesh Shader Demo with Hardware Ray Traced Shadows
//!
//! Demonstrates mesh shaders with ray-traced shadows using VK_KHR_ray_query

use ash::{vk, Device, Entry, Instance};
use ash::ext::mesh_shader;
use ash::khr::acceleration_structure;
use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3, Vec4};
use gpu_allocator::vulkan::{Allocator, AllocatorCreateDesc, Allocation, AllocationCreateDesc, AllocationScheme};
use gpu_allocator::MemoryLocation;
use std::ffi::CStr;
use std::sync::{Arc, Mutex};
use winit::{
    event::{Event, WindowEvent, ElementState},
    event_loop::{ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    raw_window_handle::{HasDisplayHandle, HasWindowHandle},
    window::WindowBuilder,
};

const WIDTH: u32 = 1280;
const HEIGHT: u32 = 720;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct PushConstants {
    mvp: [[f32; 4]; 4],
    inv_view_proj: [[f32; 4]; 4],
    camera_pos: [f32; 4],
    light_pos: [f32; 4],
    time: f32,
    _padding: [f32; 3],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Vertex {
    pos: [f32; 3],
    _pad: f32,
}

struct Texture {
    image: vk::Image,
    view: vk::ImageView,
    allocation: Option<Allocation>,
}

struct AccelerationStructure {
    handle: vk::AccelerationStructureKHR,
    buffer: vk::Buffer,
    allocation: Option<Allocation>,
    device_address: vk::DeviceAddress,
}

struct VulkanApp {
    _entry: Entry,
    instance: Instance,
    device: Device,
    allocator: Arc<Mutex<Allocator>>,
    mesh_shader_ext: mesh_shader::Device,
    accel_struct_ext: acceleration_structure::Device,
    graphics_queue: vk::Queue,
    #[allow(dead_code)]
    queue_family_index: u32,
    surface_loader: ash::khr::surface::Instance,
    swapchain_loader: ash::khr::swapchain::Device,
    surface: vk::SurfaceKHR,
    swapchain: vk::SwapchainKHR,
    swapchain_image_views: Vec<vk::ImageView>,
    render_pass: vk::RenderPass,
    framebuffers: Vec<vk::Framebuffer>,
    descriptor_set_layout: vk::DescriptorSetLayout,
    descriptor_pool: vk::DescriptorPool,
    descriptor_set: vk::DescriptorSet,
    pipeline_layout: vk::PipelineLayout,
    pipeline: vk::Pipeline,
    // Background pipeline
    bg_descriptor_set_layout: vk::DescriptorSetLayout,
    bg_descriptor_set: vk::DescriptorSet,
    bg_pipeline_layout: vk::PipelineLayout,
    bg_pipeline: vk::Pipeline,
    cubemap_texture: Texture,
    command_pool: vk::CommandPool,
    command_buffers: Vec<vk::CommandBuffer>,
    image_available_semaphore: vk::Semaphore,
    render_finished_semaphore: vk::Semaphore,
    in_flight_fence: vk::Fence,
    extent: vk::Extent2D,
    sampler: vk::Sampler,
    albedo_texture: Texture,
    normal_texture: Texture,
    rma_texture: Texture,
    // Ray tracing resources
    blas: AccelerationStructure,
    tlas: AccelerationStructure,
    vertex_buffer: vk::Buffer,
    vertex_buffer_allocation: Option<Allocation>,
    index_buffer: vk::Buffer,
    index_buffer_allocation: Option<Allocation>,
}


impl VulkanApp {
    unsafe fn new(window: &winit::window::Window) -> Result<Self, Box<dyn std::error::Error>> {
        let entry = Entry::load()?;

        // Create instance
        let app_info = vk::ApplicationInfo::default()
            .application_name(CStr::from_bytes_with_nul(b"Mesh Shader RT Demo\0")?)
            .application_version(vk::make_api_version(0, 1, 0, 0))
            .engine_name(CStr::from_bytes_with_nul(b"No Engine\0")?)
            .engine_version(vk::make_api_version(0, 1, 0, 0))
            .api_version(vk::API_VERSION_1_3);

        let display_handle = window.display_handle().map_err(|e| format!("{e}"))?;
        let mut instance_extensions =
            ash_window::enumerate_required_extensions(display_handle.as_raw())?.to_vec();
        instance_extensions.push(ash::khr::get_physical_device_properties2::NAME.as_ptr());

        let create_info = vk::InstanceCreateInfo::default()
            .application_info(&app_info)
            .enabled_extension_names(&instance_extensions);

        let instance = entry.create_instance(&create_info, None)?;

        // Create surface
        let surface_loader = ash::khr::surface::Instance::new(&entry, &instance);
        let window_handle = window.window_handle().map_err(|e| format!("{e}"))?;
        let surface = ash_window::create_surface(
            &entry,
            &instance,
            display_handle.as_raw(),
            window_handle.as_raw(),
            None,
        )?;

        // Pick physical device with mesh shader AND ray tracing support
        let physical_devices = instance.enumerate_physical_devices()?;
        let physical_device = physical_devices
            .into_iter()
            .find(|&pd| Self::check_device_support(&instance, pd))
            .ok_or("No GPU with mesh shader and ray tracing support found")?;

        // Find queue family
        let queue_families = instance.get_physical_device_queue_family_properties(physical_device);
        let queue_family_index = queue_families
            .iter()
            .enumerate()
            .find(|(i, qf)| {
                qf.queue_flags.contains(vk::QueueFlags::GRAPHICS)
                    && surface_loader
                        .get_physical_device_surface_support(physical_device, *i as u32, surface)
                        .unwrap_or(false)
            })
            .map(|(i, _)| i as u32)
            .ok_or("No suitable queue family")?;

        // Create device with ray tracing extensions
        let queue_priorities = [1.0f32];
        let queue_create_info = vk::DeviceQueueCreateInfo::default()
            .queue_family_index(queue_family_index)
            .queue_priorities(&queue_priorities);

        let device_extensions = [
            ash::khr::swapchain::NAME.as_ptr(),
            mesh_shader::NAME.as_ptr(),
            acceleration_structure::NAME.as_ptr(),
            ash::khr::ray_query::NAME.as_ptr(),
            ash::khr::deferred_host_operations::NAME.as_ptr(),
        ];

        let mut mesh_shader_features = vk::PhysicalDeviceMeshShaderFeaturesEXT::default()
            .mesh_shader(true)
            .task_shader(true);

        let mut accel_struct_features = vk::PhysicalDeviceAccelerationStructureFeaturesKHR::default()
            .acceleration_structure(true);

        let mut ray_query_features = vk::PhysicalDeviceRayQueryFeaturesKHR::default()
            .ray_query(true);

        let mut features_12 = vk::PhysicalDeviceVulkan12Features::default()
            .buffer_device_address(true)
            .descriptor_indexing(true);

        let mut features_13 = vk::PhysicalDeviceVulkan13Features::default().maintenance4(true);

        let device_create_info = vk::DeviceCreateInfo::default()
            .queue_create_infos(std::slice::from_ref(&queue_create_info))
            .enabled_extension_names(&device_extensions)
            .push_next(&mut mesh_shader_features)
            .push_next(&mut accel_struct_features)
            .push_next(&mut ray_query_features)
            .push_next(&mut features_12)
            .push_next(&mut features_13);

        let device = instance.create_device(physical_device, &device_create_info, None)?;
        let mesh_shader_ext = mesh_shader::Device::new(&instance, &device);
        let accel_struct_ext = acceleration_structure::Device::new(&instance, &device);
        let graphics_queue = device.get_device_queue(queue_family_index, 0);

        // Create allocator
        let allocator = Allocator::new(&AllocatorCreateDesc {
            instance: instance.clone(),
            device: device.clone(),
            physical_device,
            debug_settings: Default::default(),
            buffer_device_address: true,
            allocation_sizes: Default::default(),
        })?;
        let allocator = Arc::new(Mutex::new(allocator));

        // Create swapchain
        let swapchain_loader = ash::khr::swapchain::Device::new(&instance, &device);
        let surface_caps =
            surface_loader.get_physical_device_surface_capabilities(physical_device, surface)?;
        let surface_formats =
            surface_loader.get_physical_device_surface_formats(physical_device, surface)?;

        let surface_format = surface_formats
            .iter()
            .find(|f| f.format == vk::Format::B8G8R8A8_SRGB)
            .unwrap_or(&surface_formats[0]);

        let extent = if surface_caps.current_extent.width != u32::MAX {
            surface_caps.current_extent
        } else {
            vk::Extent2D { width: WIDTH, height: HEIGHT }
        };

        let image_count = (surface_caps.min_image_count + 1).min(surface_caps.max_image_count);

        let swapchain_create_info = vk::SwapchainCreateInfoKHR::default()
            .surface(surface)
            .min_image_count(image_count)
            .image_format(surface_format.format)
            .image_color_space(surface_format.color_space)
            .image_extent(extent)
            .image_array_layers(1)
            .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
            .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
            .pre_transform(surface_caps.current_transform)
            .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
            .present_mode(vk::PresentModeKHR::FIFO)
            .clipped(true);

        let swapchain = swapchain_loader.create_swapchain(&swapchain_create_info, None)?;
        let swapchain_images = swapchain_loader.get_swapchain_images(swapchain)?;

        let swapchain_image_views: Vec<_> = swapchain_images
            .iter()
            .map(|&image| {
                let create_info = vk::ImageViewCreateInfo::default()
                    .image(image)
                    .view_type(vk::ImageViewType::TYPE_2D)
                    .format(surface_format.format)
                    .subresource_range(vk::ImageSubresourceRange {
                        aspect_mask: vk::ImageAspectFlags::COLOR,
                        base_mip_level: 0,
                        level_count: 1,
                        base_array_layer: 0,
                        layer_count: 1,
                    });
                device.create_image_view(&create_info, None).unwrap()
            })
            .collect();

        // Create render pass
        let attachment = vk::AttachmentDescription::default()
            .format(surface_format.format)
            .samples(vk::SampleCountFlags::TYPE_1)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::STORE)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .final_layout(vk::ImageLayout::PRESENT_SRC_KHR);

        let attachment_ref = vk::AttachmentReference::default()
            .attachment(0)
            .layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL);

        let subpass = vk::SubpassDescription::default()
            .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
            .color_attachments(std::slice::from_ref(&attachment_ref));

        let dependency = vk::SubpassDependency::default()
            .src_subpass(vk::SUBPASS_EXTERNAL)
            .dst_subpass(0)
            .src_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
            .dst_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
            .dst_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE);

        let render_pass_info = vk::RenderPassCreateInfo::default()
            .attachments(std::slice::from_ref(&attachment))
            .subpasses(std::slice::from_ref(&subpass))
            .dependencies(std::slice::from_ref(&dependency));

        let render_pass = device.create_render_pass(&render_pass_info, None)?;

        // Create framebuffers
        let framebuffers: Vec<_> = swapchain_image_views
            .iter()
            .map(|&view| {
                let fb_info = vk::FramebufferCreateInfo::default()
                    .render_pass(render_pass)
                    .attachments(std::slice::from_ref(&view))
                    .width(extent.width)
                    .height(extent.height)
                    .layers(1);
                device.create_framebuffer(&fb_info, None).unwrap()
            })
            .collect();

        // Create command pool
        let pool_info = vk::CommandPoolCreateInfo::default()
            .queue_family_index(queue_family_index)
            .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER);
        let command_pool = device.create_command_pool(&pool_info, None)?;

        // Create sampler
        let sampler_info = vk::SamplerCreateInfo::default()
            .mag_filter(vk::Filter::LINEAR)
            .min_filter(vk::Filter::LINEAR)
            .mipmap_mode(vk::SamplerMipmapMode::LINEAR)
            .address_mode_u(vk::SamplerAddressMode::REPEAT)
            .address_mode_v(vk::SamplerAddressMode::REPEAT)
            .address_mode_w(vk::SamplerAddressMode::REPEAT)
            .anisotropy_enable(true)
            .max_anisotropy(16.0);
        let sampler = device.create_sampler(&sampler_info, None)?;

        // Load textures
        let albedo_texture = Self::load_texture(
            &device,
            &allocator,
            command_pool,
            graphics_queue,
            "assets/floor_albedo.png",
            true,
        )?;
        let normal_texture = Self::load_texture(
            &device,
            &allocator,
            command_pool,
            graphics_queue,
            "assets/floor_normal.png",
            false,
        )?;
        let rma_texture = Self::load_texture(
            &device,
            &allocator,
            command_pool,
            graphics_queue,
            "assets/floor_rma.png",
            false,
        )?;

        let cubemap_texture = Self::load_cubemap(
            &device,
            &allocator,
            command_pool,
            graphics_queue,
            "assets/cubemap.png",
        )?;

        // Create acceleration structures for ray tracing
        let (blas, tlas, vertex_buffer, vertex_buffer_allocation, index_buffer, index_buffer_allocation) = 
            Self::create_acceleration_structures(&device, &allocator, &accel_struct_ext, command_pool, graphics_queue)?;


        // Create descriptor set layout for main pipeline (textures + TLAS)
        let bindings = [
            vk::DescriptorSetLayoutBinding::default()
                .binding(0)
                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::FRAGMENT),
            vk::DescriptorSetLayoutBinding::default()
                .binding(1)
                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::FRAGMENT),
            vk::DescriptorSetLayoutBinding::default()
                .binding(2)
                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::FRAGMENT),
            vk::DescriptorSetLayoutBinding::default()
                .binding(3)
                .descriptor_type(vk::DescriptorType::ACCELERATION_STRUCTURE_KHR)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::FRAGMENT),
        ];

        let layout_info = vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings);
        let descriptor_set_layout = device.create_descriptor_set_layout(&layout_info, None)?;

        // Create descriptor pool
        let pool_sizes = [
            vk::DescriptorPoolSize::default()
                .ty(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .descriptor_count(4),
            vk::DescriptorPoolSize::default()
                .ty(vk::DescriptorType::ACCELERATION_STRUCTURE_KHR)
                .descriptor_count(1),
        ];
        let pool_info = vk::DescriptorPoolCreateInfo::default()
            .max_sets(2)
            .pool_sizes(&pool_sizes);
        let descriptor_pool = device.create_descriptor_pool(&pool_info, None)?;

        // Allocate descriptor set
        let alloc_info = vk::DescriptorSetAllocateInfo::default()
            .descriptor_pool(descriptor_pool)
            .set_layouts(std::slice::from_ref(&descriptor_set_layout));
        let descriptor_sets = device.allocate_descriptor_sets(&alloc_info)?;
        let descriptor_set = descriptor_sets[0];

        // Update descriptor set
        let image_infos = [
            vk::DescriptorImageInfo::default()
                .sampler(sampler)
                .image_view(albedo_texture.view)
                .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL),
            vk::DescriptorImageInfo::default()
                .sampler(sampler)
                .image_view(normal_texture.view)
                .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL),
            vk::DescriptorImageInfo::default()
                .sampler(sampler)
                .image_view(rma_texture.view)
                .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL),
        ];

        let mut writes: Vec<_> = image_infos
            .iter()
            .enumerate()
            .map(|(i, info)| {
                vk::WriteDescriptorSet::default()
                    .dst_set(descriptor_set)
                    .dst_binding(i as u32)
                    .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                    .image_info(std::slice::from_ref(info))
            })
            .collect();

        // Add TLAS descriptor
        let accel_structs = [tlas.handle];
        let mut accel_write_info = vk::WriteDescriptorSetAccelerationStructureKHR::default()
            .acceleration_structures(&accel_structs);
        
        let accel_write = vk::WriteDescriptorSet::default()
            .dst_set(descriptor_set)
            .dst_binding(3)
            .descriptor_type(vk::DescriptorType::ACCELERATION_STRUCTURE_KHR)
            .descriptor_count(1)
            .push_next(&mut accel_write_info);
        writes.push(accel_write);

        device.update_descriptor_sets(&writes, &[]);

        // Create pipeline layout
        let push_constant_range = vk::PushConstantRange::default()
            .stage_flags(vk::ShaderStageFlags::MESH_EXT | vk::ShaderStageFlags::TASK_EXT | vk::ShaderStageFlags::FRAGMENT)
            .size(std::mem::size_of::<PushConstants>() as u32);

        let pipeline_layout_info = vk::PipelineLayoutCreateInfo::default()
            .set_layouts(std::slice::from_ref(&descriptor_set_layout))
            .push_constant_ranges(std::slice::from_ref(&push_constant_range));
        let pipeline_layout = device.create_pipeline_layout(&pipeline_layout_info, None)?;

        // Create pipeline
        let pipeline = Self::create_mesh_pipeline(&device, render_pass, pipeline_layout, extent)?;

        // === BACKGROUND PIPELINE SETUP ===
        let bg_bindings = [
            vk::DescriptorSetLayoutBinding::default()
                .binding(0)
                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::FRAGMENT),
        ];
        let bg_layout_info = vk::DescriptorSetLayoutCreateInfo::default().bindings(&bg_bindings);
        let bg_descriptor_set_layout = device.create_descriptor_set_layout(&bg_layout_info, None)?;

        let bg_alloc_info = vk::DescriptorSetAllocateInfo::default()
            .descriptor_pool(descriptor_pool)
            .set_layouts(std::slice::from_ref(&bg_descriptor_set_layout));
        let bg_descriptor_sets = device.allocate_descriptor_sets(&bg_alloc_info)?;
        let bg_descriptor_set = bg_descriptor_sets[0];

        let bg_image_info = vk::DescriptorImageInfo::default()
            .sampler(sampler)
            .image_view(cubemap_texture.view)
            .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);
        let bg_write = vk::WriteDescriptorSet::default()
            .dst_set(bg_descriptor_set)
            .dst_binding(0)
            .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
            .image_info(std::slice::from_ref(&bg_image_info));
        device.update_descriptor_sets(&[bg_write], &[]);

        let bg_push_constant_range = vk::PushConstantRange::default()
            .stage_flags(vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT)
            .size(std::mem::size_of::<PushConstants>() as u32);
        let bg_pipeline_layout_info = vk::PipelineLayoutCreateInfo::default()
            .set_layouts(std::slice::from_ref(&bg_descriptor_set_layout))
            .push_constant_ranges(std::slice::from_ref(&bg_push_constant_range));
        let bg_pipeline_layout = device.create_pipeline_layout(&bg_pipeline_layout_info, None)?;

        let bg_pipeline = Self::create_background_pipeline(&device, render_pass, bg_pipeline_layout, extent)?;

        // Allocate command buffers
        let alloc_info = vk::CommandBufferAllocateInfo::default()
            .command_pool(command_pool)
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(swapchain_images.len() as u32);
        let command_buffers = device.allocate_command_buffers(&alloc_info)?;

        // Create sync objects
        let semaphore_info = vk::SemaphoreCreateInfo::default();
        let fence_info = vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED);
        let image_available_semaphore = device.create_semaphore(&semaphore_info, None)?;
        let render_finished_semaphore = device.create_semaphore(&semaphore_info, None)?;
        let in_flight_fence = device.create_fence(&fence_info, None)?;

        Ok(Self {
            _entry: entry,
            instance,
            device,
            allocator,
            mesh_shader_ext,
            accel_struct_ext,
            graphics_queue,
            queue_family_index,
            surface_loader,
            swapchain_loader,
            surface,
            swapchain,
            swapchain_image_views,
            render_pass,
            framebuffers,
            descriptor_set_layout,
            descriptor_pool,
            descriptor_set,
            pipeline_layout,
            pipeline,
            bg_descriptor_set_layout,
            bg_descriptor_set,
            bg_pipeline_layout,
            bg_pipeline,
            cubemap_texture,
            command_pool,
            command_buffers,
            image_available_semaphore,
            render_finished_semaphore,
            in_flight_fence,
            extent,
            sampler,
            albedo_texture,
            normal_texture,
            rma_texture,
            blas,
            tlas,
            vertex_buffer,
            vertex_buffer_allocation,
            index_buffer,
            index_buffer_allocation,
        })
    }


    unsafe fn check_device_support(instance: &Instance, device: vk::PhysicalDevice) -> bool {
        // Check mesh shader support
        let mut mesh_shader_features = vk::PhysicalDeviceMeshShaderFeaturesEXT::default();
        let mut accel_struct_features = vk::PhysicalDeviceAccelerationStructureFeaturesKHR::default();
        let mut ray_query_features = vk::PhysicalDeviceRayQueryFeaturesKHR::default();
        let mut features2 = vk::PhysicalDeviceFeatures2::default()
            .push_next(&mut mesh_shader_features)
            .push_next(&mut accel_struct_features)
            .push_next(&mut ray_query_features);
        instance.get_physical_device_features2(device, &mut features2);
        
        mesh_shader_features.mesh_shader == vk::TRUE 
            && accel_struct_features.acceleration_structure == vk::TRUE
            && ray_query_features.ray_query == vk::TRUE
    }

    /// Create acceleration structures for the cubes
    unsafe fn create_acceleration_structures(
        device: &Device,
        allocator: &Arc<Mutex<Allocator>>,
        accel_ext: &acceleration_structure::Device,
        command_pool: vk::CommandPool,
        queue: vk::Queue,
    ) -> Result<(AccelerationStructure, AccelerationStructure, vk::Buffer, Option<Allocation>, vk::Buffer, Option<Allocation>), Box<dyn std::error::Error>> {
        // Create geometry for 8 cubes in a ring (matching mesh shader)
        let num_cubes = 8u32;
        let mut all_vertices: Vec<Vertex> = Vec::new();
        let mut all_indices: Vec<u32> = Vec::new();
        
        for cube_idx in 0..num_cubes {
            let angle = cube_idx as f32 * std::f32::consts::TAU / num_cubes as f32;
            let radius = 2.0f32;
            let offset = Vec3::new(angle.cos() * radius, 0.0, angle.sin() * radius);
            let size = 0.35f32;
            
            let base_vertex = all_vertices.len() as u32;
            
            // 8 vertices per cube
            let positions = [
                Vec3::new(-size, -size, -size),
                Vec3::new( size, -size, -size),
                Vec3::new( size,  size, -size),
                Vec3::new(-size,  size, -size),
                Vec3::new(-size, -size,  size),
                Vec3::new( size, -size,  size),
                Vec3::new( size,  size,  size),
                Vec3::new(-size,  size,  size),
            ];
            
            for pos in &positions {
                let world_pos = *pos + offset;
                all_vertices.push(Vertex { pos: world_pos.to_array(), _pad: 0.0 });
            }
            
            // 12 triangles (36 indices) per cube
            let cube_indices: [u32; 36] = [
                0, 2, 1, 0, 3, 2,  // Front
                4, 5, 6, 4, 6, 7,  // Back
                0, 4, 7, 0, 7, 3,  // Left
                1, 2, 6, 1, 6, 5,  // Right
                3, 7, 6, 3, 6, 2,  // Top
                0, 1, 5, 0, 5, 4,  // Bottom
            ];
            
            for idx in &cube_indices {
                all_indices.push(base_vertex + idx);
            }
        }

        // Create vertex buffer
        let vertex_buffer_size = (all_vertices.len() * std::mem::size_of::<Vertex>()) as vk::DeviceSize;
        let vertex_buffer_info = vk::BufferCreateInfo::default()
            .size(vertex_buffer_size)
            .usage(vk::BufferUsageFlags::ACCELERATION_STRUCTURE_BUILD_INPUT_READ_ONLY_KHR 
                | vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS
                | vk::BufferUsageFlags::TRANSFER_DST)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);
        
        let vertex_buffer = device.create_buffer(&vertex_buffer_info, None)?;
        let vertex_req = device.get_buffer_memory_requirements(vertex_buffer);
        
        let vertex_allocation = allocator.lock().unwrap().allocate(&AllocationCreateDesc {
            name: "vertex_buffer",
            requirements: vertex_req,
            location: MemoryLocation::GpuOnly,
            linear: true,
            allocation_scheme: AllocationScheme::GpuAllocatorManaged,
        })?;
        device.bind_buffer_memory(vertex_buffer, vertex_allocation.memory(), vertex_allocation.offset())?;

        // Create index buffer
        let index_buffer_size = (all_indices.len() * std::mem::size_of::<u32>()) as vk::DeviceSize;
        let index_buffer_info = vk::BufferCreateInfo::default()
            .size(index_buffer_size)
            .usage(vk::BufferUsageFlags::ACCELERATION_STRUCTURE_BUILD_INPUT_READ_ONLY_KHR 
                | vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS
                | vk::BufferUsageFlags::TRANSFER_DST)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);
        
        let index_buffer = device.create_buffer(&index_buffer_info, None)?;
        let index_req = device.get_buffer_memory_requirements(index_buffer);
        
        let index_allocation = allocator.lock().unwrap().allocate(&AllocationCreateDesc {
            name: "index_buffer",
            requirements: index_req,
            location: MemoryLocation::GpuOnly,
            linear: true,
            allocation_scheme: AllocationScheme::GpuAllocatorManaged,
        })?;
        device.bind_buffer_memory(index_buffer, index_allocation.memory(), index_allocation.offset())?;

        // Upload vertex and index data via staging buffer
        let staging_size = vertex_buffer_size + index_buffer_size;
        let staging_info = vk::BufferCreateInfo::default()
            .size(staging_size)
            .usage(vk::BufferUsageFlags::TRANSFER_SRC)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);
        
        let staging_buffer = device.create_buffer(&staging_info, None)?;
        let staging_req = device.get_buffer_memory_requirements(staging_buffer);
        
        let staging_allocation = allocator.lock().unwrap().allocate(&AllocationCreateDesc {
            name: "staging",
            requirements: staging_req,
            location: MemoryLocation::CpuToGpu,
            linear: true,
            allocation_scheme: AllocationScheme::GpuAllocatorManaged,
        })?;
        device.bind_buffer_memory(staging_buffer, staging_allocation.memory(), staging_allocation.offset())?;

        // Copy data to staging
        let mapped = staging_allocation.mapped_ptr().unwrap().as_ptr() as *mut u8;
        std::ptr::copy_nonoverlapping(
            all_vertices.as_ptr() as *const u8,
            mapped,
            vertex_buffer_size as usize,
        );
        std::ptr::copy_nonoverlapping(
            all_indices.as_ptr() as *const u8,
            mapped.add(vertex_buffer_size as usize),
            index_buffer_size as usize,
        );

        // Transfer to GPU
        let cmd_info = vk::CommandBufferAllocateInfo::default()
            .command_pool(command_pool)
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(1);
        let cmd = device.allocate_command_buffers(&cmd_info)?[0];
        device.begin_command_buffer(cmd, &vk::CommandBufferBeginInfo::default())?;

        let vertex_copy = vk::BufferCopy::default().size(vertex_buffer_size);
        device.cmd_copy_buffer(cmd, staging_buffer, vertex_buffer, &[vertex_copy]);

        let index_copy = vk::BufferCopy::default()
            .src_offset(vertex_buffer_size)
            .size(index_buffer_size);
        device.cmd_copy_buffer(cmd, staging_buffer, index_buffer, &[index_copy]);

        // Memory barrier before AS build
        let barrier = vk::MemoryBarrier::default()
            .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
            .dst_access_mask(vk::AccessFlags::ACCELERATION_STRUCTURE_READ_KHR);
        device.cmd_pipeline_barrier(
            cmd,
            vk::PipelineStageFlags::TRANSFER,
            vk::PipelineStageFlags::ACCELERATION_STRUCTURE_BUILD_KHR,
            vk::DependencyFlags::empty(),
            &[barrier],
            &[],
            &[],
        );

        device.end_command_buffer(cmd)?;
        let submit_info = vk::SubmitInfo::default().command_buffers(std::slice::from_ref(&cmd));
        device.queue_submit(queue, &[submit_info], vk::Fence::null())?;
        device.queue_wait_idle(queue)?;
        device.free_command_buffers(command_pool, &[cmd]);

        // Clean up staging
        device.destroy_buffer(staging_buffer, None);
        allocator.lock().unwrap().free(staging_allocation)?;

        // Get buffer device addresses
        let vertex_address = device.get_buffer_device_address(&vk::BufferDeviceAddressInfo::default().buffer(vertex_buffer));
        let index_address = device.get_buffer_device_address(&vk::BufferDeviceAddressInfo::default().buffer(index_buffer));

        // Build BLAS
        let geometry = vk::AccelerationStructureGeometryKHR::default()
            .geometry_type(vk::GeometryTypeKHR::TRIANGLES)
            .flags(vk::GeometryFlagsKHR::OPAQUE)
            .geometry(vk::AccelerationStructureGeometryDataKHR {
                triangles: vk::AccelerationStructureGeometryTrianglesDataKHR::default()
                    .vertex_format(vk::Format::R32G32B32_SFLOAT)
                    .vertex_data(vk::DeviceOrHostAddressConstKHR { device_address: vertex_address })
                    .vertex_stride(std::mem::size_of::<Vertex>() as vk::DeviceSize)
                    .max_vertex(all_vertices.len() as u32 - 1)
                    .index_type(vk::IndexType::UINT32)
                    .index_data(vk::DeviceOrHostAddressConstKHR { device_address: index_address }),
            });

        let build_info = vk::AccelerationStructureBuildGeometryInfoKHR::default()
            .ty(vk::AccelerationStructureTypeKHR::BOTTOM_LEVEL)
            .flags(vk::BuildAccelerationStructureFlagsKHR::PREFER_FAST_TRACE)
            .mode(vk::BuildAccelerationStructureModeKHR::BUILD)
            .geometries(std::slice::from_ref(&geometry));

        let primitive_count = (all_indices.len() / 3) as u32;
        let mut size_info = vk::AccelerationStructureBuildSizesInfoKHR::default();
        accel_ext.get_acceleration_structure_build_sizes(
            vk::AccelerationStructureBuildTypeKHR::DEVICE,
            &build_info,
            &[primitive_count],
            &mut size_info,
        );

        // Create BLAS buffer
        let blas_buffer_info = vk::BufferCreateInfo::default()
            .size(size_info.acceleration_structure_size)
            .usage(vk::BufferUsageFlags::ACCELERATION_STRUCTURE_STORAGE_KHR | vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);
        
        let blas_buffer = device.create_buffer(&blas_buffer_info, None)?;
        let blas_req = device.get_buffer_memory_requirements(blas_buffer);
        
        let blas_allocation = allocator.lock().unwrap().allocate(&AllocationCreateDesc {
            name: "blas",
            requirements: blas_req,
            location: MemoryLocation::GpuOnly,
            linear: true,
            allocation_scheme: AllocationScheme::GpuAllocatorManaged,
        })?;
        device.bind_buffer_memory(blas_buffer, blas_allocation.memory(), blas_allocation.offset())?;

        // Create BLAS
        let blas_create_info = vk::AccelerationStructureCreateInfoKHR::default()
            .buffer(blas_buffer)
            .size(size_info.acceleration_structure_size)
            .ty(vk::AccelerationStructureTypeKHR::BOTTOM_LEVEL);
        
        let blas_handle = accel_ext.create_acceleration_structure(&blas_create_info, None)?;
        let blas_address = accel_ext.get_acceleration_structure_device_address(
            &vk::AccelerationStructureDeviceAddressInfoKHR::default().acceleration_structure(blas_handle)
        );

        // Create scratch buffer for BLAS build
        let scratch_buffer_info = vk::BufferCreateInfo::default()
            .size(size_info.build_scratch_size)
            .usage(vk::BufferUsageFlags::STORAGE_BUFFER | vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);
        
        let scratch_buffer = device.create_buffer(&scratch_buffer_info, None)?;
        let scratch_req = device.get_buffer_memory_requirements(scratch_buffer);
        
        let scratch_allocation = allocator.lock().unwrap().allocate(&AllocationCreateDesc {
            name: "scratch",
            requirements: scratch_req,
            location: MemoryLocation::GpuOnly,
            linear: true,
            allocation_scheme: AllocationScheme::GpuAllocatorManaged,
        })?;
        device.bind_buffer_memory(scratch_buffer, scratch_allocation.memory(), scratch_allocation.offset())?;
        let scratch_address = device.get_buffer_device_address(&vk::BufferDeviceAddressInfo::default().buffer(scratch_buffer));

        // Build BLAS
        let cmd = device.allocate_command_buffers(&cmd_info)?[0];
        device.begin_command_buffer(cmd, &vk::CommandBufferBeginInfo::default())?;

        let build_info = vk::AccelerationStructureBuildGeometryInfoKHR::default()
            .ty(vk::AccelerationStructureTypeKHR::BOTTOM_LEVEL)
            .flags(vk::BuildAccelerationStructureFlagsKHR::PREFER_FAST_TRACE)
            .mode(vk::BuildAccelerationStructureModeKHR::BUILD)
            .dst_acceleration_structure(blas_handle)
            .geometries(std::slice::from_ref(&geometry))
            .scratch_data(vk::DeviceOrHostAddressKHR { device_address: scratch_address });

        let build_range = vk::AccelerationStructureBuildRangeInfoKHR::default()
            .primitive_count(primitive_count)
            .primitive_offset(0)
            .first_vertex(0);

        accel_ext.cmd_build_acceleration_structures(cmd, &[build_info], &[&[build_range]]);

        // Barrier between BLAS and TLAS build
        let barrier = vk::MemoryBarrier::default()
            .src_access_mask(vk::AccessFlags::ACCELERATION_STRUCTURE_WRITE_KHR)
            .dst_access_mask(vk::AccessFlags::ACCELERATION_STRUCTURE_READ_KHR);
        device.cmd_pipeline_barrier(
            cmd,
            vk::PipelineStageFlags::ACCELERATION_STRUCTURE_BUILD_KHR,
            vk::PipelineStageFlags::ACCELERATION_STRUCTURE_BUILD_KHR,
            vk::DependencyFlags::empty(),
            &[barrier],
            &[],
            &[],
        );

        device.end_command_buffer(cmd)?;
        device.queue_submit(queue, &[submit_info], vk::Fence::null())?;
        device.queue_wait_idle(queue)?;
        device.free_command_buffers(command_pool, &[cmd]);

        // Clean up scratch buffer
        device.destroy_buffer(scratch_buffer, None);
        allocator.lock().unwrap().free(scratch_allocation)?;

        // Build TLAS
        let instance = vk::AccelerationStructureInstanceKHR {
            transform: vk::TransformMatrixKHR { matrix: [
                1.0, 0.0, 0.0, 0.0,
                0.0, 1.0, 0.0, 0.0,
                0.0, 0.0, 1.0, 0.0,
            ]},
            instance_custom_index_and_mask: vk::Packed24_8::new(0, 0xFF),
            instance_shader_binding_table_record_offset_and_flags: vk::Packed24_8::new(0, vk::GeometryInstanceFlagsKHR::TRIANGLE_FACING_CULL_DISABLE.as_raw() as u8),
            acceleration_structure_reference: vk::AccelerationStructureReferenceKHR { device_handle: blas_address },
        };

        // Create instance buffer
        let instance_buffer_size = std::mem::size_of::<vk::AccelerationStructureInstanceKHR>() as vk::DeviceSize;
        let instance_buffer_info = vk::BufferCreateInfo::default()
            .size(instance_buffer_size)
            .usage(vk::BufferUsageFlags::ACCELERATION_STRUCTURE_BUILD_INPUT_READ_ONLY_KHR 
                | vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);
        
        let instance_buffer = device.create_buffer(&instance_buffer_info, None)?;
        let instance_req = device.get_buffer_memory_requirements(instance_buffer);
        
        let instance_allocation = allocator.lock().unwrap().allocate(&AllocationCreateDesc {
            name: "instance_buffer",
            requirements: instance_req,
            location: MemoryLocation::CpuToGpu,
            linear: true,
            allocation_scheme: AllocationScheme::GpuAllocatorManaged,
        })?;
        device.bind_buffer_memory(instance_buffer, instance_allocation.memory(), instance_allocation.offset())?;

        // Copy instance data
        let mapped = instance_allocation.mapped_ptr().unwrap().as_ptr() as *mut vk::AccelerationStructureInstanceKHR;
        std::ptr::copy_nonoverlapping(&instance, mapped, 1);

        let instance_address = device.get_buffer_device_address(&vk::BufferDeviceAddressInfo::default().buffer(instance_buffer));

        // TLAS geometry
        let tlas_geometry = vk::AccelerationStructureGeometryKHR::default()
            .geometry_type(vk::GeometryTypeKHR::INSTANCES)
            .flags(vk::GeometryFlagsKHR::OPAQUE)
            .geometry(vk::AccelerationStructureGeometryDataKHR {
                instances: vk::AccelerationStructureGeometryInstancesDataKHR::default()
                    .data(vk::DeviceOrHostAddressConstKHR { device_address: instance_address }),
            });

        let tlas_build_info = vk::AccelerationStructureBuildGeometryInfoKHR::default()
            .ty(vk::AccelerationStructureTypeKHR::TOP_LEVEL)
            .flags(vk::BuildAccelerationStructureFlagsKHR::PREFER_FAST_TRACE)
            .mode(vk::BuildAccelerationStructureModeKHR::BUILD)
            .geometries(std::slice::from_ref(&tlas_geometry));

        let mut tlas_size_info = vk::AccelerationStructureBuildSizesInfoKHR::default();
        accel_ext.get_acceleration_structure_build_sizes(
            vk::AccelerationStructureBuildTypeKHR::DEVICE,
            &tlas_build_info,
            &[1],
            &mut tlas_size_info,
        );

        // Create TLAS buffer
        let tlas_buffer_info = vk::BufferCreateInfo::default()
            .size(tlas_size_info.acceleration_structure_size)
            .usage(vk::BufferUsageFlags::ACCELERATION_STRUCTURE_STORAGE_KHR | vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);
        
        let tlas_buffer = device.create_buffer(&tlas_buffer_info, None)?;
        let tlas_req = device.get_buffer_memory_requirements(tlas_buffer);
        
        let tlas_allocation = allocator.lock().unwrap().allocate(&AllocationCreateDesc {
            name: "tlas",
            requirements: tlas_req,
            location: MemoryLocation::GpuOnly,
            linear: true,
            allocation_scheme: AllocationScheme::GpuAllocatorManaged,
        })?;
        device.bind_buffer_memory(tlas_buffer, tlas_allocation.memory(), tlas_allocation.offset())?;

        // Create TLAS
        let tlas_create_info = vk::AccelerationStructureCreateInfoKHR::default()
            .buffer(tlas_buffer)
            .size(tlas_size_info.acceleration_structure_size)
            .ty(vk::AccelerationStructureTypeKHR::TOP_LEVEL);
        
        let tlas_handle = accel_ext.create_acceleration_structure(&tlas_create_info, None)?;
        let tlas_address = accel_ext.get_acceleration_structure_device_address(
            &vk::AccelerationStructureDeviceAddressInfoKHR::default().acceleration_structure(tlas_handle)
        );

        // Create scratch buffer for TLAS build
        let tlas_scratch_info = vk::BufferCreateInfo::default()
            .size(tlas_size_info.build_scratch_size)
            .usage(vk::BufferUsageFlags::STORAGE_BUFFER | vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);
        
        let tlas_scratch = device.create_buffer(&tlas_scratch_info, None)?;
        let tlas_scratch_req = device.get_buffer_memory_requirements(tlas_scratch);
        
        let tlas_scratch_alloc = allocator.lock().unwrap().allocate(&AllocationCreateDesc {
            name: "tlas_scratch",
            requirements: tlas_scratch_req,
            location: MemoryLocation::GpuOnly,
            linear: true,
            allocation_scheme: AllocationScheme::GpuAllocatorManaged,
        })?;
        device.bind_buffer_memory(tlas_scratch, tlas_scratch_alloc.memory(), tlas_scratch_alloc.offset())?;
        let tlas_scratch_address = device.get_buffer_device_address(&vk::BufferDeviceAddressInfo::default().buffer(tlas_scratch));

        // Build TLAS
        let cmd = device.allocate_command_buffers(&cmd_info)?[0];
        device.begin_command_buffer(cmd, &vk::CommandBufferBeginInfo::default())?;

        let tlas_build_info = vk::AccelerationStructureBuildGeometryInfoKHR::default()
            .ty(vk::AccelerationStructureTypeKHR::TOP_LEVEL)
            .flags(vk::BuildAccelerationStructureFlagsKHR::PREFER_FAST_TRACE)
            .mode(vk::BuildAccelerationStructureModeKHR::BUILD)
            .dst_acceleration_structure(tlas_handle)
            .geometries(std::slice::from_ref(&tlas_geometry))
            .scratch_data(vk::DeviceOrHostAddressKHR { device_address: tlas_scratch_address });

        let tlas_build_range = vk::AccelerationStructureBuildRangeInfoKHR::default()
            .primitive_count(1)
            .primitive_offset(0)
            .first_vertex(0);

        accel_ext.cmd_build_acceleration_structures(cmd, &[tlas_build_info], &[&[tlas_build_range]]);

        device.end_command_buffer(cmd)?;
        device.queue_submit(queue, &[submit_info], vk::Fence::null())?;
        device.queue_wait_idle(queue)?;
        device.free_command_buffers(command_pool, &[cmd]);

        // Clean up
        device.destroy_buffer(tlas_scratch, None);
        allocator.lock().unwrap().free(tlas_scratch_alloc)?;
        device.destroy_buffer(instance_buffer, None);
        allocator.lock().unwrap().free(instance_allocation)?;

        let blas = AccelerationStructure {
            handle: blas_handle,
            buffer: blas_buffer,
            allocation: Some(blas_allocation),
            device_address: blas_address,
        };

        let tlas = AccelerationStructure {
            handle: tlas_handle,
            buffer: tlas_buffer,
            allocation: Some(tlas_allocation),
            device_address: tlas_address,
        };

        Ok((blas, tlas, vertex_buffer, Some(vertex_allocation), index_buffer, Some(index_allocation)))
    }


    unsafe fn load_texture(
        device: &Device,
        allocator: &Arc<Mutex<Allocator>>,
        command_pool: vk::CommandPool,
        queue: vk::Queue,
        path: &str,
        srgb: bool,
    ) -> Result<Texture, Box<dyn std::error::Error>> {
        let img = image::open(path)?.to_rgba8();
        let (width, height) = img.dimensions();
        let pixels = img.into_raw();

        let format = if srgb {
            vk::Format::R8G8B8A8_SRGB
        } else {
            vk::Format::R8G8B8A8_UNORM
        };

        let image_info = vk::ImageCreateInfo::default()
            .image_type(vk::ImageType::TYPE_2D)
            .format(format)
            .extent(vk::Extent3D { width, height, depth: 1 })
            .mip_levels(1)
            .array_layers(1)
            .samples(vk::SampleCountFlags::TYPE_1)
            .tiling(vk::ImageTiling::OPTIMAL)
            .usage(vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_DST)
            .sharing_mode(vk::SharingMode::EXCLUSIVE)
            .initial_layout(vk::ImageLayout::UNDEFINED);

        let image = device.create_image(&image_info, None)?;
        let mem_req = device.get_image_memory_requirements(image);

        let allocation = allocator.lock().unwrap().allocate(&AllocationCreateDesc {
            name: path,
            requirements: mem_req,
            location: MemoryLocation::GpuOnly,
            linear: false,
            allocation_scheme: AllocationScheme::GpuAllocatorManaged,
        })?;

        device.bind_image_memory(image, allocation.memory(), allocation.offset())?;

        let buffer_size = (width * height * 4) as vk::DeviceSize;
        let buffer_info = vk::BufferCreateInfo::default()
            .size(buffer_size)
            .usage(vk::BufferUsageFlags::TRANSFER_SRC)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);

        let staging_buffer = device.create_buffer(&buffer_info, None)?;
        let staging_req = device.get_buffer_memory_requirements(staging_buffer);

        let staging_alloc = allocator.lock().unwrap().allocate(&AllocationCreateDesc {
            name: "staging",
            requirements: staging_req,
            location: MemoryLocation::CpuToGpu,
            linear: true,
            allocation_scheme: AllocationScheme::GpuAllocatorManaged,
        })?;

        device.bind_buffer_memory(staging_buffer, staging_alloc.memory(), staging_alloc.offset())?;

        let mapped = staging_alloc.mapped_ptr().unwrap().as_ptr() as *mut u8;
        std::ptr::copy_nonoverlapping(pixels.as_ptr(), mapped, pixels.len());

        let cmd_info = vk::CommandBufferAllocateInfo::default()
            .command_pool(command_pool)
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(1);
        let cmd = device.allocate_command_buffers(&cmd_info)?[0];

        device.begin_command_buffer(cmd, &vk::CommandBufferBeginInfo::default())?;

        let barrier = vk::ImageMemoryBarrier::default()
            .image(image)
            .old_layout(vk::ImageLayout::UNDEFINED)
            .new_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
            .src_access_mask(vk::AccessFlags::empty())
            .dst_access_mask(vk::AccessFlags::TRANSFER_WRITE)
            .subresource_range(vk::ImageSubresourceRange {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                base_mip_level: 0,
                level_count: 1,
                base_array_layer: 0,
                layer_count: 1,
            });

        device.cmd_pipeline_barrier(
            cmd,
            vk::PipelineStageFlags::TOP_OF_PIPE,
            vk::PipelineStageFlags::TRANSFER,
            vk::DependencyFlags::empty(),
            &[],
            &[],
            &[barrier],
        );

        let region = vk::BufferImageCopy::default()
            .image_subresource(vk::ImageSubresourceLayers {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                mip_level: 0,
                base_array_layer: 0,
                layer_count: 1,
            })
            .image_extent(vk::Extent3D { width, height, depth: 1 });

        device.cmd_copy_buffer_to_image(
            cmd,
            staging_buffer,
            image,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            &[region],
        );

        let barrier = vk::ImageMemoryBarrier::default()
            .image(image)
            .old_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
            .new_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
            .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
            .dst_access_mask(vk::AccessFlags::SHADER_READ)
            .subresource_range(vk::ImageSubresourceRange {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                base_mip_level: 0,
                level_count: 1,
                base_array_layer: 0,
                layer_count: 1,
            });

        device.cmd_pipeline_barrier(
            cmd,
            vk::PipelineStageFlags::TRANSFER,
            vk::PipelineStageFlags::FRAGMENT_SHADER,
            vk::DependencyFlags::empty(),
            &[],
            &[],
            &[barrier],
        );

        device.end_command_buffer(cmd)?;

        let submit_info = vk::SubmitInfo::default().command_buffers(std::slice::from_ref(&cmd));
        device.queue_submit(queue, &[submit_info], vk::Fence::null())?;
        device.queue_wait_idle(queue)?;

        device.free_command_buffers(command_pool, &[cmd]);
        device.destroy_buffer(staging_buffer, None);
        allocator.lock().unwrap().free(staging_alloc)?;

        let view_info = vk::ImageViewCreateInfo::default()
            .image(image)
            .view_type(vk::ImageViewType::TYPE_2D)
            .format(format)
            .subresource_range(vk::ImageSubresourceRange {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                base_mip_level: 0,
                level_count: 1,
                base_array_layer: 0,
                layer_count: 1,
            });
        let view = device.create_image_view(&view_info, None)?;

        Ok(Texture {
            image,
            view,
            allocation: Some(allocation),
        })
    }

    fn convert_cross_to_cubemap(src: &image::RgbaImage) -> (Vec<Vec<u8>>, u32) {
        let (src_width, src_height) = src.dimensions();
        let face_w = src_width / 4;
        let face_h = src_height / 3;
        let face_size = face_w.min(face_h);
        
        let mut faces: Vec<Vec<u8>> = vec![vec![0u8; (face_size * face_size * 4) as usize]; 6];
        
        let face_configs: [(u32, u32, bool, bool); 6] = [
            (2, 1, false, false),
            (0, 1, false, false),
            (1, 0, false, false),
            (1, 2, false, false),
            (1, 1, false, false),
            (3, 1, false, false),
        ];
        
        for (face_idx, (col, row, flip_x, flip_y)) in face_configs.iter().enumerate() {
            let src_x_offset = col * face_w;
            let src_y_offset = row * face_h;
            
            for y in 0..face_size {
                for x in 0..face_size {
                    let sample_x = if *flip_x { face_size - 1 - x } else { x };
                    let sample_y = if *flip_y { face_size - 1 - y } else { y };
                    
                    let src_x = (src_x_offset + sample_x).min(src_width - 1);
                    let src_y = (src_y_offset + sample_y).min(src_height - 1);
                    
                    let pixel = src.get_pixel(src_x, src_y);
                    
                    let idx = ((y * face_size + x) * 4) as usize;
                    faces[face_idx][idx] = pixel[0];
                    faces[face_idx][idx + 1] = pixel[1];
                    faces[face_idx][idx + 2] = pixel[2];
                    faces[face_idx][idx + 3] = pixel[3];
                }
            }
        }
        
        (faces, face_size)
    }

    unsafe fn load_cubemap(
        device: &Device,
        allocator: &Arc<Mutex<Allocator>>,
        command_pool: vk::CommandPool,
        queue: vk::Queue,
        path: &str,
    ) -> Result<Texture, Box<dyn std::error::Error>> {
        let img = image::open(path)?.to_rgba8();
        let (faces, face_size) = Self::convert_cross_to_cubemap(&img);
        
        let format = vk::Format::R8G8B8A8_SRGB;
        
        let image_info = vk::ImageCreateInfo::default()
            .image_type(vk::ImageType::TYPE_2D)
            .format(format)
            .extent(vk::Extent3D { width: face_size, height: face_size, depth: 1 })
            .mip_levels(1)
            .array_layers(6)
            .samples(vk::SampleCountFlags::TYPE_1)
            .tiling(vk::ImageTiling::OPTIMAL)
            .usage(vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_DST)
            .sharing_mode(vk::SharingMode::EXCLUSIVE)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .flags(vk::ImageCreateFlags::CUBE_COMPATIBLE);
        
        let image = device.create_image(&image_info, None)?;
        let mem_req = device.get_image_memory_requirements(image);
        
        let allocation = allocator.lock().unwrap().allocate(&AllocationCreateDesc {
            name: "cubemap",
            requirements: mem_req,
            location: MemoryLocation::GpuOnly,
            linear: false,
            allocation_scheme: AllocationScheme::GpuAllocatorManaged,
        })?;
        
        device.bind_image_memory(image, allocation.memory(), allocation.offset())?;
        
        let face_bytes = (face_size * face_size * 4) as vk::DeviceSize;
        let buffer_size = face_bytes * 6;
        let buffer_info = vk::BufferCreateInfo::default()
            .size(buffer_size)
            .usage(vk::BufferUsageFlags::TRANSFER_SRC)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);
        
        let staging_buffer = device.create_buffer(&buffer_info, None)?;
        let staging_req = device.get_buffer_memory_requirements(staging_buffer);
        
        let staging_alloc = allocator.lock().unwrap().allocate(&AllocationCreateDesc {
            name: "cubemap_staging",
            requirements: staging_req,
            location: MemoryLocation::CpuToGpu,
            linear: true,
            allocation_scheme: AllocationScheme::GpuAllocatorManaged,
        })?;
        
        device.bind_buffer_memory(staging_buffer, staging_alloc.memory(), staging_alloc.offset())?;
        
        let mapped = staging_alloc.mapped_ptr().unwrap().as_ptr() as *mut u8;
        for (i, face_data) in faces.iter().enumerate() {
            let offset = i * face_data.len();
            std::ptr::copy_nonoverlapping(face_data.as_ptr(), mapped.add(offset), face_data.len());
        }
        
        let cmd_info = vk::CommandBufferAllocateInfo::default()
            .command_pool(command_pool)
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(1);
        let cmd = device.allocate_command_buffers(&cmd_info)?[0];
        
        device.begin_command_buffer(cmd, &vk::CommandBufferBeginInfo::default())?;
        
        let barrier = vk::ImageMemoryBarrier::default()
            .image(image)
            .old_layout(vk::ImageLayout::UNDEFINED)
            .new_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
            .src_access_mask(vk::AccessFlags::empty())
            .dst_access_mask(vk::AccessFlags::TRANSFER_WRITE)
            .subresource_range(vk::ImageSubresourceRange {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                base_mip_level: 0,
                level_count: 1,
                base_array_layer: 0,
                layer_count: 6,
            });
        
        device.cmd_pipeline_barrier(
            cmd,
            vk::PipelineStageFlags::TOP_OF_PIPE,
            vk::PipelineStageFlags::TRANSFER,
            vk::DependencyFlags::empty(),
            &[],
            &[],
            &[barrier],
        );
        
        let mut regions = Vec::with_capacity(6);
        for face in 0..6u32 {
            regions.push(vk::BufferImageCopy::default()
                .buffer_offset(face as vk::DeviceSize * face_bytes)
                .image_subresource(vk::ImageSubresourceLayers {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    mip_level: 0,
                    base_array_layer: face,
                    layer_count: 1,
                })
                .image_extent(vk::Extent3D { width: face_size, height: face_size, depth: 1 }));
        }
        
        device.cmd_copy_buffer_to_image(
            cmd,
            staging_buffer,
            image,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            &regions,
        );
        
        let barrier = vk::ImageMemoryBarrier::default()
            .image(image)
            .old_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
            .new_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
            .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
            .dst_access_mask(vk::AccessFlags::SHADER_READ)
            .subresource_range(vk::ImageSubresourceRange {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                base_mip_level: 0,
                level_count: 1,
                base_array_layer: 0,
                layer_count: 6,
            });
        
        device.cmd_pipeline_barrier(
            cmd,
            vk::PipelineStageFlags::TRANSFER,
            vk::PipelineStageFlags::FRAGMENT_SHADER,
            vk::DependencyFlags::empty(),
            &[],
            &[],
            &[barrier],
        );
        
        device.end_command_buffer(cmd)?;
        
        let submit_info = vk::SubmitInfo::default().command_buffers(std::slice::from_ref(&cmd));
        device.queue_submit(queue, &[submit_info], vk::Fence::null())?;
        device.queue_wait_idle(queue)?;
        
        device.free_command_buffers(command_pool, &[cmd]);
        device.destroy_buffer(staging_buffer, None);
        allocator.lock().unwrap().free(staging_alloc)?;
        
        let view_info = vk::ImageViewCreateInfo::default()
            .image(image)
            .view_type(vk::ImageViewType::CUBE)
            .format(format)
            .subresource_range(vk::ImageSubresourceRange {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                base_mip_level: 0,
                level_count: 1,
                base_array_layer: 0,
                layer_count: 6,
            });
        let view = device.create_image_view(&view_info, None)?;
        
        Ok(Texture {
            image,
            view,
            allocation: Some(allocation),
        })
    }


    unsafe fn create_mesh_pipeline(
        device: &Device,
        render_pass: vk::RenderPass,
        layout: vk::PipelineLayout,
        extent: vk::Extent2D,
    ) -> Result<vk::Pipeline, Box<dyn std::error::Error>> {
        let task_code = include_bytes!("shaders/task.spv");
        let mesh_code = include_bytes!("shaders/mesh.spv");
        let frag_code = include_bytes!("shaders/frag.spv");

        let task_module = Self::create_shader_module(device, task_code)?;
        let mesh_module = Self::create_shader_module(device, mesh_code)?;
        let frag_module = Self::create_shader_module(device, frag_code)?;

        let entry_point = CStr::from_bytes_with_nul(b"main\0")?;

        let shader_stages = [
            vk::PipelineShaderStageCreateInfo::default()
                .stage(vk::ShaderStageFlags::TASK_EXT)
                .module(task_module)
                .name(entry_point),
            vk::PipelineShaderStageCreateInfo::default()
                .stage(vk::ShaderStageFlags::MESH_EXT)
                .module(mesh_module)
                .name(entry_point),
            vk::PipelineShaderStageCreateInfo::default()
                .stage(vk::ShaderStageFlags::FRAGMENT)
                .module(frag_module)
                .name(entry_point),
        ];

        let viewport = vk::Viewport {
            x: 0.0,
            y: 0.0,
            width: extent.width as f32,
            height: extent.height as f32,
            min_depth: 0.0,
            max_depth: 1.0,
        };

        let scissor = vk::Rect2D {
            offset: vk::Offset2D { x: 0, y: 0 },
            extent,
        };

        let viewport_state = vk::PipelineViewportStateCreateInfo::default()
            .viewports(std::slice::from_ref(&viewport))
            .scissors(std::slice::from_ref(&scissor));

        let rasterizer = vk::PipelineRasterizationStateCreateInfo::default()
            .polygon_mode(vk::PolygonMode::FILL)
            .line_width(1.0)
            .cull_mode(vk::CullModeFlags::NONE)
            .front_face(vk::FrontFace::COUNTER_CLOCKWISE);

        let multisampling = vk::PipelineMultisampleStateCreateInfo::default()
            .rasterization_samples(vk::SampleCountFlags::TYPE_1);

        let color_blend_attachment = vk::PipelineColorBlendAttachmentState::default()
            .color_write_mask(vk::ColorComponentFlags::RGBA)
            .blend_enable(true)
            .src_color_blend_factor(vk::BlendFactor::SRC_ALPHA)
            .dst_color_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
            .color_blend_op(vk::BlendOp::ADD)
            .src_alpha_blend_factor(vk::BlendFactor::ONE)
            .dst_alpha_blend_factor(vk::BlendFactor::ZERO)
            .alpha_blend_op(vk::BlendOp::ADD);

        let color_blending = vk::PipelineColorBlendStateCreateInfo::default()
            .attachments(std::slice::from_ref(&color_blend_attachment));

        let pipeline_info = vk::GraphicsPipelineCreateInfo::default()
            .stages(&shader_stages)
            .viewport_state(&viewport_state)
            .rasterization_state(&rasterizer)
            .multisample_state(&multisampling)
            .color_blend_state(&color_blending)
            .layout(layout)
            .render_pass(render_pass)
            .subpass(0);

        let pipelines = device
            .create_graphics_pipelines(vk::PipelineCache::null(), &[pipeline_info], None)
            .map_err(|e| format!("Pipeline creation failed: {:?}", e.1))?;

        device.destroy_shader_module(task_module, None);
        device.destroy_shader_module(mesh_module, None);
        device.destroy_shader_module(frag_module, None);

        Ok(pipelines[0])
    }

    unsafe fn create_background_pipeline(
        device: &Device,
        render_pass: vk::RenderPass,
        layout: vk::PipelineLayout,
        extent: vk::Extent2D,
    ) -> Result<vk::Pipeline, Box<dyn std::error::Error>> {
        let vert_code = include_bytes!("shaders/background.vert.spv");
        let frag_code = include_bytes!("shaders/background.frag.spv");

        let vert_module = Self::create_shader_module(device, vert_code)?;
        let frag_module = Self::create_shader_module(device, frag_code)?;

        let entry_point = CStr::from_bytes_with_nul(b"main\0")?;

        let shader_stages = [
            vk::PipelineShaderStageCreateInfo::default()
                .stage(vk::ShaderStageFlags::VERTEX)
                .module(vert_module)
                .name(entry_point),
            vk::PipelineShaderStageCreateInfo::default()
                .stage(vk::ShaderStageFlags::FRAGMENT)
                .module(frag_module)
                .name(entry_point),
        ];

        let vertex_input = vk::PipelineVertexInputStateCreateInfo::default();
        let input_assembly = vk::PipelineInputAssemblyStateCreateInfo::default()
            .topology(vk::PrimitiveTopology::TRIANGLE_LIST);

        let viewport = vk::Viewport {
            x: 0.0,
            y: 0.0,
            width: extent.width as f32,
            height: extent.height as f32,
            min_depth: 0.0,
            max_depth: 1.0,
        };

        let scissor = vk::Rect2D {
            offset: vk::Offset2D { x: 0, y: 0 },
            extent,
        };

        let viewport_state = vk::PipelineViewportStateCreateInfo::default()
            .viewports(std::slice::from_ref(&viewport))
            .scissors(std::slice::from_ref(&scissor));

        let rasterizer = vk::PipelineRasterizationStateCreateInfo::default()
            .polygon_mode(vk::PolygonMode::FILL)
            .line_width(1.0)
            .cull_mode(vk::CullModeFlags::NONE)
            .front_face(vk::FrontFace::COUNTER_CLOCKWISE);

        let multisampling = vk::PipelineMultisampleStateCreateInfo::default()
            .rasterization_samples(vk::SampleCountFlags::TYPE_1);

        let color_blend_attachment = vk::PipelineColorBlendAttachmentState::default()
            .color_write_mask(vk::ColorComponentFlags::RGBA)
            .blend_enable(false);

        let color_blending = vk::PipelineColorBlendStateCreateInfo::default()
            .attachments(std::slice::from_ref(&color_blend_attachment));

        let pipeline_info = vk::GraphicsPipelineCreateInfo::default()
            .stages(&shader_stages)
            .vertex_input_state(&vertex_input)
            .input_assembly_state(&input_assembly)
            .viewport_state(&viewport_state)
            .rasterization_state(&rasterizer)
            .multisample_state(&multisampling)
            .color_blend_state(&color_blending)
            .layout(layout)
            .render_pass(render_pass)
            .subpass(0);

        let pipelines = device
            .create_graphics_pipelines(vk::PipelineCache::null(), &[pipeline_info], None)
            .map_err(|e| format!("Background pipeline creation failed: {:?}", e.1))?;

        device.destroy_shader_module(vert_module, None);
        device.destroy_shader_module(frag_module, None);

        Ok(pipelines[0])
    }

    unsafe fn create_shader_module(device: &Device, code: &[u8]) -> Result<vk::ShaderModule, vk::Result> {
        let code_u32: Vec<u32> = code
            .chunks_exact(4)
            .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();
        let create_info = vk::ShaderModuleCreateInfo::default().code(&code_u32);
        device.create_shader_module(&create_info, None)
    }

    unsafe fn draw_frame(&mut self, time: f32, pitch: f32) -> Result<(), vk::Result> {
        self.device.wait_for_fences(&[self.in_flight_fence], true, u64::MAX)?;
        self.device.reset_fences(&[self.in_flight_fence])?;

        let (image_index, _) = self.swapchain_loader.acquire_next_image(
            self.swapchain,
            u64::MAX,
            self.image_available_semaphore,
            vk::Fence::null(),
        )?;

        let cmd = self.command_buffers[image_index as usize];
        self.device.reset_command_buffer(cmd, vk::CommandBufferResetFlags::empty())?;
        self.device.begin_command_buffer(cmd, &vk::CommandBufferBeginInfo::default())?;

        let clear_values = [vk::ClearValue {
            color: vk::ClearColorValue { float32: [0.02, 0.02, 0.03, 1.0] },
        }];

        let render_pass_info = vk::RenderPassBeginInfo::default()
            .render_pass(self.render_pass)
            .framebuffer(self.framebuffers[image_index as usize])
            .render_area(vk::Rect2D { offset: vk::Offset2D { x: 0, y: 0 }, extent: self.extent })
            .clear_values(&clear_values);

        self.device.cmd_begin_render_pass(cmd, &render_pass_info, vk::SubpassContents::INLINE);

        let aspect = self.extent.width as f32 / self.extent.height as f32;
        let proj = Mat4::perspective_rh(45.0_f32.to_radians(), aspect, 0.1, 100.0);
        
        let camera_angle = time * 0.3;
        let camera_radius = 5.0;
        
        let camera_pos = Vec3::new(
            camera_angle.sin() * camera_radius,
            1.5,
            camera_angle.cos() * camera_radius,
        );
        
        let look_dir = Vec3::new(
            -camera_pos.x,
            pitch.sin() * camera_radius,
            -camera_pos.z,
        ).normalize();
        let look_target = camera_pos + look_dir;
        let view = Mat4::look_at_rh(camera_pos, look_target, Vec3::Y);
        
        let model = Mat4::from_rotation_y(time * 0.5);
        let mvp = proj * view * model;
        
        let view_proj = proj * view;
        let inv_view_proj = view_proj.inverse();

        // Light position behind the camera
        let light_pos = camera_pos + Vec3::new(0.0, 3.0, 0.0);

        // === DRAW BACKGROUND FIRST ===
        self.device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, self.bg_pipeline);
        self.device.cmd_bind_descriptor_sets(
            cmd,
            vk::PipelineBindPoint::GRAPHICS,
            self.bg_pipeline_layout,
            0,
            &[self.bg_descriptor_set],
            &[],
        );

        let bg_push_constants = PushConstants {
            mvp: mvp.to_cols_array_2d(),
            inv_view_proj: inv_view_proj.to_cols_array_2d(),
            camera_pos: Vec4::new(camera_pos.x, camera_pos.y, camera_pos.z, 1.0).to_array(),
            light_pos: Vec4::new(light_pos.x, light_pos.y, light_pos.z, 1.0).to_array(),
            time,
            _padding: [0.0; 3],
        };

        self.device.cmd_push_constants(
            cmd,
            self.bg_pipeline_layout,
            vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
            0,
            bytemuck::bytes_of(&bg_push_constants),
        );

        self.device.cmd_draw(cmd, 3, 1, 0, 0);

        // === DRAW MESH SHADER CUBES ===
        self.device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, self.pipeline);
        self.device.cmd_bind_descriptor_sets(
            cmd,
            vk::PipelineBindPoint::GRAPHICS,
            self.pipeline_layout,
            0,
            &[self.descriptor_set],
            &[],
        );

        let push_constants = PushConstants {
            mvp: mvp.to_cols_array_2d(),
            inv_view_proj: inv_view_proj.to_cols_array_2d(),
            camera_pos: Vec4::new(camera_pos.x, camera_pos.y, camera_pos.z, 1.0).to_array(),
            light_pos: Vec4::new(light_pos.x, light_pos.y, light_pos.z, 1.0).to_array(),
            time,
            _padding: [0.0; 3],
        };

        self.device.cmd_push_constants(
            cmd,
            self.pipeline_layout,
            vk::ShaderStageFlags::MESH_EXT | vk::ShaderStageFlags::TASK_EXT | vk::ShaderStageFlags::FRAGMENT,
            0,
            bytemuck::bytes_of(&push_constants),
        );

        self.mesh_shader_ext.cmd_draw_mesh_tasks(cmd, 1, 1, 1);

        self.device.cmd_end_render_pass(cmd);
        self.device.end_command_buffer(cmd)?;

        let wait_semaphores = [self.image_available_semaphore];
        let wait_stages = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
        let signal_semaphores = [self.render_finished_semaphore];

        let submit_info = vk::SubmitInfo::default()
            .wait_semaphores(&wait_semaphores)
            .wait_dst_stage_mask(&wait_stages)
            .command_buffers(std::slice::from_ref(&cmd))
            .signal_semaphores(&signal_semaphores);

        self.device.queue_submit(self.graphics_queue, &[submit_info], self.in_flight_fence)?;

        let present_info = vk::PresentInfoKHR::default()
            .wait_semaphores(&signal_semaphores)
            .swapchains(std::slice::from_ref(&self.swapchain))
            .image_indices(std::slice::from_ref(&image_index));

        self.swapchain_loader.queue_present(self.graphics_queue, &present_info)?;
        Ok(())
    }
}


impl Drop for VulkanApp {
    fn drop(&mut self) {
        unsafe {
            self.device.device_wait_idle().unwrap();

            // Destroy textures
            self.device.destroy_image_view(self.albedo_texture.view, None);
            self.device.destroy_image(self.albedo_texture.image, None);
            if let Some(alloc) = self.albedo_texture.allocation.take() {
                self.allocator.lock().unwrap().free(alloc).unwrap();
            }

            self.device.destroy_image_view(self.normal_texture.view, None);
            self.device.destroy_image(self.normal_texture.image, None);
            if let Some(alloc) = self.normal_texture.allocation.take() {
                self.allocator.lock().unwrap().free(alloc).unwrap();
            }

            self.device.destroy_image_view(self.rma_texture.view, None);
            self.device.destroy_image(self.rma_texture.image, None);
            if let Some(alloc) = self.rma_texture.allocation.take() {
                self.allocator.lock().unwrap().free(alloc).unwrap();
            }

            self.device.destroy_image_view(self.cubemap_texture.view, None);
            self.device.destroy_image(self.cubemap_texture.image, None);
            if let Some(alloc) = self.cubemap_texture.allocation.take() {
                self.allocator.lock().unwrap().free(alloc).unwrap();
            }

            // Destroy acceleration structures
            self.accel_struct_ext.destroy_acceleration_structure(self.tlas.handle, None);
            self.device.destroy_buffer(self.tlas.buffer, None);
            if let Some(alloc) = self.tlas.allocation.take() {
                self.allocator.lock().unwrap().free(alloc).unwrap();
            }

            self.accel_struct_ext.destroy_acceleration_structure(self.blas.handle, None);
            self.device.destroy_buffer(self.blas.buffer, None);
            if let Some(alloc) = self.blas.allocation.take() {
                self.allocator.lock().unwrap().free(alloc).unwrap();
            }

            // Destroy vertex/index buffers
            self.device.destroy_buffer(self.vertex_buffer, None);
            if let Some(alloc) = self.vertex_buffer_allocation.take() {
                self.allocator.lock().unwrap().free(alloc).unwrap();
            }
            self.device.destroy_buffer(self.index_buffer, None);
            if let Some(alloc) = self.index_buffer_allocation.take() {
                self.allocator.lock().unwrap().free(alloc).unwrap();
            }

            self.device.destroy_sampler(self.sampler, None);
            self.device.destroy_descriptor_pool(self.descriptor_pool, None);
            self.device.destroy_descriptor_set_layout(self.descriptor_set_layout, None);
            self.device.destroy_descriptor_set_layout(self.bg_descriptor_set_layout, None);

            self.device.destroy_fence(self.in_flight_fence, None);
            self.device.destroy_semaphore(self.render_finished_semaphore, None);
            self.device.destroy_semaphore(self.image_available_semaphore, None);
            self.device.destroy_command_pool(self.command_pool, None);

            for &fb in &self.framebuffers {
                self.device.destroy_framebuffer(fb, None);
            }

            self.device.destroy_pipeline(self.pipeline, None);
            self.device.destroy_pipeline_layout(self.pipeline_layout, None);
            self.device.destroy_pipeline(self.bg_pipeline, None);
            self.device.destroy_pipeline_layout(self.bg_pipeline_layout, None);
            self.device.destroy_render_pass(self.render_pass, None);

            for &view in &self.swapchain_image_views {
                self.device.destroy_image_view(view, None);
            }

            self.swapchain_loader.destroy_swapchain(self.swapchain, None);
            self.surface_loader.destroy_surface(self.surface, None);
            self.device.destroy_device(None);
            self.instance.destroy_instance(None);
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let event_loop = EventLoop::new()?;
    let window = WindowBuilder::new()
        .with_title("Mesh Shader + RT Shadows Demo")
        .with_inner_size(winit::dpi::LogicalSize::new(WIDTH, HEIGHT))
        .build(&event_loop)?;

    let mut app = unsafe { VulkanApp::new(&window)? };
    let start_time = std::time::Instant::now();
    
    let mut pitch: f32 = 0.0;
    let pitch_speed: f32 = 0.05;
    let max_pitch: f32 = std::f32::consts::FRAC_PI_2 - 0.01;

    event_loop.run(move |event, elwt| {
        elwt.set_control_flow(ControlFlow::Poll);

        match event {
            Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => elwt.exit(),
            Event::WindowEvent { event: WindowEvent::KeyboardInput { event, .. }, .. } => {
                if event.state == ElementState::Pressed {
                    match event.physical_key {
                        PhysicalKey::Code(KeyCode::KeyW) | PhysicalKey::Code(KeyCode::ArrowUp) => {
                            pitch = (pitch + pitch_speed).min(max_pitch);
                        }
                        PhysicalKey::Code(KeyCode::KeyS) | PhysicalKey::Code(KeyCode::ArrowDown) => {
                            pitch = (pitch - pitch_speed).max(-max_pitch);
                        }
                        PhysicalKey::Code(KeyCode::Escape) => elwt.exit(),
                        _ => {}
                    }
                }
            }
            Event::AboutToWait => {
                let time = start_time.elapsed().as_secs_f32();
                unsafe {
                    if let Err(e) = app.draw_frame(time, pitch) {
                        eprintln!("Draw error: {:?}", e);
                    }
                }
            }
            _ => {}
        }
    })?;

    Ok(())
}
