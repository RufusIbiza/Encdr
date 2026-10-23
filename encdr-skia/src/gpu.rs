use std::sync::Arc;

use ash::vk::Handle;
use skia_safe::gpu::{self, DirectContext};
use skia_safe::{AlphaType, ColorSpace, ColorType, ImageInfo, Surface};

/// Vulkan context for offscreen GPU rendering with ash.
pub struct VulkanContext {
    pub entry: ash::Entry,
    pub instance: ash::Instance,
    pub physical_device: ash::vk::PhysicalDevice,
    pub device: ash::Device,
    pub queue: ash::vk::Queue,
    pub queue_family_index: u32,
}

impl VulkanContext {
    /// Try to initialize a headless Vulkan context with graphics support.
    pub fn new() -> Result<Self, String> {
        let entry = unsafe { ash::Entry::load() }
            .map_err(|e| format!("Failed to load Vulkan library: {e}"))?;

        let app_name = std::ffi::CString::new("encdr-skia").unwrap();
        let app_info = ash::vk::ApplicationInfo::default()
            .application_name(&app_name)
            .application_version(ash::vk::make_api_version(0, 1, 0, 0))
            .engine_name(&app_name)
            .engine_version(ash::vk::make_api_version(0, 1, 0, 0))
            .api_version(ash::vk::API_VERSION_1_1);

        let create_info = ash::vk::InstanceCreateInfo::default().application_info(&app_info);

        let instance = unsafe { entry.create_instance(&create_info, None) }
            .map_err(|e| format!("Failed to create Vulkan instance: {e}"))?;

        let pdevices = unsafe { instance.enumerate_physical_devices() }
            .map_err(|e| format!("Failed to enumerate physical devices: {e}"))?;

        if pdevices.is_empty() {
            unsafe { instance.destroy_instance(None) };
            return Err("No Vulkan physical devices found".into());
        }

        // Find a device with a graphics queue family
        let mut selected_device = None;
        let mut selected_queue_family = None;

        for pdevice in pdevices {
            let props = unsafe { instance.get_physical_device_queue_family_properties(pdevice) };
            for (idx, qf) in props.iter().enumerate() {
                if qf.queue_flags.contains(ash::vk::QueueFlags::GRAPHICS) {
                    selected_device = Some(pdevice);
                    selected_queue_family = Some(idx as u32);
                    break;
                }
            }
            if selected_device.is_some() {
                break;
            }
        }

        let (pdevice, queue_family_index) = match (selected_device, selected_queue_family) {
            (Some(dev), Some(qf)) => (dev, qf),
            _ => {
                unsafe { instance.destroy_instance(None) };
                return Err("No Vulkan physical device with graphics queue found".into());
            }
        };

        let queue_priorities = [1.0f32];
        let queue_info = ash::vk::DeviceQueueCreateInfo::default()
            .queue_family_index(queue_family_index)
            .queue_priorities(&queue_priorities);

        let queue_infos = [queue_info];
        let device_create_info =
            ash::vk::DeviceCreateInfo::default().queue_create_infos(&queue_infos);

        let device = unsafe { instance.create_device(pdevice, &device_create_info, None) }
            .map_err(|e| {
                unsafe { instance.destroy_instance(None) };
                format!("Failed to create Vulkan device: {e}")
            })?;

        let queue = unsafe { device.get_device_queue(queue_family_index, 0) };

        Ok(Self {
            entry,
            instance,
            physical_device: pdevice,
            device,
            queue,
            queue_family_index,
        })
    }
}

impl Drop for VulkanContext {
    fn drop(&mut self) {
        unsafe {
            self.device.destroy_device(None);
            self.instance.destroy_instance(None);
        }
    }
}

/// GPU context manager for Skia.
///
/// Encapsulates the Vulkan backend and Skia `DirectContext`. If Vulkan is
/// unavailable or initialization fails, falls back gracefully to CPU rasterization.
pub struct SkiaGpuContext {
    /// Skia direct context (must drop BEFORE vulkan context)
    direct_context: Option<DirectContext>,
    /// Underlying Vulkan instance/device handles
    #[allow(dead_code)]
    vulkan: Option<Arc<VulkanContext>>,
}

