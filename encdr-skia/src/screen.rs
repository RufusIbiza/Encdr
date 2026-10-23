use std::sync::{Arc, Mutex};

use encdr::core::descriptor::PixelFormat;
use encdr::core::event::DeviceId;
use encdr::Encdr;
use skia_safe::{AlphaType, Canvas, ColorSpace, ColorType, ImageInfo, Surface};

use crate::gpu::SkiaGpuContext;

/// A Skia-backed hardware screen renderer for a single device screen.
///
/// Manages an offscreen GPU-accelerated Skia surface sized to the device screen's native resolution.
/// Drawing commands rendered to the canvas are flushed on the GPU, transferred to the pixel buffer,
/// and submitted directly to encdr's USB screen transfer pipeline.
pub struct SkiaScreenView {
    gpu: Arc<Mutex<SkiaGpuContext>>,
    surface: Surface,
    device_id: DeviceId,
    screen_name: String,
    width: u32,
    height: u32,
    pixel_buf: Vec<u8>,
}

impl SkiaScreenView {
    /// Create a new Skia-backed screen by automatically using the primary/first screen of the device.
    pub fn for_device(encdr: &Encdr, device_id: DeviceId) -> Result<Self, String> {
        let descriptor = encdr
            .device_descriptor(device_id)
            .ok_or_else(|| format!("Device {:?} not connected", device_id))?;

        if let Some(screen) = descriptor.screens.first() {
            let name = screen.name.clone();
            Self::new(encdr, device_id, &name)
        } else {
            Err(format!("Device {:?} has no screens", device_id))
        }
    }

    /// Create a new Skia-backed screen for a connected device using a dedicated GPU context.
    pub fn new(encdr: &Encdr, device_id: DeviceId, screen_name: &str) -> Result<Self, String> {
        let gpu = Arc::new(Mutex::new(SkiaGpuContext::new()));
        Self::with_gpu_context(encdr, device_id, screen_name, gpu)
    }

    /// Create a new Skia-backed screen using an existing shared GPU context.
    pub fn with_gpu_context(
        encdr: &Encdr,
        device_id: DeviceId,
        screen_name: &str,
        gpu: Arc<Mutex<SkiaGpuContext>>,
    ) -> Result<Self, String> {
        let descriptor = encdr
            .device_descriptor(device_id)
            .ok_or_else(|| format!("Device {:?} not connected", device_id))?;

        let screen_desc = descriptor
            .screens
            .iter()
            .find(|s| s.name == screen_name || (descriptor.screens.len() == 1))
            .ok_or_else(|| format!("Screen '{}' not found on device", screen_name))?;

        let width = screen_desc.width as u32;
        let height = screen_desc.height as u32;

        let surface = {
            let mut gpu_lock = gpu.lock().map_err(|e| e.to_string())?;
            gpu_lock.create_surface(width as i32, height as i32)?
        };

        let pixel_buf = vec![0u8; (width * height * 4) as usize];

        Ok(Self {
            gpu,
            surface,
            device_id,
            screen_name: screen_desc.name.clone(),
            width,
            height,
            pixel_buf,
        })
    }

    /// Access the Skia canvas for direct rendering.
    pub fn canvas(&mut self) -> &Canvas {
        self.surface.canvas()
    }

    /// Render a frame with a closure and immediately flush and submit it to the device screen.
    pub fn draw<F>(&mut self, encdr: &Encdr, f: F) -> Result<(), String>
    where
        F: FnOnce(&Canvas),
    {
        f(self.surface.canvas());
        self.submit(encdr)
    }

    /// Flush GPU rendering commands, read pixels, and submit the frame to Encdr for USB transfer.
    ///
    /// If another application currently holds hardware control, submission is safely skipped.
    pub fn submit(&mut self, encdr: &Encdr) -> Result<(), String> {
        if !encdr.has_control(self.device_id) {
            tracing::trace!("Skipping Skia screen submit: another application has the hardware");
            return Ok(());
        }

        // Flush commands to GPU
        {
            let mut gpu_lock = self.gpu.lock().map_err(|e| e.to_string())?;
            gpu_lock.flush_and_submit(&mut self.surface);
        }

        // Read pixels
        let image_info = ImageInfo::new(
            (self.width as i32, self.height as i32),
            ColorType::RGBA8888,
            AlphaType::Premul,
            Some(ColorSpace::new_srgb()),
        );

        let row_bytes = (self.width as usize) * 4;
        if !self
            .surface
            .read_pixels(&image_info, &mut self.pixel_buf, row_bytes, (0, 0))
        {
            return Err("Failed to read pixels from Skia surface".into());
        }

        // Submit to encdr pipeline
        encdr.submit_screen_with_format(
            self.device_id,
            &self.screen_name,
            &self.pixel_buf,
            PixelFormat::Rgba8888,
        );

        Ok(())
    }

    /// Width of the screen in pixels.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Height of the screen in pixels.
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Returns `true` if this view's surface is hardware GPU accelerated.
    pub fn is_gpu_accelerated(&self) -> bool {
        self.gpu
            .lock()
            .map(|g| g.is_gpu_accelerated())
            .unwrap_or(false)
    }

    /// Device ID this view is attached to.
    pub fn device_id(&self) -> DeviceId {
        self.device_id
    }

    /// Screen name this view renders to.
    pub fn screen_name(&self) -> &str {
        &self.screen_name
    }
}
