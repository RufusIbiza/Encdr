#[cfg(feature = "gpu")]
use std::sync::Arc;

/// Shared GPU context for format conversion and frame diffing.
/// Can be provided by the host app or created by Encdr.
#[cfg(feature = "gpu")]
pub struct GpuContext {
    pub device: Arc<wgpu::Device>,
    pub queue: Arc<wgpu::Queue>,
}

/// Stub GPU context when built without the `gpu` feature (CPU fallback is used).
#[cfg(not(feature = "gpu"))]
pub struct GpuContext {
    _private: (),
}

impl GpuContext {
    /// Create a new GPU context with a high-performance adapter.
    /// Returns `None` if no GPU is available or the `gpu` feature is disabled.
    #[cfg(feature = "gpu")]
    pub async fn new() -> Option<Self> {
        let instance = wgpu::Instance::default();
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                ..Default::default()
            })
            .await
            .ok()?;

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default())
            .await
            .ok()?;

        device.on_uncaptured_error(Arc::new(|err| {
            tracing::error!("wgpu uncaptured error: {err}");
        }));

        Some(Self {
            device: Arc::new(device),
            queue: Arc::new(queue),
        })
    }

    /// Create a new GPU context. Always returns `None` when built without the `gpu` feature.
    #[cfg(not(feature = "gpu"))]
    pub async fn new() -> Option<Self> {
        None
    }

    /// Create from an existing wgpu device/queue (shared with host app).
    #[cfg(feature = "gpu")]
    pub fn from_existing(device: Arc<wgpu::Device>, queue: Arc<wgpu::Queue>) -> Self {
        Self { device, queue }
    }
}