impl SkiaGpuContext {
    /// Create a new Skia GPU context, attempting Vulkan hardware acceleration first.
    ///
    /// If Vulkan fails, a warning is logged and the context operates in CPU fallback mode.
    pub fn new() -> Self {
        match VulkanContext::new() {
            Ok(vk_ctx) => {
                let entry_clone = vk_ctx.entry.clone();
                let instance_clone = vk_ctx.instance.clone();

                let get_proc = move |of: gpu::vk::GetProcOf| -> gpu::vk::GetProcResult {
                    match of {
                        gpu::vk::GetProcOf::Instance(inst, name) => unsafe {
                            let ash_inst = ash::vk::Instance::from_raw(inst as _);
                            match entry_clone.get_instance_proc_addr(ash_inst, name) {
                                Some(f) => f as _,
                                None => std::ptr::null(),
                            }
                        },
                        gpu::vk::GetProcOf::Device(dev, name) => unsafe {
                            let ash_dev = ash::vk::Device::from_raw(dev as _);
                            match instance_clone.get_device_proc_addr(ash_dev, name) {
                                Some(f) => f as _,
                                None => std::ptr::null(),
                            }
                        },
                    }
                };

                let backend_context = unsafe {
                    gpu::vk::BackendContext::new_builder(
                        vk_ctx.instance.handle().as_raw() as _,
                        vk_ctx.physical_device.as_raw() as _,
                        vk_ctx.device.handle().as_raw() as _,
                        (vk_ctx.queue.as_raw() as _, vk_ctx.queue_family_index as usize),
                        &get_proc,
                        None,
                    )
                    .build()
                };

                match gpu::direct_contexts::make_vulkan(&backend_context, None) {
                    Some(direct_context) => {
                        tracing::info!("Skia GPU acceleration initialized successfully via Vulkan");
                        Self {
                            direct_context: Some(direct_context),
                            vulkan: Some(Arc::new(vk_ctx)),
                        }
                    }
                    None => {
                        tracing::warn!("Failed to create Skia Vulkan DirectContext, falling back to CPU");
                        Self {
                            direct_context: None,
                            vulkan: None,
                        }
                    }
                }
            }
            Err(e) => {
                tracing::warn!("Vulkan initialization failed ({}), falling back to CPU Skia rendering", e);
                Self {
                    direct_context: None,
                    vulkan: None,
                }
            }
        }
    }

    /// Create an explicit CPU-only context (useful for testing or benchmarking).
    pub fn cpu() -> Self {
        Self {
            direct_context: None,
            vulkan: None,
        }
    }

    /// Returns `true` if this context is hardware GPU accelerated via Vulkan.
    pub fn is_gpu_accelerated(&self) -> bool {
        self.direct_context.is_some()
    }

    /// Creates an offscreen Skia surface matching the given dimensions.
    ///
    /// If GPU accelerated, allocates a GPU render target. Otherwise allocates
    /// a CPU raster surface.
    pub fn create_surface(&mut self, width: i32, height: i32) -> Result<Surface, String> {
        let image_info = ImageInfo::new(
            (width, height),
            ColorType::RGBA8888,
            AlphaType::Premul,
            Some(ColorSpace::new_srgb()),
        );

        if let Some(ref mut d_ctx) = self.direct_context {
            if let Some(surface) = gpu::surfaces::render_target(
                d_ctx,
                gpu::Budgeted::Yes,
                &image_info,
                0,
                gpu::SurfaceOrigin::TopLeft,
                None,
                false,
                false,
            ) {
                return Ok(surface);
            }
            tracing::warn!("Failed to create GPU render target surface, falling back to CPU raster surface");
        }

        skia_safe::surfaces::raster_n32_premul((width, height))
            .ok_or_else(|| format!("Failed to create Skia CPU surface for {}x{}", width, height))
    }

    /// Flush pending GPU rendering commands.
    pub fn flush_and_submit(&mut self, surface: &mut Surface) {
        if let Some(ref mut d_ctx) = self.direct_context {
            d_ctx.flush_surface(surface);
            d_ctx.submit(gpu::SyncCpu::No);
        }
    }
}

impl Default for SkiaGpuContext {
    fn default() -> Self {
        Self::new()
    }
}
