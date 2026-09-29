use std::borrow::Cow;
use std::sync::Arc;

use super::gpu::GpuContext;

/// GPU compute pipeline for RGBA→BGR565BE format conversion.
///
/// Manages wgpu buffers and compute pipeline, reusable across frames.
/// Buffers are lazily sized to the first frame and reallocated if the
/// frame size changes.
pub struct GpuConvertPipeline {
    gpu: Arc<GpuContext>,
    pipeline: wgpu::ComputePipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    /// Current capacity in pixels (0 = uninitialized)
    capacity_pixels: usize,
    /// GPU buffers — created/resized on first use or when frame size changes
    input_buf: Option<wgpu::Buffer>,
    output_buf: Option<wgpu::Buffer>,
    params_buf: Option<wgpu::Buffer>,
    staging_buf: Option<wgpu::Buffer>,
    bind_group: Option<wgpu::BindGroup>,
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct Params {
    pixel_count: u32,
}

impl GpuConvertPipeline {
    /// Create the compute pipeline. This is cheap — no buffers are allocated yet.
    pub fn new(gpu: Arc<GpuContext>) -> Self {
        let shader = gpu
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("encdr_convert_shader"),
                source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!(
                    "shaders/convert.wgsl"
                ))),
            });

        let bind_group_layout =
            gpu.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("encdr_convert_bgl"),
                    entries: &[
                        // @binding(0) input: storage read
                        wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: wgpu::ShaderStages::COMPUTE,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Storage { read_only: true },
                                has_dynamic_offset: false,
                                min_binding_size: None,
                            },
                            count: None,
                        },
                        // @binding(1) output: storage read_write
                        wgpu::BindGroupLayoutEntry {
                            binding: 1,
                            visibility: wgpu::ShaderStages::COMPUTE,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Storage { read_only: false },
                                has_dynamic_offset: false,
                                min_binding_size: None,
                            },
                            count: None,
                        },
                        // @binding(2) params: uniform
                        wgpu::BindGroupLayoutEntry {
                            binding: 2,
                            visibility: wgpu::ShaderStages::COMPUTE,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Uniform,
                                has_dynamic_offset: false,
                                min_binding_size: None,
                            },
                            count: None,
                        },
                    ],
                });

        let pipeline_layout =
            gpu.device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("encdr_convert_layout"),
                    bind_group_layouts: &[Some(&bind_group_layout)],
                    immediate_size: 0,
                });

        let pipeline = gpu
            .device
            .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("encdr_convert_pipeline"),
                layout: Some(&pipeline_layout),
                module: &shader,
                entry_point: Some("main"),
                compilation_options: Default::default(),
                cache: None,
            });

        Self {
            gpu,
            pipeline,
            bind_group_layout,
            capacity_pixels: 0,
            input_buf: None,
            output_buf: None,
            params_buf: None,
            staging_buf: None,
            bind_group: None,
        }
    }

    /// Ensure buffers are allocated for the given pixel count.
    fn ensure_capacity(&mut self, pixel_count: usize) {
        if self.capacity_pixels >= pixel_count && self.input_buf.is_some() {
            return;
        }

        let input_bytes = (pixel_count * 4) as u64; // RGBA: 4 bytes/pixel
        let output_pairs = (pixel_count + 1) / 2; // Two pixels per u32
        let output_bytes = (output_pairs * 4) as u64;

        let input_buf = self.gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("encdr_convert_input"),
            size: input_bytes,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let output_buf = self.gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("encdr_convert_output"),
            size: output_bytes,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let params_buf = self.gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("encdr_convert_params"),
            size: std::mem::size_of::<Params>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let staging_buf = self.gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("encdr_convert_staging"),
            size: output_bytes,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group = self
            .gpu
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("encdr_convert_bg"),
                layout: &self.bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: input_buf.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: output_buf.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: params_buf.as_entire_binding(),
                    },
                ],
            });

        self.input_buf = Some(input_buf);
        self.output_buf = Some(output_buf);
        self.params_buf = Some(params_buf);
        self.staging_buf = Some(staging_buf);
        self.bind_group = Some(bind_group);
        self.capacity_pixels = pixel_count;
    }

    /// Convert RGBA8888 pixels to BGR565 big-endian using the GPU.
    ///
    /// `rgba` must be `width * height * 4` bytes (RGBA).
    /// Returns `width * height * 2` bytes (BGR565 BE).
    pub async fn convert_rgba_to_bgr565(&mut self, rgba: &[u8]) -> Vec<u8> {
        let pixel_count = rgba.len() / 4;
        if pixel_count == 0 {
            return Vec::new();
        }

        self.ensure_capacity(pixel_count);

        let input_buf = self.input_buf.as_ref().unwrap();
        let output_buf = self.output_buf.as_ref().unwrap();
        let params_buf = self.params_buf.as_ref().unwrap();
        let staging_buf = self.staging_buf.as_ref().unwrap();
        let bind_group = self.bind_group.as_ref().unwrap();

        // Upload input data
        self.gpu.queue.write_buffer(input_buf, 0, rgba);

        // Upload params
        let params = Params {
            pixel_count: pixel_count as u32,
        };
        self.gpu
            .queue
            .write_buffer(params_buf, 0, bytemuck::bytes_of(&params));

        // Dispatch compute
        let output_pairs = (pixel_count + 1) / 2;
        let workgroups = ((output_pairs as u32) + 255) / 256;

        let mut encoder = self
            .gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("encdr_convert_encoder"),
            });

        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("encdr_convert_pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, bind_group, &[]);
            pass.dispatch_workgroups(workgroups, 1, 1);
        }

        // Copy output to staging buffer for readback
        let output_bytes = (output_pairs * 4) as u64;
        encoder.copy_buffer_to_buffer(output_buf, 0, staging_buf, 0, output_bytes);

        self.gpu.queue.submit(std::iter::once(encoder.finish()));

        // Map the staging buffer and read back
        let buffer_slice = staging_buf.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();

        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            tx.send(result).ok();
        });

        let _ = self.gpu.device.poll(wgpu::PollType::wait_indefinitely());

        if let Ok(Ok(())) = rx.recv() {
            if let Ok(data) = buffer_slice.get_mapped_range() {
                let result_bytes = pixel_count * 2;
                let out = data[..result_bytes].to_vec();
                drop(data);
                staging_buf.unmap();
                return out;
            }
        }

        tracing::warn!("GPU readback failed, falling back to CPU conversion");
        super::rgba8_to_bgr565_be(rgba, pixel_count, 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gpu_pipeline_conversion_if_supported() {
        let gpu = futures_lite::future::block_on(GpuContext::new());
        assert!(gpu.is_some(), "GPU context should be created on this system");
        if let Some(gpu) = gpu {
            let mut pipeline = GpuConvertPipeline::new(Arc::new(gpu));
            // 4 pixels: Red, Green, Blue, White
            let rgba = vec![
                255, 0, 0, 255,
                0, 255, 0, 255,
                0, 0, 255, 255,
                255, 255, 255, 255,
            ];
            let converted = futures_lite::future::block_on(pipeline.convert_rgba_to_bgr565(&rgba));
            let cpu_expected = super::super::rgba8_to_bgr565_be(&rgba, 4, 1);
            assert_eq!(converted, cpu_expected);
        }
    }

    #[test]
    fn test_gpu_pipeline_conversion_full_screen() {
        let gpu = futures_lite::future::block_on(GpuContext::new());
        assert!(gpu.is_some(), "GPU context should be created on this system");
        if let Some(gpu) = gpu {
            let mut pipeline = GpuConvertPipeline::new(Arc::new(gpu));
            let pixel_count = 480 * 272;
            let mut rgba = vec![0u8; pixel_count * 4];
            for i in 0..pixel_count {
                rgba[i * 4] = (i % 256) as u8;
                rgba[i * 4 + 1] = ((i / 256) % 256) as u8;
                rgba[i * 4 + 2] = 200;
                rgba[i * 4 + 3] = 255;
            }
            let converted = futures_lite::future::block_on(pipeline.convert_rgba_to_bgr565(&rgba));
            let cpu_expected = super::super::rgba8_to_bgr565_be(&rgba, pixel_count, 1);
            assert_eq!(converted.len(), cpu_expected.len());
            assert_eq!(converted, cpu_expected);
        }
    }
}
