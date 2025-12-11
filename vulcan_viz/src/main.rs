use ash::{vk, Entry};
use ash::extensions::{
    khr::{Surface, Swapchain, RayTracingPipeline, AccelerationStructure, DeferredHostOperations},
    ext::DebugUtils,
};
use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};
use raw_window_handle::{HasRawDisplayHandle, HasRawWindowHandle};
use std::ffi::CStr;
use std::mem::size_of;

const NUM_BARS: usize = 12;

struct SortSystem {
    values: Vec<u32>,
    i: usize,
    j: usize,
    sorted: bool,
}

impl SortSystem {
    fn new(count: usize) -> Self {
        let mut values: Vec<u32> = (1..=count as u32).collect();
        // Simple shuffle
        for i in 0..count {
            let r = (i * 1987 + 3) % count;
            values.swap(i, r);
        }
        Self { values, i: 0, j: 0, sorted: false }
    }

    fn step(&mut self) {
        if self.sorted { return; }
        let n = self.values.len();
        if self.i < n {
            if self.j < n - 1 - self.i {
                if self.values[self.j] > self.values[self.j + 1] {
                    self.values.swap(self.j, self.j + 1);
                }
                self.j += 1;
            } else {
                self.j = 0;
                self.i += 1;
            }
        } else {
            self.sorted = true;
            // Reset for infinite loop
            self.i = 0;
            self.j = 0;
            self.sorted = false;
            // Shuffle again?
             for i in 0..n {
                let r = (i * 1987 + 3) % n;
                self.values.swap(i, r);
            }
        }
    }
}

// Helper to find memory type
fn find_memorytype_index(
    memory_req: &vk::MemoryRequirements,
    memory_prop: &vk::PhysicalDeviceMemoryProperties,
    flags: vk::MemoryPropertyFlags,
) -> Option<u32> {
    memory_prop.memory_types[..memory_prop.memory_type_count as _]
        .iter()
        .enumerate()
        .find(|(index, memory_type)| {
            (1 << index) & memory_req.memory_type_bits != 0
                && memory_type.property_flags & flags == flags
        })
        .map(|(index, _)| index as _)
}

