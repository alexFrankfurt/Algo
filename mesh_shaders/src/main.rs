//! Mesh Shader Demo with Textures
//!
//! Demonstrates mesh shaders with PBR textures (albedo, normal, roughness/metallic/AO)

use ash::{vk, Device, Entry, Instance};
use ash::ext::mesh_shader;
use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};
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
    time: f32,
    _padding: [f32; 3],
}

struct Texture {
    image: vk::Image,
    view: vk::ImageView,
    allocation: Option<Allocation>,
}

struct VulkanApp {
    _entry: Entry,
    instance: Instance,
    device: Device,
    allocator: Arc<Mutex<Allocator>>,
    mesh_shader_ext: mesh_shader::Device,
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
}

impl VulkanApp {
    unsafe fn new(window: &winit::window::Window) -> Result<Self, Box<dyn std::error::Error>> {
        let entry = Entry::load()?;

        // Create instance
        let app_info = vk::ApplicationInfo::default()
            .application_name(CStr::from_bytes_with_nul(b"Mesh Shader Demo\0")?)
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

        // Pick physical device
        let physical_devices = instance.enumerate_physical_devices()?;
        let physical_device = physical_devices
            .into_iter()
            .find(|&pd| Self::check_mesh_shader_support(&instance, pd))
            .ok_or("No GPU with mesh shader support found")?;

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

        // Create device
        let queue_priorities = [1.0f32];
        let queue_create_info = vk::DeviceQueueCreateInfo::default()
            .queue_family_index(queue_family_index)
            .queue_priorities(&queue_priorities);

        let device_extensions = [
            ash::khr::swapchain::NAME.as_ptr(),
            mesh_shader::NAME.as_ptr(),
        ];

        let mut mesh_shader_features = vk::PhysicalDeviceMeshShaderFeaturesEXT::default()
            .mesh_shader(true)
            .task_shader(true);

        let mut features_12 = vk::PhysicalDeviceVulkan12Features::default()
            .buffer_device_address(true)
            .descriptor_indexing(true);

        let mut features_13 = vk::PhysicalDeviceVulkan13Features::default().maintenance4(true);

        let device_create_info = vk::DeviceCreateInfo::default()
            .queue_create_infos(std::slice::from_ref(&queue_create_info))
            .enabled_extension_names(&device_extensions)
            .push_next(&mut mesh_shader_features)
            .push_next(&mut features_12)
            .push_next(&mut features_13);

        let device = instance.create_device(physical_device, &device_create_info, None)?;
        let mesh_shader_ext = mesh_shader::Device::new(&instance, &device);
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
            true, // sRGB
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

        // Load cubemap for background (convert equirectangular to proper cubemap)
        let cubemap_texture = Self::load_cubemap(
            &device,
            &allocator,
            command_pool,
            graphics_queue,
            "assets/cubemap.png",
        )?;

        // Create descriptor set layout for main pipeline
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
        ];

