use std::sync::{Arc, Mutex};

use encdr::core::descriptor::PixelFormat;
use encdr::core::event::DeviceId;
use encdr::Encdr;
use skia_safe::{AlphaType, Canvas, ColorSpace, ColorType, ImageInfo, Surface};

use crate::gpu::SkiaGpuContext;

/// A Skia-backed hardware screen renderer for dual-screen devices using a single wide canvas.
///
/// Manages a single offscreen GPU-accelerated Skia surface sized to `(left_width + right_width) x height`.
/// Renders 2D graphics seamlessly across the entire dual display surface (e.g. 960x272 on Maschine MK3/Studio/S8)
/// and submits the double-width frame to encdr, which splits the frame horizontally and updates both
/// controller screens independently.
pub struct SkiaDualScreenView {
    gpu: Arc<Mutex<SkiaGpuContext>>,
    surface: Surface,
    device_id: DeviceId,
    left_screen: String,
    right_screen: String,
    total_width: u32,
    left_width: u32,
    right_width: u32,
    height: u32,
    pixel_buf: Vec<u8>,
}

impl SkiaDualScreenView {
    /// Create a new Skia-backed dual screen renderer for a connected device using a dedicated GPU context.
    pub fn new(
        encdr: &Encdr,
        device_id: DeviceId,
        left_screen: &str,
        right_screen: &str,
    ) -> Result<Self, String> {
        let gpu = Arc::new(Mutex::new(SkiaGpuContext::new()));
        Self::with_gpu_context(encdr, device_id, left_screen, right_screen, gpu)
    }

    /// Create a new Skia-backed dual screen renderer using an existing shared GPU context.
    pub fn with_gpu_context(
        encdr: &Encdr,
        device_id: DeviceId,
        left_screen: &str,
        right_screen: &str,
        gpu: Arc<Mutex<SkiaGpuContext>>,
    ) -> Result<Self, String> {
        let descriptor = encdr
            .device_descriptor(device_id)
            .ok_or_else(|| format!("Device {:?} not connected", device_id))?;

        let left_desc = descriptor
            .screens
            .iter()
            .find(|s| s.name == left_screen)
            .ok_or_else(|| format!("Left screen '{}' not found on device", left_screen))?;

        let right_desc = descriptor
            .screens
            .iter()
            .find(|s| s.name == right_screen)
            .ok_or_else(|| format!("Right screen '{}' not found on device", right_screen))?;

        if left_desc.height != right_desc.height {
            return Err(format!(
                "Screen height mismatch: left is {}, right is {}",
                left_desc.height, right_desc.height
            ));
        }

        let left_width = left_desc.width as u32;
        let right_width = right_desc.width as u32;
        let total_width = left_width + right_width;
        let height = left_desc.height as u32;

        let surface = {
            let mut gpu_lock = gpu.lock().map_err(|e| e.to_string())?;
            gpu_lock.create_surface(total_width as i32, height as i32)?
        };

        let pixel_buf = vec![0u8; (total_width * height * 4) as usize];

        Ok(Self {
            gpu,
            surface,
            device_id,
            left_screen: left_screen.to_string(),
            right_screen: right_screen.to_string(),
            total_width,
            left_width,
            right_width,
            height,
            pixel_buf,
        })
    }

    /// Access the Skia canvas for direct rendering across both screens.
    pub fn canvas(&mut self) -> &Canvas {
        self.surface.canvas()
    }

    /// Render a frame with a closure and immediately flush and submit it to both device screens.
    pub fn draw<F>(&mut self, encdr: &Encdr, f: F) -> Result<(), String>
    where
        F: FnOnce(&Canvas),
    {
        f(self.surface.canvas());
        self.submit(encdr)
    }

    /// Flush GPU rendering commands, read pixels, and submit the dual-screen frame to Encdr.
    ///
    /// If another application currently holds hardware control, submission is safely skipped.
    pub fn submit(&mut self, encdr: &Encdr) -> Result<(), String> {
        if !encdr.has_control(self.device_id) {
            tracing::trace!("Skipping Skia dual screen submit: another application has the hardware");
            return Ok(());
        }

        // Flush commands to GPU
        {
            let mut gpu_lock = self.gpu.lock().map_err(|e| e.to_string())?;
            gpu_lock.flush_and_submit(&mut self.surface);
        }

        // Read pixels
        let image_info = ImageInfo::new(
            (self.total_width as i32, self.height as i32),
            ColorType::RGBA8888,
            AlphaType::Premul,
            Some(ColorSpace::new_srgb()),
        );

        let row_bytes = (self.total_width as usize) * 4;
        if !self
            .surface
            .read_pixels(&image_info, &mut self.pixel_buf, row_bytes, (0, 0))
        {
            return Err("Failed to read pixels from Skia dual-screen surface".into());
        }

        // Submit double-width frame to encdr pipeline
        encdr.submit_dual_screen_with_format(
            self.device_id,
            &self.left_screen,
            &self.right_screen,
            &self.pixel_buf,
            PixelFormat::Rgba8888,
        );

        Ok(())
    }

    /// Total combined width of both screens in pixels.
    pub fn width(&self) -> u32 {
        self.total_width
    }

    /// Width of the left screen in pixels.
    pub fn left_width(&self) -> u32 {
        self.left_width
    }

    /// Width of the right screen in pixels.
    pub fn right_width(&self) -> u32 {
        self.right_width
    }

    /// Height of the screens in pixels.
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

    /// Left screen name.
    pub fn left_screen_name(&self) -> &str {
        &self.left_screen
    }

    /// Right screen name.
    pub fn right_screen_name(&self) -> &str {
        &self.right_screen
    }
}