fn main() {
    let event_loop = EventLoop::new().unwrap();
    let window = WindowBuilder::new().with_title("Vulcan Viz RTX").build(&event_loop).unwrap();

    unsafe {
        let entry = Entry::load().unwrap();
        let app_name = CStr::from_bytes_with_nul(b"Vulcan Viz RTX\0").unwrap();
        let app_info = vk::ApplicationInfo::builder()
            .application_name(app_name)
            .api_version(vk::API_VERSION_1_2); // RT requires 1.2+

        let mut extension_names = ash_window::enumerate_required_extensions(window.raw_display_handle())
            .unwrap()
            .to_vec();
        extension_names.push(DebugUtils::name().as_ptr());

        let instance_create_info = vk::InstanceCreateInfo::builder()
            .application_info(&app_info)
            .enabled_extension_names(&extension_names);

        let instance = entry.create_instance(&instance_create_info, None).unwrap();

        let surface_loader = Surface::new(&entry, &instance);
        let surface = ash_window::create_surface(
            &entry,
            &instance,
            window.raw_display_handle(),
            window.raw_window_handle(),
            None,
        ).unwrap();

        let pdevices = instance.enumerate_physical_devices().unwrap();
        let (pdevice, queue_family_index) = pdevices
            .iter()
            .find_map(|pdevice| {
                instance.get_physical_device_queue_family_properties(*pdevice)
                    .iter()
                    .enumerate()
                    .find_map(|(index, info)| {
                        let supports_graphic_and_surface = info.queue_flags.contains(vk::QueueFlags::GRAPHICS)
                            && surface_loader.get_physical_device_surface_support(*pdevice, index as u32, surface).unwrap();
                        if supports_graphic_and_surface {
                            Some((*pdevice, index as u32))
                        } else {
                            None
                        }
                    })
            })
            .expect("No suitable physical device found");

        // Enable RT extensions
        let device_extension_names = [
            Swapchain::name().as_ptr(),
            RayTracingPipeline::name().as_ptr(),
            AccelerationStructure::name().as_ptr(),
            DeferredHostOperations::name().as_ptr(),
            vk::KhrBufferDeviceAddressFn::name().as_ptr(),
            vk::ExtDescriptorIndexingFn::name().as_ptr(),
            vk::KhrSpirv14Fn::name().as_ptr(),
            vk::KhrShaderFloatControlsFn::name().as_ptr(),
        ];

        let mut buffer_device_address_features = vk::PhysicalDeviceBufferDeviceAddressFeatures::builder()
            .buffer_device_address(true);
        let mut ray_tracing_pipeline_features = vk::PhysicalDeviceRayTracingPipelineFeaturesKHR::builder()
            .ray_tracing_pipeline(true);
        let mut acceleration_structure_features = vk::PhysicalDeviceAccelerationStructureFeaturesKHR::builder()
            .acceleration_structure(true);
        let mut scalar_block_layout_features = vk::PhysicalDeviceScalarBlockLayoutFeatures::builder()
            .scalar_block_layout(true);

        let queue_priorities = [1.0];
        let queue_create_info = vk::DeviceQueueCreateInfo::builder()
            .queue_family_index(queue_family_index)
            .queue_priorities(&queue_priorities)
            .build();
        let queue_create_infos = [queue_create_info];

        let device_create_info = vk::DeviceCreateInfo::builder()
            .queue_create_infos(&queue_create_infos)
            .enabled_extension_names(&device_extension_names)
            .push_next(&mut buffer_device_address_features)
            .push_next(&mut ray_tracing_pipeline_features)
            .push_next(&mut acceleration_structure_features)
            .push_next(&mut scalar_block_layout_features);

        let device = instance.create_device(pdevice, &device_create_info, None).unwrap();
        let queue = device.get_device_queue(queue_family_index, 0);

        // Load RT Loaders
        let rt_pipeline_loader = RayTracingPipeline::new(&instance, &device);
        let as_loader = AccelerationStructure::new(&instance, &device);

        // Swapchain
        let mut surface_caps = surface_loader.get_physical_device_surface_capabilities(pdevice, surface).unwrap();
        let format = surface_loader.get_physical_device_surface_formats(pdevice, surface).unwrap()[0];
        let swapchain_loader = Swapchain::new(&instance, &device);
        let mut swapchain_create_info = vk::SwapchainCreateInfoKHR::builder()
            .surface(surface)
            .min_image_count(2.max(surface_caps.min_image_count))
            .image_format(format.format)
            .image_color_space(format.color_space)
            .image_extent(surface_caps.current_extent)
            .image_array_layers(1)
            .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::TRANSFER_DST) // Important for copy
            .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
            .pre_transform(surface_caps.current_transform)
            .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
            .present_mode(vk::PresentModeKHR::FIFO);
        let mut swapchain = swapchain_loader.create_swapchain(&swapchain_create_info, None).unwrap();
        let mut present_images = swapchain_loader.get_swapchain_images(swapchain).unwrap();

        // Command Pool
        let command_pool = device.create_command_pool(
            &vk::CommandPoolCreateInfo::builder()
                .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER)
                .queue_family_index(queue_family_index),
            None,
        ).unwrap();
        let command_buffer = device.allocate_command_buffers(
            &vk::CommandBufferAllocateInfo::builder()
                .command_pool(command_pool)
                .level(vk::CommandBufferLevel::PRIMARY)
                .command_buffer_count(1),
        ).unwrap()[0];

        // --- Ray Tracing Setup ---

        // 1. Bottom Level AS (BLAS) - The Cube
        // Vertices (Unit Cube: -0.5 to 0.5 in X/Z, 0.0 to 1.0 in Y)
        let vertices: [f32; 24] = [
            -0.5, 0.0,  0.5,   0.5, 0.0,  0.5,   0.5,  1.0,  0.5,  -0.5,  1.0,  0.5, // Front
            -0.5, 0.0, -0.5,   0.5, 0.0, -0.5,   0.5,  1.0, -0.5,  -0.5,  1.0, -0.5, // Back
        ];
        let indices: [u32; 36] = [
            0, 1, 2, 2, 3, 0, // Front
            1, 5, 6, 6, 2, 1, // Right
            5, 4, 7, 7, 6, 5, // Back
            4, 0, 3, 3, 7, 4, // Left
            3, 2, 6, 6, 7, 3, // Top
            4, 5, 1, 1, 0, 4, // Bottom
        ];

        // Create Buffers (Helper needed, but inline for now)
        let memory_props = instance.get_physical_device_memory_properties(pdevice);
        
        let create_buffer = |size: u64, usage: vk::BufferUsageFlags, props: vk::MemoryPropertyFlags| -> (vk::Buffer, vk::DeviceMemory) {
            let buffer_info = vk::BufferCreateInfo::builder()
                .size(size)
                .usage(usage)
                .sharing_mode(vk::SharingMode::EXCLUSIVE);
            let buffer = device.create_buffer(&buffer_info, None).unwrap();
            let req = device.get_buffer_memory_requirements(buffer);
            let index = find_memorytype_index(&req, &memory_props, props).unwrap();
            let alloc_info = vk::MemoryAllocateInfo::builder()
                .allocation_size(req.size)
                .memory_type_index(index);
            // Enable buffer device address if needed (for AS inputs)
            let mut flags_info = vk::MemoryAllocateFlagsInfo::builder()
                .flags(vk::MemoryAllocateFlags::DEVICE_ADDRESS);
            let alloc_info = if usage.contains(vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS) {
                alloc_info.push_next(&mut flags_info)
            } else {
                alloc_info
            };
            let memory = device.allocate_memory(&alloc_info, None).unwrap();
            device.bind_buffer_memory(buffer, memory, 0).unwrap();
            (buffer, memory)
        };

        let (vertex_buffer, vertex_mem) = create_buffer(
            (vertices.len() * 4) as u64,
            vk::BufferUsageFlags::ACCELERATION_STRUCTURE_BUILD_INPUT_READ_ONLY_KHR | vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS | vk::BufferUsageFlags::STORAGE_BUFFER,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
        );
        let ptr = device.map_memory(vertex_mem, 0, (vertices.len() * 4) as u64, vk::MemoryMapFlags::empty()).unwrap();
        std::ptr::copy_nonoverlapping(vertices.as_ptr() as *const u8, ptr as *mut u8, vertices.len() * 4);
        device.unmap_memory(vertex_mem);

        let (index_buffer, index_mem) = create_buffer(
            (indices.len() * 4) as u64,
            vk::BufferUsageFlags::ACCELERATION_STRUCTURE_BUILD_INPUT_READ_ONLY_KHR | vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS | vk::BufferUsageFlags::STORAGE_BUFFER,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
        );
        let ptr = device.map_memory(index_mem, 0, (indices.len() * 4) as u64, vk::MemoryMapFlags::empty()).unwrap();
        std::ptr::copy_nonoverlapping(indices.as_ptr() as *const u8, ptr as *mut u8, indices.len() * 4);
        device.unmap_memory(index_mem);

        let vertex_addr = vk::DeviceOrHostAddressConstKHR {
            device_address: device.get_buffer_device_address(&vk::BufferDeviceAddressInfo::builder().buffer(vertex_buffer)),
        };
        let index_addr = vk::DeviceOrHostAddressConstKHR {
            device_address: device.get_buffer_device_address(&vk::BufferDeviceAddressInfo::builder().buffer(index_buffer)),
        };

        let geometry = vk::AccelerationStructureGeometryKHR::builder()
            .geometry_type(vk::GeometryTypeKHR::TRIANGLES)
            .geometry(vk::AccelerationStructureGeometryDataKHR {
                triangles: vk::AccelerationStructureGeometryTrianglesDataKHR::builder()
                    .vertex_format(vk::Format::R32G32B32_SFLOAT)
                    .vertex_data(vertex_addr)
                    .vertex_stride(12)
                    .max_vertex(8)
                    .index_type(vk::IndexType::UINT32)
                    .index_data(index_addr)
                    .build(),
            })
            .flags(vk::GeometryFlagsKHR::OPAQUE);

        let build_range = vk::AccelerationStructureBuildRangeInfoKHR::builder()
            .primitive_count(12)
            .primitive_offset(0)
            .first_vertex(0)
            .transform_offset(0);

        let build_info = vk::AccelerationStructureBuildGeometryInfoKHR::builder()
            .ty(vk::AccelerationStructureTypeKHR::BOTTOM_LEVEL)
            .flags(vk::BuildAccelerationStructureFlagsKHR::PREFER_FAST_TRACE)
            .geometries(std::slice::from_ref(&geometry));

        let size_info = as_loader.get_acceleration_structure_build_sizes(
            vk::AccelerationStructureBuildTypeKHR::DEVICE,
            &build_info,
            &[12],
        );

        let (blas_buffer, _blas_mem) = create_buffer(
            size_info.acceleration_structure_size,
            vk::BufferUsageFlags::ACCELERATION_STRUCTURE_STORAGE_KHR | vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS,
            vk::MemoryPropertyFlags::DEVICE_LOCAL,
        );
        let blas = as_loader.create_acceleration_structure(
            &vk::AccelerationStructureCreateInfoKHR::builder()
                .buffer(blas_buffer)
                .size(size_info.acceleration_structure_size)
                .ty(vk::AccelerationStructureTypeKHR::BOTTOM_LEVEL),
            None,
        ).unwrap();

        let (scratch_buffer, _scratch_mem) = create_buffer(
            size_info.build_scratch_size,
            vk::BufferUsageFlags::STORAGE_BUFFER | vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS,
            vk::MemoryPropertyFlags::DEVICE_LOCAL,
        );
        let scratch_addr = device.get_buffer_device_address(&vk::BufferDeviceAddressInfo::builder().buffer(scratch_buffer));

        let build_info = build_info.dst_acceleration_structure(blas).scratch_data(vk::DeviceOrHostAddressKHR { device_address: scratch_addr });

        // Build BLAS
        device.begin_command_buffer(command_buffer, &vk::CommandBufferBeginInfo::builder().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT)).unwrap();
        as_loader.cmd_build_acceleration_structures(command_buffer, &[build_info.build()], &[&[build_range.build()]]);
        
        // Barrier for BLAS build
        let memory_barrier = vk::MemoryBarrier::builder()
            .src_access_mask(vk::AccessFlags::ACCELERATION_STRUCTURE_WRITE_KHR)
            .dst_access_mask(vk::AccessFlags::ACCELERATION_STRUCTURE_READ_KHR);
        device.cmd_pipeline_barrier(command_buffer, vk::PipelineStageFlags::ACCELERATION_STRUCTURE_BUILD_KHR, vk::PipelineStageFlags::ACCELERATION_STRUCTURE_BUILD_KHR, vk::DependencyFlags::empty(), &[memory_barrier.build()], &[], &[]);

        device.end_command_buffer(command_buffer).unwrap();
        device.queue_submit(queue, &[vk::SubmitInfo::builder().command_buffers(&[command_buffer]).build()], vk::Fence::null()).unwrap();
        device.device_wait_idle().unwrap();

        // 2. Top Level AS (TLAS)
        let mut sort_system = SortSystem::new(NUM_BARS);
        let total_instances = NUM_BARS + 1; // Bars + Floor

        let (instance_buffer, instance_mem) = create_buffer(
            (size_of::<vk::AccelerationStructureInstanceKHR>() * total_instances) as u64,
            vk::BufferUsageFlags::ACCELERATION_STRUCTURE_BUILD_INPUT_READ_ONLY_KHR | vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
        );
        
        let instance_addr = vk::DeviceOrHostAddressConstKHR {
            device_address: device.get_buffer_device_address(&vk::BufferDeviceAddressInfo::builder().buffer(instance_buffer)),
        };

        let geometry_tlas = vk::AccelerationStructureGeometryKHR::builder()
            .geometry_type(vk::GeometryTypeKHR::INSTANCES)
            .geometry(vk::AccelerationStructureGeometryDataKHR {
                instances: vk::AccelerationStructureGeometryInstancesDataKHR::builder()
                    .data(instance_addr)
                    .build(),
            });

        let build_info_tlas = vk::AccelerationStructureBuildGeometryInfoKHR::builder()
            .ty(vk::AccelerationStructureTypeKHR::TOP_LEVEL)
            .flags(vk::BuildAccelerationStructureFlagsKHR::PREFER_FAST_TRACE | vk::BuildAccelerationStructureFlagsKHR::ALLOW_UPDATE)
            .geometries(std::slice::from_ref(&geometry_tlas));

        let size_info_tlas = as_loader.get_acceleration_structure_build_sizes(
            vk::AccelerationStructureBuildTypeKHR::DEVICE,
            &build_info_tlas,
            &[total_instances as u32],
        );

        let (tlas_buffer, _tlas_mem) = create_buffer(
            size_info_tlas.acceleration_structure_size,
            vk::BufferUsageFlags::ACCELERATION_STRUCTURE_STORAGE_KHR | vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS,
            vk::MemoryPropertyFlags::DEVICE_LOCAL,
        );
        let tlas = as_loader.create_acceleration_structure(
            &vk::AccelerationStructureCreateInfoKHR::builder()
                .buffer(tlas_buffer)
                .size(size_info_tlas.acceleration_structure_size)
                .ty(vk::AccelerationStructureTypeKHR::TOP_LEVEL),
            None,
        ).unwrap();

        let (scratch_buffer_tlas, _scratch_mem_tlas) = create_buffer(
            size_info_tlas.build_scratch_size,
            vk::BufferUsageFlags::STORAGE_BUFFER | vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS,
            vk::MemoryPropertyFlags::DEVICE_LOCAL,
        );
        let scratch_addr_tlas = device.get_buffer_device_address(&vk::BufferDeviceAddressInfo::builder().buffer(scratch_buffer_tlas));

        let mut build_info_tlas = build_info_tlas.dst_acceleration_structure(tlas).scratch_data(vk::DeviceOrHostAddressKHR { device_address: scratch_addr_tlas });
        let build_range_tlas = vk::AccelerationStructureBuildRangeInfoKHR::builder()
            .primitive_count(total_instances as u32)
            .primitive_offset(0)
            .first_vertex(0)
            .transform_offset(0);

        // Initial Instance Update
        {
             let ptr = device.map_memory(instance_mem, 0, (size_of::<vk::AccelerationStructureInstanceKHR>() * total_instances) as u64, vk::MemoryMapFlags::empty()).unwrap() as *mut u8;
             let blas_addr = as_loader.get_acceleration_structure_device_address(&vk::AccelerationStructureDeviceAddressInfoKHR::builder().acceleration_structure(blas));
             
             // Floor
             let transform_floor = vk::TransformMatrixKHR { matrix: [
                 50.0, 0.0, 0.0, 0.0,
                 0.0, 0.1, 0.0, -0.1,
                 0.0, 0.0, 50.0, 0.0,
             ]};
             let instance_floor = vk::AccelerationStructureInstanceKHR {
                 transform: transform_floor,
                 instance_custom_index_and_mask: vk::Packed24_8::new(0, 0xFF),
                 instance_shader_binding_table_record_offset_and_flags: vk::Packed24_8::new(0, vk::GeometryInstanceFlagsKHR::TRIANGLE_FACING_CULL_DISABLE.as_raw() as u8),
                 acceleration_structure_reference: vk::AccelerationStructureReferenceKHR { device_handle: blas_addr },
             };
             unsafe {
                 std::ptr::copy_nonoverlapping(&instance_floor as *const _ as *const u8, ptr, size_of::<vk::AccelerationStructureInstanceKHR>());
             }

             // Bars
             for (i, &val) in sort_system.values.iter().enumerate() {
                 let x = (i as f32 - NUM_BARS as f32 / 2.0) * 1.2;
                 let height = val as f32 / NUM_BARS as f32 * 5.0;
                 let transform = vk::TransformMatrixKHR { matrix: [
                     1.0, 0.0, 0.0, x,
                     0.0, height, 0.0, 0.0,
                     0.0, 0.0, 1.0, 0.0,
                 ]};
                 let instance = vk::AccelerationStructureInstanceKHR {
                     transform,
                     instance_custom_index_and_mask: vk::Packed24_8::new(val, 0xFF),
                     instance_shader_binding_table_record_offset_and_flags: vk::Packed24_8::new(0, vk::GeometryInstanceFlagsKHR::TRIANGLE_FACING_CULL_DISABLE.as_raw() as u8),
                     acceleration_structure_reference: vk::AccelerationStructureReferenceKHR { device_handle: blas_addr },
                 };
                 unsafe {
                     std::ptr::copy_nonoverlapping(&instance as *const _ as *const u8, ptr.add((i + 1) * size_of::<vk::AccelerationStructureInstanceKHR>()), size_of::<vk::AccelerationStructureInstanceKHR>());
                 }
             }
             device.unmap_memory(instance_mem);
        }

        device.reset_command_pool(command_pool, vk::CommandPoolResetFlags::empty()).unwrap();
        device.begin_command_buffer(command_buffer, &vk::CommandBufferBeginInfo::builder().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT)).unwrap();
        as_loader.cmd_build_acceleration_structures(command_buffer, &[build_info_tlas.build()], &[&[build_range_tlas.build()]]);
        device.end_command_buffer(command_buffer).unwrap();
        device.queue_submit(queue, &[vk::SubmitInfo::builder().command_buffers(&[command_buffer]).build()], vk::Fence::null()).unwrap();
        device.device_wait_idle().unwrap();

        // 3. Storage Image (Ray Tracing Output)
        let mut storage_image: vk::Image;
        let mut storage_mem: vk::DeviceMemory;
        let mut storage_view: vk::ImageView;

        {
            let image_info = vk::ImageCreateInfo::builder()
                .image_type(vk::ImageType::TYPE_2D)
                .format(vk::Format::R8G8B8A8_UNORM)
                .extent(vk::Extent3D { width: surface_caps.current_extent.width, height: surface_caps.current_extent.height, depth: 1 })
                .mip_levels(1)
                .array_layers(1)
                .samples(vk::SampleCountFlags::TYPE_1)
                .tiling(vk::ImageTiling::OPTIMAL)
                .usage(vk::ImageUsageFlags::STORAGE | vk::ImageUsageFlags::TRANSFER_SRC)
                .sharing_mode(vk::SharingMode::EXCLUSIVE)
                .initial_layout(vk::ImageLayout::UNDEFINED);
            storage_image = device.create_image(&image_info, None).unwrap();
            let req = device.get_image_memory_requirements(storage_image);
            let index = find_memorytype_index(&req, &memory_props, vk::MemoryPropertyFlags::DEVICE_LOCAL).unwrap();
            storage_mem = device.allocate_memory(&vk::MemoryAllocateInfo::builder().allocation_size(req.size).memory_type_index(index), None).unwrap();
            device.bind_image_memory(storage_image, storage_mem, 0).unwrap();
            
            storage_view = device.create_image_view(&vk::ImageViewCreateInfo::builder()
                .image(storage_image)
                .view_type(vk::ImageViewType::TYPE_2D)
                .format(vk::Format::R8G8B8A8_UNORM)
                .subresource_range(vk::ImageSubresourceRange {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    base_mip_level: 0,
                    level_count: 1,
                    base_array_layer: 0,
                    layer_count: 1,
                }), None).unwrap();
        }

        // 4. Descriptors
        let descriptor_pool = device.create_descriptor_pool(&vk::DescriptorPoolCreateInfo::builder()
            .max_sets(1)
            .pool_sizes(&[
                vk::DescriptorPoolSize { ty: vk::DescriptorType::ACCELERATION_STRUCTURE_KHR, descriptor_count: 1 },
                vk::DescriptorPoolSize { ty: vk::DescriptorType::STORAGE_IMAGE, descriptor_count: 1 },
                vk::DescriptorPoolSize { ty: vk::DescriptorType::UNIFORM_BUFFER, descriptor_count: 1 },
                vk::DescriptorPoolSize { ty: vk::DescriptorType::STORAGE_BUFFER, descriptor_count: 2 }, // Vertices + Indices
            ]), None).unwrap();

        let descriptor_set_layout = device.create_descriptor_set_layout(&vk::DescriptorSetLayoutCreateInfo::builder()
            .bindings(&[
                vk::DescriptorSetLayoutBinding::builder().binding(0).descriptor_type(vk::DescriptorType::ACCELERATION_STRUCTURE_KHR).descriptor_count(1).stage_flags(vk::ShaderStageFlags::RAYGEN_KHR | vk::ShaderStageFlags::CLOSEST_HIT_KHR).build(),
                vk::DescriptorSetLayoutBinding::builder().binding(1).descriptor_type(vk::DescriptorType::STORAGE_IMAGE).descriptor_count(1).stage_flags(vk::ShaderStageFlags::RAYGEN_KHR).build(),
                vk::DescriptorSetLayoutBinding::builder().binding(2).descriptor_type(vk::DescriptorType::UNIFORM_BUFFER).descriptor_count(1).stage_flags(vk::ShaderStageFlags::RAYGEN_KHR | vk::ShaderStageFlags::CLOSEST_HIT_KHR).build(),
                vk::DescriptorSetLayoutBinding::builder().binding(3).descriptor_type(vk::DescriptorType::STORAGE_BUFFER).descriptor_count(1).stage_flags(vk::ShaderStageFlags::CLOSEST_HIT_KHR).build(), // Vertices
                vk::DescriptorSetLayoutBinding::builder().binding(4).descriptor_type(vk::DescriptorType::STORAGE_BUFFER).descriptor_count(1).stage_flags(vk::ShaderStageFlags::CLOSEST_HIT_KHR).build(), // Indices
            ]), None).unwrap();

        let descriptor_set = device.allocate_descriptor_sets(&vk::DescriptorSetAllocateInfo::builder()
            .descriptor_pool(descriptor_pool)
            .set_layouts(&[descriptor_set_layout])).unwrap()[0];

        let mut as_write = vk::WriteDescriptorSetAccelerationStructureKHR::builder()
            .acceleration_structures(std::slice::from_ref(&tlas));
        
        let mut write_set_as = vk::WriteDescriptorSet::builder()
            .dst_set(descriptor_set)
            .dst_binding(0)
            .descriptor_type(vk::DescriptorType::ACCELERATION_STRUCTURE_KHR)
            .push_next(&mut as_write)
            .build();
        write_set_as.descriptor_count = 1;

        let write_sets = [
            write_set_as,
            vk::WriteDescriptorSet::builder()
                .dst_set(descriptor_set)
                .dst_binding(1)
                .descriptor_type(vk::DescriptorType::STORAGE_IMAGE)
                .image_info(&[vk::DescriptorImageInfo::builder().image_view(storage_view).image_layout(vk::ImageLayout::GENERAL).build()])
                .build(),
        ];
        // We need a camera buffer for binding 2
        let (camera_buffer, camera_mem) = create_buffer(
            128, // 2 mat4s
            vk::BufferUsageFlags::UNIFORM_BUFFER,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
        );
        let camera_write = vk::WriteDescriptorSet::builder()
            .dst_set(descriptor_set)
            .dst_binding(2)
            .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
            .buffer_info(&[vk::DescriptorBufferInfo::builder().buffer(camera_buffer).offset(0).range(128).build()])
            .build();

        let vertex_write = vk::WriteDescriptorSet::builder()
            .dst_set(descriptor_set)
            .dst_binding(3)
            .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
            .buffer_info(&[vk::DescriptorBufferInfo::builder().buffer(vertex_buffer).offset(0).range(vk::WHOLE_SIZE).build()])
            .build();

        let index_write = vk::WriteDescriptorSet::builder()
            .dst_set(descriptor_set)
            .dst_binding(4)
            .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
            .buffer_info(&[vk::DescriptorBufferInfo::builder().buffer(index_buffer).offset(0).range(vk::WHOLE_SIZE).build()])
            .build();
        
        device.update_descriptor_sets(&[write_sets[0], write_sets[1], camera_write, vertex_write, index_write], &[]);

        // 5. Pipeline
        let compiler = shaderc::Compiler::new().unwrap();
        let mut options = shaderc::CompileOptions::new().unwrap();
        options.set_target_env(shaderc::TargetEnv::Vulkan, shaderc::EnvVersion::Vulkan1_2 as u32);
        
        let compile = |source: &str, kind: shaderc::ShaderKind| -> vk::ShaderModule {
            let binary = compiler.compile_into_spirv(source, kind, "shader.glsl", "main", Some(&options)).unwrap();
            device.create_shader_module(&vk::ShaderModuleCreateInfo::builder().code(binary.as_binary()), None).unwrap()
        };

        let rgen_module = compile(include_str!("shaders/raygen.glsl"), shaderc::ShaderKind::RayGeneration);
        let rmiss_module = compile(include_str!("shaders/miss.glsl"), shaderc::ShaderKind::Miss);
        let rchit_module = compile(include_str!("shaders/closesthit.glsl"), shaderc::ShaderKind::ClosestHit);

        let shader_stages = [
            vk::PipelineShaderStageCreateInfo::builder().stage(vk::ShaderStageFlags::RAYGEN_KHR).module(rgen_module).name(CStr::from_bytes_with_nul(b"main\0").unwrap()).build(),
            vk::PipelineShaderStageCreateInfo::builder().stage(vk::ShaderStageFlags::MISS_KHR).module(rmiss_module).name(CStr::from_bytes_with_nul(b"main\0").unwrap()).build(),
            vk::PipelineShaderStageCreateInfo::builder().stage(vk::ShaderStageFlags::CLOSEST_HIT_KHR).module(rchit_module).name(CStr::from_bytes_with_nul(b"main\0").unwrap()).build(),
        ];

        let shader_groups = [
            vk::RayTracingShaderGroupCreateInfoKHR::builder().ty(vk::RayTracingShaderGroupTypeKHR::GENERAL).general_shader(0).closest_hit_shader(vk::SHADER_UNUSED_KHR).any_hit_shader(vk::SHADER_UNUSED_KHR).intersection_shader(vk::SHADER_UNUSED_KHR).build(),
            vk::RayTracingShaderGroupCreateInfoKHR::builder().ty(vk::RayTracingShaderGroupTypeKHR::GENERAL).general_shader(1).closest_hit_shader(vk::SHADER_UNUSED_KHR).any_hit_shader(vk::SHADER_UNUSED_KHR).intersection_shader(vk::SHADER_UNUSED_KHR).build(),
            vk::RayTracingShaderGroupCreateInfoKHR::builder().ty(vk::RayTracingShaderGroupTypeKHR::TRIANGLES_HIT_GROUP).general_shader(vk::SHADER_UNUSED_KHR).closest_hit_shader(2).any_hit_shader(vk::SHADER_UNUSED_KHR).intersection_shader(vk::SHADER_UNUSED_KHR).build(),
        ];

        let push_constant_ranges = [
            vk::PushConstantRange::builder()
                .stage_flags(vk::ShaderStageFlags::RAYGEN_KHR | vk::ShaderStageFlags::CLOSEST_HIT_KHR | vk::ShaderStageFlags::MISS_KHR)
                .offset(0)
                .size(4) // float time
                .build()
        ];

        let pipeline_layout = device.create_pipeline_layout(&vk::PipelineLayoutCreateInfo::builder()
            .set_layouts(&[descriptor_set_layout])
            .push_constant_ranges(&push_constant_ranges), None).unwrap();
        
        let pipeline = rt_pipeline_loader.create_ray_tracing_pipelines(vk::DeferredOperationKHR::null(), vk::PipelineCache::null(), &[vk::RayTracingPipelineCreateInfoKHR::builder()
            .stages(&shader_stages)
            .groups(&shader_groups)
            .max_pipeline_ray_recursion_depth(6) // Reduced recursion depth to prevent TDR
            .layout(pipeline_layout)
            .build()], None).unwrap()[0];

        // 6. SBT
        let rt_props = vk::PhysicalDeviceRayTracingPipelinePropertiesKHR::builder();
        let mut _props = vk::PhysicalDeviceProperties2::builder().push_next(&mut rt_props.clone()); 
        let mut rt_pipeline_properties = vk::PhysicalDeviceRayTracingPipelinePropertiesKHR::default();
        let mut properties2 = vk::PhysicalDeviceProperties2::builder().push_next(&mut rt_pipeline_properties);
        instance.get_physical_device_properties2(pdevice, &mut properties2);
        
        let handle_size = rt_pipeline_properties.shader_group_handle_size;
        let handle_alignment = rt_pipeline_properties.shader_group_base_alignment;
        let group_count = shader_groups.len() as u32;
        let sbt_size = (group_count * handle_alignment) as u64;

        let (sbt_buffer, sbt_mem) = create_buffer(
            sbt_size,
            vk::BufferUsageFlags::SHADER_BINDING_TABLE_KHR | vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
        );
        
        let handles_data = rt_pipeline_loader.get_ray_tracing_shader_group_handles(pipeline, 0, group_count, group_count as usize * handle_size as usize).unwrap();
        let ptr = device.map_memory(sbt_mem, 0, sbt_size, vk::MemoryMapFlags::empty()).unwrap() as *mut u8;
        
        for i in 0..group_count {
            std::ptr::copy_nonoverlapping(
                handles_data.as_ptr().add(i as usize * handle_size as usize),
                ptr.add(i as usize * handle_alignment as usize),
                handle_size as usize
            );
        }
        device.unmap_memory(sbt_mem);
        
        let sbt_addr = device.get_buffer_device_address(&vk::BufferDeviceAddressInfo::builder().buffer(sbt_buffer));
        let rgen_region = vk::StridedDeviceAddressRegionKHR { device_address: sbt_addr, stride: handle_alignment as u64, size: handle_alignment as u64 };
        let miss_region = vk::StridedDeviceAddressRegionKHR { device_address: sbt_addr + handle_alignment as u64, stride: handle_alignment as u64, size: handle_alignment as u64 };
        let hit_region = vk::StridedDeviceAddressRegionKHR { device_address: sbt_addr + handle_alignment as u64 * 2, stride: handle_alignment as u64, size: handle_alignment as u64 };
        let call_region = vk::StridedDeviceAddressRegionKHR::default();

        // Update Camera
        // Simple look at
        let view = glam::Mat4::look_at_rh(glam::vec3(0.0, 12.0, 25.0), glam::vec3(0.0, 4.0, 0.0), glam::vec3(0.0, 1.0, 0.0));
        let mut proj = glam::Mat4::perspective_rh(45.0f32.to_radians(), surface_caps.current_extent.width as f32 / surface_caps.current_extent.height as f32, 0.1, 100.0);
        let mut cam_data = [view.inverse(), proj.inverse()];
        let ptr = device.map_memory(camera_mem, 0, 128, vk::MemoryMapFlags::empty()).unwrap();
        std::ptr::copy_nonoverlapping(cam_data.as_ptr() as *const u8, ptr as *mut u8, 128);
        device.unmap_memory(camera_mem);

        // Main Loop
        let fence = device.create_fence(&vk::FenceCreateInfo::builder().flags(vk::FenceCreateFlags::SIGNALED), None).unwrap();
        let semaphore = device.create_semaphore(&vk::SemaphoreCreateInfo::default(), None).unwrap();
        let render_semaphore = device.create_semaphore(&vk::SemaphoreCreateInfo::default(), None).unwrap();

        let mut need_resize = false;
        let start_time = std::time::Instant::now();

        event_loop.run(move |event, elwt| {
            elwt.set_control_flow(ControlFlow::Poll);
            match event {
                Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => elwt.exit(),
                Event::AboutToWait => window.request_redraw(),
                Event::WindowEvent { event: WindowEvent::Resized(_), .. } => {
                    need_resize = true;
                }
                Event::WindowEvent { event: WindowEvent::RedrawRequested, .. } => {
                    if need_resize {
                        device.device_wait_idle().unwrap();
                        
                        // Destroy old resources
                        device.destroy_image_view(storage_view, None);
                        device.destroy_image(storage_image, None);
                        device.free_memory(storage_mem, None);
                        swapchain_loader.destroy_swapchain(swapchain, None);

                        // Recreate Swapchain
                        surface_caps = surface_loader.get_physical_device_surface_capabilities(pdevice, surface).unwrap();
                        swapchain_create_info.image_extent = surface_caps.current_extent;
                        swapchain_create_info.pre_transform = surface_caps.current_transform;
                        swapchain = swapchain_loader.create_swapchain(&swapchain_create_info, None).unwrap();
                        present_images = swapchain_loader.get_swapchain_images(swapchain).unwrap();

                        // Recreate Storage Image
                        let image_info = vk::ImageCreateInfo::builder()
                            .image_type(vk::ImageType::TYPE_2D)
                            .format(vk::Format::R8G8B8A8_UNORM)
                            .extent(vk::Extent3D { width: surface_caps.current_extent.width, height: surface_caps.current_extent.height, depth: 1 })
                            .mip_levels(1)
                            .array_layers(1)
                            .samples(vk::SampleCountFlags::TYPE_1)
                            .tiling(vk::ImageTiling::OPTIMAL)
                            .usage(vk::ImageUsageFlags::STORAGE | vk::ImageUsageFlags::TRANSFER_SRC)
                            .sharing_mode(vk::SharingMode::EXCLUSIVE)
                            .initial_layout(vk::ImageLayout::UNDEFINED);
                        storage_image = device.create_image(&image_info, None).unwrap();
                        let req = device.get_image_memory_requirements(storage_image);
                        let index = find_memorytype_index(&req, &memory_props, vk::MemoryPropertyFlags::DEVICE_LOCAL).unwrap();
                        storage_mem = device.allocate_memory(&vk::MemoryAllocateInfo::builder().allocation_size(req.size).memory_type_index(index), None).unwrap();
                        device.bind_image_memory(storage_image, storage_mem, 0).unwrap();
                        
                        storage_view = device.create_image_view(&vk::ImageViewCreateInfo::builder()
                            .image(storage_image)
                            .view_type(vk::ImageViewType::TYPE_2D)
                            .format(vk::Format::R8G8B8A8_UNORM)
                            .subresource_range(vk::ImageSubresourceRange {
                                aspect_mask: vk::ImageAspectFlags::COLOR,
                                base_mip_level: 0,
                                level_count: 1,
                                base_array_layer: 0,
                                layer_count: 1,
                            }), None).unwrap();

                        // Update Descriptor Set
                        let write_set = vk::WriteDescriptorSet::builder()
                            .dst_set(descriptor_set)
                            .dst_binding(1)
                            .descriptor_type(vk::DescriptorType::STORAGE_IMAGE)
                            .image_info(&[vk::DescriptorImageInfo::builder().image_view(storage_view).image_layout(vk::ImageLayout::GENERAL).build()])
                            .build();
                        device.update_descriptor_sets(&[write_set], &[]);

                        // Update Camera
                        let proj = glam::Mat4::perspective_rh(45.0f32.to_radians(), surface_caps.current_extent.width as f32 / surface_caps.current_extent.height as f32, 0.1, 100.0);
                        let cam_data = [view.inverse(), proj.inverse()];
                        let ptr = device.map_memory(camera_mem, 0, 128, vk::MemoryMapFlags::empty()).unwrap();
                        std::ptr::copy_nonoverlapping(cam_data.as_ptr() as *const u8, ptr as *mut u8, 128);
                        device.unmap_memory(camera_mem);

                        need_resize = false;
                    }

                    device.wait_for_fences(&[fence], true, u64::MAX).unwrap();
                    device.reset_fences(&[fence]).unwrap();

                    let (index, is_suboptimal) = match swapchain_loader.acquire_next_image(swapchain, u64::MAX, semaphore, vk::Fence::null()) {
                        Ok(r) => r,
                        Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                            need_resize = true;
                            return;
                        }
                        Err(e) => panic!("{}", e),
                    };

                    if is_suboptimal {
                        need_resize = true;
                    }

                    // Update Sort & Instances
                    sort_system.step();
                    {
                         let ptr = device.map_memory(instance_mem, 0, (size_of::<vk::AccelerationStructureInstanceKHR>() * total_instances) as u64, vk::MemoryMapFlags::empty()).unwrap() as *mut u8;
                         let blas_addr = as_loader.get_acceleration_structure_device_address(&vk::AccelerationStructureDeviceAddressInfoKHR::builder().acceleration_structure(blas));
                         
                         // Floor
                         let transform_floor = vk::TransformMatrixKHR { matrix: [
                             50.0, 0.0, 0.0, 0.0,
                             0.0, 0.1, 0.0, -0.1,
                             0.0, 0.0, 50.0, 0.0,
                         ]};
                         let instance_floor = vk::AccelerationStructureInstanceKHR {
                             transform: transform_floor,
                             instance_custom_index_and_mask: vk::Packed24_8::new(0, 0xFF),
                             instance_shader_binding_table_record_offset_and_flags: vk::Packed24_8::new(0, vk::GeometryInstanceFlagsKHR::TRIANGLE_FACING_CULL_DISABLE.as_raw() as u8),
                             acceleration_structure_reference: vk::AccelerationStructureReferenceKHR { device_handle: blas_addr },
                         };
                         unsafe {
                             std::ptr::copy_nonoverlapping(&instance_floor as *const _ as *const u8, ptr, size_of::<vk::AccelerationStructureInstanceKHR>());
                         }

                         // Bars
                         for (i, &val) in sort_system.values.iter().enumerate() {
                             let x = (i as f32 - NUM_BARS as f32 / 2.0) * 1.2;
                             let height = val as f32 / NUM_BARS as f32 * 5.0;
                             let transform = vk::TransformMatrixKHR { matrix: [
                                 1.0, 0.0, 0.0, x,
                                 0.0, height, 0.0, 0.0,
                                 0.0, 0.0, 1.0, 0.0,
                             ]};
                             let instance = vk::AccelerationStructureInstanceKHR {
                                 transform,
                                 instance_custom_index_and_mask: vk::Packed24_8::new(val, 0xFF),
                                 instance_shader_binding_table_record_offset_and_flags: vk::Packed24_8::new(0, vk::GeometryInstanceFlagsKHR::TRIANGLE_FACING_CULL_DISABLE.as_raw() as u8),
                                 acceleration_structure_reference: vk::AccelerationStructureReferenceKHR { device_handle: blas_addr },
                             };
                             unsafe {
                                 std::ptr::copy_nonoverlapping(&instance as *const _ as *const u8, ptr.add((i + 1) * size_of::<vk::AccelerationStructureInstanceKHR>()), size_of::<vk::AccelerationStructureInstanceKHR>());
                             }
                         }
                         device.unmap_memory(instance_mem);
                    }

                    device.reset_command_pool(command_pool, vk::CommandPoolResetFlags::empty()).unwrap();
                    device.begin_command_buffer(command_buffer, &vk::CommandBufferBeginInfo::builder().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT)).unwrap();

                    // Rebuild TLAS
                    let geometry_tlas_loop = vk::AccelerationStructureGeometryKHR::builder()
                        .geometry_type(vk::GeometryTypeKHR::INSTANCES)
                        .geometry(vk::AccelerationStructureGeometryDataKHR {
                            instances: vk::AccelerationStructureGeometryInstancesDataKHR::builder()
                                .data(instance_addr)
                                .build(),
                        });
                    
                    let build_info_tlas_loop = vk::AccelerationStructureBuildGeometryInfoKHR::builder()
                        .ty(vk::AccelerationStructureTypeKHR::TOP_LEVEL)
                        .flags(vk::BuildAccelerationStructureFlagsKHR::PREFER_FAST_TRACE | vk::BuildAccelerationStructureFlagsKHR::ALLOW_UPDATE)
                        .dst_acceleration_structure(tlas)
                        .scratch_data(vk::DeviceOrHostAddressKHR { device_address: scratch_addr_tlas })
                        .geometries(std::slice::from_ref(&geometry_tlas_loop));

                    let build_range_tlas_loop = vk::AccelerationStructureBuildRangeInfoKHR::builder()
                        .primitive_count(total_instances as u32)
                        .primitive_offset(0)
                        .first_vertex(0)
                        .transform_offset(0);

                    as_loader.cmd_build_acceleration_structures(command_buffer, &[build_info_tlas_loop.build()], &[&[build_range_tlas_loop.build()]]);

                    // Barrier: TLAS Build -> Ray Tracing
                    let memory_barrier = vk::MemoryBarrier::builder()
                        .src_access_mask(vk::AccessFlags::ACCELERATION_STRUCTURE_WRITE_KHR)
                        .dst_access_mask(vk::AccessFlags::ACCELERATION_STRUCTURE_READ_KHR);
                    device.cmd_pipeline_barrier(command_buffer, vk::PipelineStageFlags::ACCELERATION_STRUCTURE_BUILD_KHR, vk::PipelineStageFlags::RAY_TRACING_SHADER_KHR, vk::DependencyFlags::empty(), &[memory_barrier.build()], &[], &[]);

                    // Transition storage image to GENERAL
                    let barrier = vk::ImageMemoryBarrier::builder()
                        .old_layout(vk::ImageLayout::UNDEFINED)
                        .new_layout(vk::ImageLayout::GENERAL)
                        .image(storage_image)
                        .subresource_range(vk::ImageSubresourceRange { aspect_mask: vk::ImageAspectFlags::COLOR, base_mip_level: 0, level_count: 1, base_array_layer: 0, layer_count: 1 })
                        .src_access_mask(vk::AccessFlags::empty())
                        .dst_access_mask(vk::AccessFlags::SHADER_WRITE);
                    device.cmd_pipeline_barrier(command_buffer, vk::PipelineStageFlags::TOP_OF_PIPE, vk::PipelineStageFlags::RAY_TRACING_SHADER_KHR, vk::DependencyFlags::empty(), &[], &[], &[barrier.build()]);

                    // Trace Rays
                    let time = start_time.elapsed().as_secs_f32();
                    device.cmd_push_constants(command_buffer, pipeline_layout, vk::ShaderStageFlags::RAYGEN_KHR | vk::ShaderStageFlags::CLOSEST_HIT_KHR | vk::ShaderStageFlags::MISS_KHR, 0, &time.to_ne_bytes());

                    device.cmd_bind_pipeline(command_buffer, vk::PipelineBindPoint::RAY_TRACING_KHR, pipeline);
                    device.cmd_bind_descriptor_sets(command_buffer, vk::PipelineBindPoint::RAY_TRACING_KHR, pipeline_layout, 0, &[descriptor_set], &[]);
                    rt_pipeline_loader.cmd_trace_rays(command_buffer, &rgen_region, &miss_region, &hit_region, &call_region, surface_caps.current_extent.width, surface_caps.current_extent.height, 1);

                    // Transition storage image to TRANSFER_SRC
                    let barrier = vk::ImageMemoryBarrier::builder()
                        .old_layout(vk::ImageLayout::GENERAL)
                        .new_layout(vk::ImageLayout::TRANSFER_SRC_OPTIMAL)
                        .image(storage_image)
                        .subresource_range(vk::ImageSubresourceRange { aspect_mask: vk::ImageAspectFlags::COLOR, base_mip_level: 0, level_count: 1, base_array_layer: 0, layer_count: 1 })
                        .src_access_mask(vk::AccessFlags::SHADER_WRITE)
                        .dst_access_mask(vk::AccessFlags::TRANSFER_READ);
                    device.cmd_pipeline_barrier(command_buffer, vk::PipelineStageFlags::RAY_TRACING_SHADER_KHR, vk::PipelineStageFlags::TRANSFER, vk::DependencyFlags::empty(), &[], &[], &[barrier.build()]);

                    // Transition swapchain image to TRANSFER_DST
                    let barrier = vk::ImageMemoryBarrier::builder()
                        .old_layout(vk::ImageLayout::UNDEFINED)
                        .new_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                        .image(present_images[index as usize])
                        .subresource_range(vk::ImageSubresourceRange { aspect_mask: vk::ImageAspectFlags::COLOR, base_mip_level: 0, level_count: 1, base_array_layer: 0, layer_count: 1 })
                        .src_access_mask(vk::AccessFlags::empty())
                        .dst_access_mask(vk::AccessFlags::TRANSFER_WRITE);
                    device.cmd_pipeline_barrier(command_buffer, vk::PipelineStageFlags::TOP_OF_PIPE, vk::PipelineStageFlags::TRANSFER, vk::DependencyFlags::empty(), &[], &[], &[barrier.build()]);

                    // Copy
                    let copy_region = vk::ImageCopy::builder()
                        .src_subresource(vk::ImageSubresourceLayers { aspect_mask: vk::ImageAspectFlags::COLOR, mip_level: 0, base_array_layer: 0, layer_count: 1 })
                        .dst_subresource(vk::ImageSubresourceLayers { aspect_mask: vk::ImageAspectFlags::COLOR, mip_level: 0, base_array_layer: 0, layer_count: 1 })
                        .extent(vk::Extent3D { width: surface_caps.current_extent.width, height: surface_caps.current_extent.height, depth: 1 });
                    device.cmd_copy_image(command_buffer, storage_image, vk::ImageLayout::TRANSFER_SRC_OPTIMAL, present_images[index as usize], vk::ImageLayout::TRANSFER_DST_OPTIMAL, &[copy_region.build()]);

                    // Transition swapchain to PRESENT
                    let barrier = vk::ImageMemoryBarrier::builder()
                        .old_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                        .new_layout(vk::ImageLayout::PRESENT_SRC_KHR)
                        .image(present_images[index as usize])
                        .subresource_range(vk::ImageSubresourceRange { aspect_mask: vk::ImageAspectFlags::COLOR, base_mip_level: 0, level_count: 1, base_array_layer: 0, layer_count: 1 })
                        .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
                        .dst_access_mask(vk::AccessFlags::empty());
                    device.cmd_pipeline_barrier(command_buffer, vk::PipelineStageFlags::TRANSFER, vk::PipelineStageFlags::BOTTOM_OF_PIPE, vk::DependencyFlags::empty(), &[], &[], &[barrier.build()]);

                    device.end_command_buffer(command_buffer).unwrap();

                    device.queue_submit(queue, &[vk::SubmitInfo::builder().wait_semaphores(&[semaphore]).wait_dst_stage_mask(&[vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT]).command_buffers(&[command_buffer]).signal_semaphores(&[render_semaphore]).build()], fence).unwrap();

                    let present_result = swapchain_loader.queue_present(queue, &vk::PresentInfoKHR::builder().wait_semaphores(&[render_semaphore]).swapchains(&[swapchain]).image_indices(&[index]).build());
                    match present_result {
                        Ok(is_suboptimal) => {
                            if is_suboptimal {
                                need_resize = true;
                            }
                        }
                        Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                            need_resize = true;
                        }
                        Err(e) => panic!("{}", e),
                    }
                }
                _ => {}
            }
        }).unwrap();
    }
}