        let layout_info = vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings);
        let descriptor_set_layout = device.create_descriptor_set_layout(&layout_info, None)?;

        // Create descriptor pool (4 samplers: 3 for main + 1 for background)
        let pool_sizes = [vk::DescriptorPoolSize::default()
            .ty(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
            .descriptor_count(4)];
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

        let writes: Vec<_> = image_infos
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
        // Background descriptor set layout (single cubemap sampler)
        let bg_bindings = [
            vk::DescriptorSetLayoutBinding::default()
                .binding(0)
                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::FRAGMENT),
        ];
        let bg_layout_info = vk::DescriptorSetLayoutCreateInfo::default().bindings(&bg_bindings);
        let bg_descriptor_set_layout = device.create_descriptor_set_layout(&bg_layout_info, None)?;

        // Allocate background descriptor set
        let bg_alloc_info = vk::DescriptorSetAllocateInfo::default()
            .descriptor_pool(descriptor_pool)
            .set_layouts(std::slice::from_ref(&bg_descriptor_set_layout));
        let bg_descriptor_sets = device.allocate_descriptor_sets(&bg_alloc_info)?;
        let bg_descriptor_set = bg_descriptor_sets[0];

        // Update background descriptor set with cubemap
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

        // Background pipeline layout
        let bg_push_constant_range = vk::PushConstantRange::default()
            .stage_flags(vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT)
            .size(std::mem::size_of::<PushConstants>() as u32);
        let bg_pipeline_layout_info = vk::PipelineLayoutCreateInfo::default()
            .set_layouts(std::slice::from_ref(&bg_descriptor_set_layout))
            .push_constant_ranges(std::slice::from_ref(&bg_push_constant_range));
        let bg_pipeline_layout = device.create_pipeline_layout(&bg_pipeline_layout_info, None)?;

        // Create background pipeline
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
        })
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

        // Create image
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

        // Create staging buffer
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

        // Copy pixels to staging
        let mapped = staging_alloc.mapped_ptr().unwrap().as_ptr() as *mut u8;
        std::ptr::copy_nonoverlapping(pixels.as_ptr(), mapped, pixels.len());

        // Transfer to GPU
        let cmd_info = vk::CommandBufferAllocateInfo::default()
            .command_pool(command_pool)
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(1);
        let cmd = device.allocate_command_buffers(&cmd_info)?[0];

        device.begin_command_buffer(cmd, &vk::CommandBufferBeginInfo::default())?;

        // Transition to transfer dst
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

        // Copy buffer to image
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

        // Transition to shader read
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

        // Create image view
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

    /// Convert equirectangular image to 6 cubemap faces
    fn convert_equirectangular_to_cubemap(src: &image::RgbaImage) -> Vec<Vec<u8>> {
        let (src_width, src_height) = src.dimensions();
        let face_size = src_width / 4;
        
        let mut faces: Vec<Vec<u8>> = vec![vec![0u8; (face_size * face_size * 4) as usize]; 6];
        
        const PI: f32 = std::f32::consts::PI;
        
        // Face indices: +X, -X, +Y, -Y, +Z, -Z
        for face in 0..6 {
            for y in 0..face_size {
                for x in 0..face_size {
                    // Normalize coordinates to -1..1
                    let a = 2.0 * (x as f32) / (face_size as f32) - 1.0;
                    let b = 2.0 * (y as f32) / (face_size as f32) - 1.0;
                    
                    // Convert face coordinates to 3D direction (physics coordinate system)
                    let dir = match face {
                        0 => glam::Vec3::new(1.0, -a, -b),      // +X
                        1 => glam::Vec3::new(-1.0, a, -b),      // -X
                        2 => glam::Vec3::new(a, 1.0, b),        // +Y (up)
                        3 => glam::Vec3::new(a, -1.0, -b),      // -Y (down)
                        4 => glam::Vec3::new(a, -b, 1.0),       // +Z
                        5 => glam::Vec3::new(-a, -b, -1.0),     // -Z
                        _ => unreachable!(),
                    };
                    
                    // Convert to spherical coordinates
                    let r = (dir.x * dir.x + dir.y * dir.y).sqrt();
                    let phi = dir.y.atan2(dir.x);
                    let theta = dir.z.atan2(r);
                    
                    // Convert to UV coordinates
                    let u = (phi + PI) / (2.0 * PI);
                    let v = (PI / 2.0 - theta) / PI;
                    
                    // Sample from source image with bilinear interpolation
                    let src_x = u * (src_width as f32);
                    let src_y = v * (src_height as f32);
                    
                    let x0 = (src_x.floor() as u32).min(src_width - 1);
                    let y0 = (src_y.floor() as u32).min(src_height - 1);
                    let x1 = (x0 + 1).min(src_width - 1);
                    let y1 = (y0 + 1).min(src_height - 1);
                    
                    let fx = src_x - src_x.floor();
                    let fy = src_y - src_y.floor();
                    
                    let p00 = src.get_pixel(x0, y0);
                    let p10 = src.get_pixel(x1, y0);
                    let p01 = src.get_pixel(x0, y1);
                    let p11 = src.get_pixel(x1, y1);
                    
                    // Bilinear interpolation
                    let mut color = [0u8; 4];
                    for c in 0..4 {
                        let v00 = p00[c] as f32;
                        let v10 = p10[c] as f32;
                        let v01 = p01[c] as f32;
                        let v11 = p11[c] as f32;
                        
                        let value = v00 * (1.0 - fx) * (1.0 - fy)
                                  + v10 * fx * (1.0 - fy)
                                  + v01 * (1.0 - fx) * fy
                                  + v11 * fx * fy;
                        color[c] = value.clamp(0.0, 255.0) as u8;
                    }
                    
                    let idx = ((y * face_size + x) * 4) as usize;
                    faces[face][idx..idx + 4].copy_from_slice(&color);
                }
            }
        }
        
        faces
    }

    /// Load equirectangular image and convert to Vulkan cubemap texture
    unsafe fn load_cubemap(
        device: &Device,
        allocator: &Arc<Mutex<Allocator>>,
        command_pool: vk::CommandPool,
        queue: vk::Queue,
        path: &str,
    ) -> Result<Texture, Box<dyn std::error::Error>> {
        let img = image::open(path)?.to_rgba8();
        let (width, _height) = img.dimensions();
        let face_size = width / 4;
        
        // Convert equirectangular to 6 cubemap faces
        let faces = Self::convert_equirectangular_to_cubemap(&img);
        
        let format = vk::Format::R8G8B8A8_SRGB;
        
        // Create cubemap image with 6 array layers
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
        
        // Create staging buffer for all 6 faces
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
        
        // Copy all faces to staging buffer
        let mapped = staging_alloc.mapped_ptr().unwrap().as_ptr() as *mut u8;
        for (i, face_data) in faces.iter().enumerate() {
            let offset = i * face_data.len();
            std::ptr::copy_nonoverlapping(face_data.as_ptr(), mapped.add(offset), face_data.len());
        }
        
        // Transfer to GPU
        let cmd_info = vk::CommandBufferAllocateInfo::default()
            .command_pool(command_pool)
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(1);
        let cmd = device.allocate_command_buffers(&cmd_info)?[0];
        
        device.begin_command_buffer(cmd, &vk::CommandBufferBeginInfo::default())?;
        
        // Transition all layers to transfer dst
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
        
        // Copy each face from staging buffer to image layer
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
        
        // Transition to shader read
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
        
        // Create cubemap image view
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

    unsafe fn check_mesh_shader_support(instance: &Instance, device: vk::PhysicalDevice) -> bool {
        let mut mesh_shader_features = vk::PhysicalDeviceMeshShaderFeaturesEXT::default();
        let mut features2 =
            vk::PhysicalDeviceFeatures2::default().push_next(&mut mesh_shader_features);
        instance.get_physical_device_features2(device, &mut features2);
        mesh_shader_features.mesh_shader == vk::TRUE
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
            .cull_mode(vk::CullModeFlags::NONE)  // Disable culling for glass
            .front_face(vk::FrontFace::COUNTER_CLOCKWISE);

        let multisampling = vk::PipelineMultisampleStateCreateInfo::default()
            .rasterization_samples(vk::SampleCountFlags::TYPE_1);

        // Enable alpha blending for glass transparency
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

        // No vertex input - fullscreen triangle generated in shader
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

        // No blending for background - it's opaque
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

        // Calculate matrices
        let aspect = self.extent.width as f32 / self.extent.height as f32;
        let proj = Mat4::perspective_rh(45.0_f32.to_radians(), aspect, 0.1, 100.0);
        
        // Rotate the camera around the scene for the skybox effect
        let camera_angle = time * 0.3;
        let camera_radius = 5.0;
        
        // Camera position orbits horizontally
        let camera_pos = Vec3::new(
            camera_angle.sin() * camera_radius,
            1.5,
            camera_angle.cos() * camera_radius,
        );
        
        // Look target adjusted by pitch (W/S or Up/Down arrows)
        let look_height = pitch * 3.0;  // Scale pitch to reasonable look distance
        let look_target = Vec3::new(0.0, look_height, 0.0);
        let view = Mat4::look_at_rh(camera_pos, look_target, Vec3::Y);
        
        // Model rotation for the cubes (independent of camera)
        let model = Mat4::from_rotation_y(time * 0.5);
        let mvp = proj * view * model;
        
        // For background: use view-projection without model transform
        let view_proj = proj * view;
        let inv_view_proj = view_proj.inverse();

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

        // Draw fullscreen triangle for background
        self.device.cmd_draw(cmd, 3, 1, 0, 0);

        // === DRAW MESH SHADER BARS ===
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

            // Destroy cubemap texture
            self.device.destroy_image_view(self.cubemap_texture.view, None);
            self.device.destroy_image(self.cubemap_texture.image, None);
            if let Some(alloc) = self.cubemap_texture.allocation.take() {
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
        .with_title("Mesh Shader + Textures Demo")
        .with_inner_size(winit::dpi::LogicalSize::new(WIDTH, HEIGHT))
        .build(&event_loop)?;

    let mut app = unsafe { VulkanApp::new(&window)? };
    let start_time = std::time::Instant::now();
    
    // Camera pitch angle (vertical look direction)
    let mut pitch: f32 = 0.0;
    let pitch_speed: f32 = 0.05;
    let max_pitch: f32 = 1.2;  // ~70 degrees

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
