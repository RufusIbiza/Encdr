use std::sync::Arc;

/// Shared GPU context for format conversion and frame diffing.
/// Can be provided by the host app or created by Encdr.
pub struct GpuContext {
    pub device: Arc<wgpu::Device>,
    pub queue: Arc<wgpu::Queue>,
}

impl GpuContext {
    /// Create a new GPU context with a high-performance adapter.
    pub async fn new() -> Option<Self> {
        let instance = wgpu::Instance::default();
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                ..Default::default()
            })
            .await?;

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default(), None)
            .await
            .ok()?;

        Some(Self {
            device: Arc::new(device),
            queue: Arc::new(queue),
        })
    }

    /// Create from an existing wgpu device/queue (shared with host app).
    pub fn from_existing(device: Arc<wgpu::Device>, queue: Arc<wgpu::Queue>) -> Self {
        Self { device, queue }
    }
}
