use std::borrow::Cow;
use std::sync::Arc;

use super::gpu::GpuContext;
use super::DirtyRect;
use crate::core::descriptor::PixelFormat;

/// Output of a single-submission fused GPU frame pass (`convert -> diff -> extract`).
#[derive(Debug, PartialEq, Eq)]
pub enum GpuFrameOutput {
    /// Frame is identical to the VRAM-resident previous frame; zero pixels read back.
    Clean,
    /// Only the aligned dirty sub-rectangle was extracted and read back.
    Partial {
        x: u16,
        y: u16,
        w: u16,
        h: u16,
        pixels: Vec<u8>,
    },
    /// Full native BGR565BE frame (keyframe, partial_blit disabled, or dirty area above threshold).
    Full { pixels: Vec<u8> },
}

/// GPU compute pipeline for format conversion, dual-screen splitting, and dirty-rect diffing.
pub struct GpuConvertPipeline {
    gpu: Arc<GpuContext>,

    // Pipelines
    convert_pipeline: wgpu::ComputePipeline,
    convert_bgl: wgpu::BindGroupLayout,

    split_pipeline: wgpu::ComputePipeline,
    split_bgl: wgpu::BindGroupLayout,

    diff_pipeline: wgpu::ComputePipeline,
    diff_bgl: wgpu::BindGroupLayout,

    extract_pipeline: wgpu::ComputePipeline,
    extract_bgl: wgpu::BindGroupLayout,

    extract_bounds_pipeline: wgpu::ComputePipeline,
    extract_bounds_bgl: wgpu::BindGroupLayout,

    // Convert buffers (output_buf is also the fused pipeline's VRAM-resident curr_buf)
    capacity_pixels: usize,
    input_buf: Option<wgpu::Buffer>,
    output_buf: Option<wgpu::Buffer>,
    params_buf: Option<wgpu::Buffer>,
    staging_buf: Option<wgpu::Buffer>,
    convert_bg: Option<wgpu::BindGroup>,

    // Dual-screen buffers
    dual_capacity_pixels: usize,
    dual_input_buf: Option<wgpu::Buffer>,
    dual_left_buf: Option<wgpu::Buffer>,
    dual_right_buf: Option<wgpu::Buffer>,
    dual_params_buf: Option<wgpu::Buffer>,
    dual_left_staging: Option<wgpu::Buffer>,
    dual_right_staging: Option<wgpu::Buffer>,
    split_bg: Option<wgpu::BindGroup>,

    // Standalone diff buffers
    diff_curr_buf: Option<wgpu::Buffer>,
    diff_prev_buf: Option<wgpu::Buffer>,
    diff_bounds_buf: Option<wgpu::Buffer>,
    diff_bounds_staging: Option<wgpu::Buffer>,
    diff_params_buf: Option<wgpu::Buffer>,
    diff_bg: Option<wgpu::BindGroup>,

    // Standalone extract buffers
    extract_src_buf: Option<wgpu::Buffer>,
    extract_dst_buf: Option<wgpu::Buffer>,
    extract_staging: Option<wgpu::Buffer>,
    extract_params_buf: Option<wgpu::Buffer>,
    extract_bg: Option<wgpu::BindGroup>,
    extract_capacity_bytes: usize,

    // Fused single-submission VRAM-resident buffers (convert -> diff -> extract_bounds)
    fused_capacity_pixels: usize,
    fused_prev_buf: Option<wgpu::Buffer>,
    fused_dst_buf: Option<wgpu::Buffer>,
    fused_staging_buf: Option<wgpu::Buffer>,
    fused_bounds_buf: Option<wgpu::Buffer>,
    fused_bounds_staging: Option<wgpu::Buffer>,
    fused_diff_params_buf: Option<wgpu::Buffer>,
    fused_extract_params_buf: Option<wgpu::Buffer>,
    fused_diff_bg: Option<wgpu::BindGroup>,
    fused_extract_bg: Option<wgpu::BindGroup>,
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct ConvertParams {
    pixel_count: u32,
    width: u32,
    dither: u32,
    _pad: u32,
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct SplitParams {
    total_width: u32,
    left_width: u32,
    right_width: u32,
    height: u32,
    dither: u32,
    _pad: [u32; 3],
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct DiffParams {
    width: u32,
    height: u32,
    pixel_count: u32,
    _pad: u32,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, bytemuck::Pod, bytemuck::Zeroable)]
struct GpuDiffBounds {
    min_x: u32,
    max_x: u32,
    min_y: u32,
    max_y: u32,
    dirty_count: u32,
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct ExtractParams {
    src_width: u32,
    rect_x: u32,
    rect_y: u32,
    rect_w: u32,
    rect_h: u32,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct FusedExtractParams {
    width: u32,
    height: u32,
    x_align: u32,
    y_align: u32,
    force_full: u32,
    partial_supported: u32,
    full_threshold_pct: u32,
    _pad: u32,
}

fn create_compute_pipeline(
    device: &wgpu::Device,
    label: &str,
    wgsl_source: &str,
    bind_group_layouts: &[&wgpu::BindGroupLayout],
) -> wgpu::ComputePipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(&format!("{}_shader", label)),
        source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(wgsl_source)),
    });

    let opt_layouts: Vec<Option<&wgpu::BindGroupLayout>> =
        bind_group_layouts.iter().map(|l| Some(*l)).collect();
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some(&format!("{}_layout", label)),
        bind_group_layouts: &opt_layouts,
        immediate_size: 0,
    });

    device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some(&format!("{}_pipeline", label)),
        layout: Some(&layout),
        module: &shader,
        entry_point: Some("main"),
        compilation_options: Default::default(),
        cache: None,
    })
}

impl GpuConvertPipeline {
    pub fn new(gpu: Arc<GpuContext>) -> Self {
        // 1. Convert
        let convert_bgl = gpu.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("encdr_convert_bgl"),
            entries: &[
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
        let convert_pipeline = create_compute_pipeline(
            &gpu.device,
            "encdr_convert",
            include_str!("shaders/convert.wgsl"),
            &[&convert_bgl],
        );

        // 2. Split
        let split_bgl = gpu.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("encdr_split_bgl"),
            entries: &[
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
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
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
        let split_pipeline = create_compute_pipeline(
            &gpu.device,
            "encdr_split",
            include_str!("shaders/split_convert.wgsl"),
            &[&split_bgl],
        );

        // 3. Diff
        let diff_bgl = gpu.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("encdr_diff_bgl"),
            entries: &[
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
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
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
        let diff_pipeline = create_compute_pipeline(
            &gpu.device,
            "encdr_diff",
            include_str!("shaders/diff.wgsl"),
            &[&diff_bgl],
        );

        // 4. Extract
        let extract_bgl = gpu.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("encdr_extract_bgl"),
            entries: &[
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
        let extract_pipeline = create_compute_pipeline(
            &gpu.device,
            "encdr_extract",
            include_str!("shaders/extract_region.wgsl"),
            &[&extract_bgl],
        );

        // 5. Fused bounds-driven extraction
        let extract_bounds_bgl =
            gpu.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("encdr_extract_bounds_bgl"),
                    entries: &[
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
                        wgpu::BindGroupLayoutEntry {
                            binding: 2,
                            visibility: wgpu::ShaderStages::COMPUTE,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Storage { read_only: true },
                                has_dynamic_offset: false,
                                min_binding_size: None,
                            },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 3,
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
        let extract_bounds_pipeline = create_compute_pipeline(
            &gpu.device,
            "encdr_extract_bounds",
            include_str!("shaders/extract_bounds.wgsl"),
            &[&extract_bounds_bgl],
        );

        Self {
            gpu,
            convert_pipeline,
            convert_bgl,
            split_pipeline,
            split_bgl,
            diff_pipeline,
            diff_bgl,
            extract_pipeline,
            extract_bgl,
            extract_bounds_pipeline,
            extract_bounds_bgl,
            capacity_pixels: 0,
            input_buf: None,
            output_buf: None,
            params_buf: None,
            staging_buf: None,
            convert_bg: None,
            dual_capacity_pixels: 0,
            dual_input_buf: None,
            dual_left_buf: None,
            dual_right_buf: None,
            dual_params_buf: None,
            dual_left_staging: None,
            dual_right_staging: None,
            split_bg: None,
            diff_curr_buf: None,
            diff_prev_buf: None,
            diff_bounds_buf: None,
            diff_bounds_staging: None,
            diff_params_buf: None,
            diff_bg: None,
            extract_src_buf: None,
            extract_dst_buf: None,
            extract_staging: None,
            extract_params_buf: None,
            extract_bg: None,
            extract_capacity_bytes: 0,
            fused_capacity_pixels: 0,
            fused_prev_buf: None,
            fused_dst_buf: None,
            fused_staging_buf: None,
            fused_bounds_buf: None,
            fused_bounds_staging: None,
            fused_diff_params_buf: None,
            fused_extract_params_buf: None,
            fused_diff_bg: None,
            fused_extract_bg: None,
        }
    }

    fn ensure_capacity(&mut self, pixel_count: usize) {
        if self.capacity_pixels >= pixel_count && self.input_buf.is_some() {
            return;
        }

        let input_bytes = (pixel_count * 4) as u64;
        let output_pairs = pixel_count.div_ceil(2);
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
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let params_buf = self.gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("encdr_convert_params"),
            size: std::mem::size_of::<ConvertParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let staging_buf = self.gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("encdr_convert_staging"),
            size: output_bytes,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let convert_bg = self
            .gpu
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("encdr_convert_bg"),
                layout: &self.convert_bgl,
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
        self.convert_bg = Some(convert_bg);
        self.capacity_pixels = pixel_count;
    }

    /// Convert RGBA8888 pixels to BGR565 big-endian using the GPU.
    pub async fn convert_rgba_to_bgr565(&mut self, rgba: &[u8], width: u32) -> Vec<u8> {
        let pixel_count = rgba.len() / 4;
        if pixel_count == 0 {
            return Vec::new();
        }

        self.ensure_capacity(pixel_count);

        let input_buf = self.input_buf.as_ref().unwrap();
        let output_buf = self.output_buf.as_ref().unwrap();
        let params_buf = self.params_buf.as_ref().unwrap();
        let staging_buf = self.staging_buf.as_ref().unwrap();
        let convert_bg = self.convert_bg.as_ref().unwrap();

        self.gpu.queue.write_buffer(input_buf, 0, &rgba[..pixel_count * 4]);
        let params = ConvertParams {
            pixel_count: pixel_count as u32,
            width: width.max(1),
            dither: super::dithering() as u32,
            _pad: 0,
        };
        self.gpu
            .queue
            .write_buffer(params_buf, 0, bytemuck::bytes_of(&params));

        let output_pairs = pixel_count.div_ceil(2);
        let workgroups = (output_pairs as u32).div_ceil(256);

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
            pass.set_pipeline(&self.convert_pipeline);
            pass.set_bind_group(0, convert_bg, &[]);
            pass.dispatch_workgroups(workgroups, 1, 1);
        }

        let output_bytes = (output_pairs * 4) as u64;
        encoder.copy_buffer_to_buffer(output_buf, 0, staging_buf, 0, output_bytes);
        self.gpu.queue.submit(std::iter::once(encoder.finish()));

        let buffer_slice = staging_buf.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            tx.send(result).ok();
        });

        let _ = self.gpu.device.poll(wgpu::PollType::wait_indefinitely());

        if let Ok(Ok(())) = rx.recv() {
            let out = buffer_slice.get_mapped_range().ok().map(|data| {
                let result_bytes = pixel_count * 2;
                data[..result_bytes].to_vec()
            });
            staging_buf.unmap();
            if let Some(out) = out {
                return out;
            }
        }

        tracing::warn!("GPU readback failed, falling back to CPU conversion");
        super::rgba8_to_bgr565_be(rgba, width.max(1) as usize, pixel_count / width.max(1) as usize)
    }

    /// Split double-width RGBA8888 input into left and right BGR565BE buffers directly on the GPU.
    pub async fn split_convert_dual(
        &mut self,
        rgba: &[u8],
        left_w: u32,
        right_w: u32,
        h: u32,
    ) -> (Vec<u8>, Vec<u8>) {
        let total_w = left_w + right_w;
        let total_pixels = (total_w * h) as usize;
        let expected_bytes = total_pixels * 4;

        if rgba.len() != expected_bytes {
            tracing::warn!(
                "split_convert_dual: size mismatch (expected {}, got {})",
                expected_bytes,
                rgba.len()
            );
            return (Vec::new(), Vec::new());
        }

        if self.dual_capacity_pixels < total_pixels || self.dual_input_buf.is_none() {
            let input_bytes = (total_pixels * 4) as u64;
            let left_bytes = ((left_w * h * 2).div_ceil(4) * 4) as u64;
            let right_bytes = ((right_w * h * 2).div_ceil(4) * 4) as u64;

            let dual_input_buf = self.gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("encdr_dual_input"),
                size: input_bytes,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });

            let dual_left_buf = self.gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("encdr_dual_left"),
                size: left_bytes,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            });

            let dual_right_buf = self.gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("encdr_dual_right"),
                size: right_bytes,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            });

            let dual_params_buf = self.gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("encdr_dual_params"),
                size: std::mem::size_of::<SplitParams>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });

            let dual_left_staging = self.gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("encdr_dual_left_staging"),
                size: left_bytes,
                usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });

            let dual_right_staging = self.gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("encdr_dual_right_staging"),
                size: right_bytes,
                usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });

            let split_bg = self.gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("encdr_split_bg"),
                layout: &self.split_bgl,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: dual_input_buf.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: dual_left_buf.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: dual_right_buf.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: dual_params_buf.as_entire_binding(),
                    },
                ],
            });

            self.dual_input_buf = Some(dual_input_buf);
            self.dual_left_buf = Some(dual_left_buf);
            self.dual_right_buf = Some(dual_right_buf);
            self.dual_params_buf = Some(dual_params_buf);
            self.dual_left_staging = Some(dual_left_staging);
            self.dual_right_staging = Some(dual_right_staging);
            self.split_bg = Some(split_bg);
            self.dual_capacity_pixels = total_pixels;
        }

        let input_buf = self.dual_input_buf.as_ref().unwrap();
        let left_buf = self.dual_left_buf.as_ref().unwrap();
        let right_buf = self.dual_right_buf.as_ref().unwrap();
        let params_buf = self.dual_params_buf.as_ref().unwrap();
        let left_staging = self.dual_left_staging.as_ref().unwrap();
        let right_staging = self.dual_right_staging.as_ref().unwrap();
        let split_bg = self.split_bg.as_ref().unwrap();

        let input_bytes = total_pixels * 4;
        if rgba.len() < input_bytes {
            return (Vec::new(), Vec::new());
        }
        self.gpu.queue.write_buffer(input_buf, 0, &rgba[..input_bytes]);
        let params = SplitParams {
            total_width: total_w,
            left_width: left_w,
            right_width: right_w,
            height: h,
            dither: super::dithering() as u32,
            _pad: [0; 3],
        };
        self.gpu
            .queue
            .write_buffer(params_buf, 0, bytemuck::bytes_of(&params));

        let pair_cols = total_w.div_ceil(2);
        let wg_x = pair_cols.div_ceil(16);
        let wg_y = h.div_ceil(16);

        let left_bytes = (left_w * h * 2) as u64;
        let right_bytes = (right_w * h * 2) as u64;
        let left_copy_bytes = left_bytes.next_multiple_of(4);
        let right_copy_bytes = right_bytes.next_multiple_of(4);

        let mut encoder = self
            .gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("encdr_split_encoder"),
            });

        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("encdr_split_pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.split_pipeline);
            pass.set_bind_group(0, split_bg, &[]);
            pass.dispatch_workgroups(wg_x, wg_y, 1);
        }

        encoder.copy_buffer_to_buffer(left_buf, 0, left_staging, 0, left_copy_bytes);
        encoder.copy_buffer_to_buffer(right_buf, 0, right_staging, 0, right_copy_bytes);
        self.gpu.queue.submit(std::iter::once(encoder.finish()));

        let left_slice = left_staging.slice(..left_copy_bytes);
        let right_slice = right_staging.slice(..right_copy_bytes);

        let (tx_l, rx_l) = std::sync::mpsc::channel();
        let (tx_r, rx_r) = std::sync::mpsc::channel();

        left_slice.map_async(wgpu::MapMode::Read, move |res| {
            tx_l.send(res).ok();
        });
        right_slice.map_async(wgpu::MapMode::Read, move |res| {
            tx_r.send(res).ok();
        });

        let _ = self.gpu.device.poll(wgpu::PollType::wait_indefinitely());

        let left_out = if let Ok(Ok(())) = rx_l.recv() {
            let v = left_slice
                .get_mapped_range()
                .ok()
                .map(|data| data[..left_bytes as usize].to_vec());
            left_staging.unmap();
            v
        } else {
            None
        };

        let right_out = if let Ok(Ok(())) = rx_r.recv() {
            let v = right_slice
                .get_mapped_range()
                .ok()
                .map(|data| data[..right_bytes as usize].to_vec());
            right_staging.unmap();
            v
        } else {
            None
        };

        match (left_out, right_out) {
            (Some(l), Some(r)) => (l, r),
            _ => {
                tracing::warn!("GPU dual-screen readback failed, falling back to CPU");
                let lw = left_w as usize;
                let rw = right_w as usize;
                let hh = h as usize;
                let total_stride = (lw + rw) * 4;
                let mut left_rgba = Vec::with_capacity(lw * hh * 4);
                let mut right_rgba = Vec::with_capacity(rw * hh * 4);
                for row in 0..hh {
                    let row_start = row * total_stride;
                    if row_start + total_stride <= rgba.len() {
                        left_rgba.extend_from_slice(&rgba[row_start..row_start + lw * 4]);
                        right_rgba.extend_from_slice(&rgba[row_start + lw * 4..row_start + total_stride]);
                    }
                }
                (
                    super::rgba8_to_bgr565_be(&left_rgba, lw, hh),
                    super::rgba8_to_bgr565_be(&right_rgba, rw, hh),
                )
            }
        }
    }

    /// Compute dirty-rect bounding box between current and previous BGR565 frames on the GPU.
    pub async fn compute_dirty_rect_gpu(
        &mut self,
        curr_bgr565: &[u8],
        prev_bgr565: &[u8],
        width: u16,
        height: u16,
        x_align: u16,
        y_align: u16,
    ) -> DirtyRect {
        if curr_bgr565.len() != prev_bgr565.len() || curr_bgr565.is_empty() {
            return DirtyRect::Full;
        }
        if !curr_bgr565.len().is_multiple_of(4) {
            return super::compute_dirty_rect(curr_bgr565, prev_bgr565, width, height, 2, x_align, y_align);
        }

        let pixel_count = curr_bgr565.len() / 2;
        let byte_len = curr_bgr565.len() as u64;

        if self.diff_curr_buf.is_none()
            || self.diff_curr_buf.as_ref().unwrap().size() < byte_len
        {
            let curr_buf = self.gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("encdr_diff_curr"),
                size: byte_len,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });

            let prev_buf = self.gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("encdr_diff_prev"),
                size: byte_len,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });

            let bounds_buf = self.gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("encdr_diff_bounds"),
                size: std::mem::size_of::<GpuDiffBounds>() as u64,
                usage: wgpu::BufferUsages::STORAGE
                    | wgpu::BufferUsages::COPY_SRC
                    | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });

            let bounds_staging = self.gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("encdr_diff_bounds_staging"),
                size: std::mem::size_of::<GpuDiffBounds>() as u64,
                usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });

            let params_buf = self.gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("encdr_diff_params"),
                size: std::mem::size_of::<DiffParams>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });

            let diff_bg = self.gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("encdr_diff_bg"),
                layout: &self.diff_bgl,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: curr_buf.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: prev_buf.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: bounds_buf.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: params_buf.as_entire_binding(),
                    },
                ],
            });

            self.diff_curr_buf = Some(curr_buf);
            self.diff_prev_buf = Some(prev_buf);
            self.diff_bounds_buf = Some(bounds_buf);
            self.diff_bounds_staging = Some(bounds_staging);
            self.diff_params_buf = Some(params_buf);
            self.diff_bg = Some(diff_bg);
        }

        let curr_buf = self.diff_curr_buf.as_ref().unwrap();
        let prev_buf = self.diff_prev_buf.as_ref().unwrap();
        let bounds_buf = self.diff_bounds_buf.as_ref().unwrap();
        let bounds_staging = self.diff_bounds_staging.as_ref().unwrap();
        let params_buf = self.diff_params_buf.as_ref().unwrap();
        let diff_bg = self.diff_bg.as_ref().unwrap();

        self.gpu.queue.write_buffer(curr_buf, 0, curr_bgr565);
        self.gpu.queue.write_buffer(prev_buf, 0, prev_bgr565);

        let initial_bounds = GpuDiffBounds {
            min_x: width as u32,
            max_x: 0,
            min_y: height as u32,
            max_y: 0,
            dirty_count: 0,
        };
        self.gpu
            .queue
            .write_buffer(bounds_buf, 0, bytemuck::bytes_of(&initial_bounds));

        let diff_params = DiffParams {
            width: width as u32,
            height: height as u32,
            pixel_count: pixel_count as u32,
            _pad: 0,
        };
        self.gpu
            .queue
            .write_buffer(params_buf, 0, bytemuck::bytes_of(&diff_params));

        let pairs = pixel_count.div_ceil(2);
        let workgroups = (pairs as u32).div_ceil(256);

        let mut encoder = self
            .gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("encdr_diff_encoder"),
            });

        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("encdr_diff_pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.diff_pipeline);
            pass.set_bind_group(0, diff_bg, &[]);
            pass.dispatch_workgroups(workgroups, 1, 1);
        }

        encoder.copy_buffer_to_buffer(
            bounds_buf,
            0,
            bounds_staging,
            0,
            std::mem::size_of::<GpuDiffBounds>() as u64,
        );
        self.gpu.queue.submit(std::iter::once(encoder.finish()));

        let slice = bounds_staging.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |res| {
            tx.send(res).ok();
        });

        let _ = self.gpu.device.poll(wgpu::PollType::wait_indefinitely());

        let bounds: Option<GpuDiffBounds> = if let Ok(Ok(())) = rx.recv() {
            let res = slice
                .get_mapped_range()
                .ok()
                .map(|data| *bytemuck::from_bytes::<GpuDiffBounds>(&data));
            bounds_staging.unmap();
            res
        } else {
            None
        };

        let Some(b) = bounds else {
            return DirtyRect::Full;
        };

        if b.dirty_count == 0 {
            return DirtyRect::Clean;
        }

        let min_x = b.min_x as u16;
        let max_x = b.max_x as u16;
        let min_y = b.min_y as u16;
        let max_y = b.max_y as u16;

        let x = if x_align > 1 {
            min_x - (min_x % x_align)
        } else {
            min_x
        };

        let y = if y_align > 1 {
            min_y - (min_y % y_align)
        } else {
            min_y
        };

        let raw_w = max_x - x + 1;
        let w = if x_align > 1 {
            raw_w.div_ceil(x_align) * x_align
        } else {
            raw_w
        };

        let raw_h = max_y - y + 1;
        let h = if y_align > 1 {
            raw_h.div_ceil(y_align) * y_align
        } else {
            raw_h
        };

        let w = w.min(width - x);
        let h = h.min(height - y);

        let total_area = width as usize * height as usize;
        let dirty_area = w as usize * h as usize;
        if dirty_area > (total_area * 85 / 100) {
            DirtyRect::Full
        } else {
            DirtyRect::Partial { x, y, w, h }
        }
    }

    /// Extract a sub-rectangle of BGR565 pixels using the GPU.
    pub async fn extract_region_gpu(
        &mut self,
        src_bgr565: &[u8],
        src_width: u16,
        x: u16,
        y: u16,
        w: u16,
        h: u16,
    ) -> Vec<u8> {
        let region_bytes = (w as usize * h as usize * 2) as u64;
        let src_bytes = src_bgr565.len() as u64;

        // WGSL extract shader operates on packed u32 pixel pairs (2 pixels / 4 bytes)
        // and wgpu requires 4-byte alignment for write_buffer and copy_buffer_to_buffer.
        if w == 0
            || h == 0
            || src_bytes == 0
            || !src_bytes.is_multiple_of(4)
            || !region_bytes.is_multiple_of(4)
            || !src_width.is_multiple_of(2)
            || !x.is_multiple_of(2)
            || !w.is_multiple_of(2)
        {
            return super::extract_region(src_bgr565, src_width, 2, x, y, w, h);
        }

        let src_needs_realloc = self
            .extract_src_buf
            .as_ref()
            .is_none_or(|b| b.size() < src_bytes);

        if self.extract_capacity_bytes < region_bytes as usize || src_needs_realloc {
            let src_buf = self.gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("encdr_extract_src"),
                size: src_bytes,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });

            let dst_buf = self.gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("encdr_extract_dst"),
                size: region_bytes,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            });

            let staging_buf = self.gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("encdr_extract_staging"),
                size: region_bytes,
                usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });

            let params_buf = self.gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("encdr_extract_params"),
                size: std::mem::size_of::<ExtractParams>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });

            let extract_bg = self.gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("encdr_extract_bg"),
                layout: &self.extract_bgl,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: src_buf.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: dst_buf.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: params_buf.as_entire_binding(),
                    },
                ],
            });

            self.extract_src_buf = Some(src_buf);
            self.extract_dst_buf = Some(dst_buf);
            self.extract_staging = Some(staging_buf);
            self.extract_params_buf = Some(params_buf);
            self.extract_bg = Some(extract_bg);
            self.extract_capacity_bytes = region_bytes as usize;
        }

        let src_buf = self.extract_src_buf.as_ref().unwrap();
        let dst_buf = self.extract_dst_buf.as_ref().unwrap();
        let staging_buf = self.extract_staging.as_ref().unwrap();
        let params_buf = self.extract_params_buf.as_ref().unwrap();
        let extract_bg = self.extract_bg.as_ref().unwrap();

        self.gpu.queue.write_buffer(src_buf, 0, src_bgr565);

        let params = ExtractParams {
            src_width: src_width as u32,
            rect_x: x as u32,
            rect_y: y as u32,
            rect_w: w as u32,
            rect_h: h as u32,
            _pad0: 0,
            _pad1: 0,
            _pad2: 0,
        };
        self.gpu
            .queue
            .write_buffer(params_buf, 0, bytemuck::bytes_of(&params));

        let pair_cols = (w as u32).div_ceil(2);
        let wg_x = pair_cols.div_ceil(16);
        let wg_y = (h as u32).div_ceil(16);

        let mut encoder = self
            .gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("encdr_extract_encoder"),
            });

        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("encdr_extract_pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.extract_pipeline);
            pass.set_bind_group(0, extract_bg, &[]);
            pass.dispatch_workgroups(wg_x, wg_y, 1);
        }

        encoder.copy_buffer_to_buffer(dst_buf, 0, staging_buf, 0, region_bytes);
        self.gpu.queue.submit(std::iter::once(encoder.finish()));

        let slice = staging_buf.slice(..region_bytes);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |res| {
            tx.send(res).ok();
        });

        let _ = self.gpu.device.poll(wgpu::PollType::wait_indefinitely());

        if let Ok(Ok(())) = rx.recv() {
            let out = slice
                .get_mapped_range()
                .ok()
                .map(|data| data[..region_bytes as usize].to_vec());
            staging_buf.unmap();
            if let Some(out) = out {
                return out;
            }
        }

        tracing::warn!("GPU region extraction failed, falling back to CPU");
        super::extract_region(src_bgr565, src_width, 2, x, y, w, h)
    }

    fn ensure_fused_capacity(&mut self, pixel_count: usize) {
        self.ensure_capacity(pixel_count);
        if self.fused_capacity_pixels == self.capacity_pixels && self.fused_prev_buf.is_some() {
            return;
        }

        let output_pairs = self.capacity_pixels.div_ceil(2);
        let output_bytes = (output_pairs * 4) as u64;
        let curr_buf = self.output_buf.as_ref().unwrap();

        let prev_buf = self.gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("encdr_fused_prev"),
            size: output_bytes,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let dst_buf = self.gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("encdr_fused_dst"),
            size: output_bytes,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let staging_buf = self.gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("encdr_fused_staging"),
            size: output_bytes,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bounds_buf = self.gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("encdr_fused_bounds"),
            size: std::mem::size_of::<GpuDiffBounds>() as u64,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bounds_staging = self.gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("encdr_fused_bounds_staging"),
            size: std::mem::size_of::<GpuDiffBounds>() as u64,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let diff_params_buf = self.gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("encdr_fused_diff_params"),
            size: std::mem::size_of::<DiffParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let extract_params_buf = self.gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("encdr_fused_extract_params"),
            size: std::mem::size_of::<FusedExtractParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let fused_diff_bg = self
            .gpu
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("encdr_fused_diff_bg"),
                layout: &self.diff_bgl,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: curr_buf.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: prev_buf.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: bounds_buf.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: diff_params_buf.as_entire_binding(),
                    },
                ],
            });

        let fused_extract_bg = self
            .gpu
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("encdr_fused_extract_bg"),
                layout: &self.extract_bounds_bgl,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: curr_buf.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: dst_buf.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: bounds_buf.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: extract_params_buf.as_entire_binding(),
                    },
                ],
            });

        self.fused_prev_buf = Some(prev_buf);
        self.fused_dst_buf = Some(dst_buf);
        self.fused_staging_buf = Some(staging_buf);
        self.fused_bounds_buf = Some(bounds_buf);
        self.fused_bounds_staging = Some(bounds_staging);
        self.fused_diff_params_buf = Some(diff_params_buf);
        self.fused_extract_params_buf = Some(extract_params_buf);
        self.fused_diff_bg = Some(fused_diff_bg);
        self.fused_extract_bg = Some(fused_extract_bg);
        self.fused_capacity_pixels = self.capacity_pixels;
    }

    /// Execute the entire `convert -> diff -> extract -> prev_frame update` pipeline
    /// in a single VRAM-resident GPU submission (1 upload, 1 `queue.submit`, 1 `device.poll`).
    ///
    /// Keeps `prev_frame` permanently in VRAM so static frames read back only 20 bytes
    /// (zero pixel copies) and partial frames read back only the dirty sub-rectangle.
    /// Returns `None` if the input format/dimensions are unsupported or GPU readback fails.
    #[allow(clippy::too_many_arguments)]
    pub fn submit_frame_fused(
        &mut self,
        pixels: &[u8],
        input_format: PixelFormat,
        width: u16,
        height: u16,
        x_align: u16,
        y_align: u16,
        partial_supported: bool,
        force_full: bool,
    ) -> Option<GpuFrameOutput> {
        const FULL_THRESHOLD_PCT: u32 = 85;

        if width == 0 || height == 0 || !width.is_multiple_of(2) {
            return None;
        }
        let pixel_count = width as usize * height as usize;
        let frame_bytes = pixel_count * 2;

        let needs_convert = match input_format {
            PixelFormat::Rgba8888 => {
                if pixels.len() != pixel_count * 4 {
                    return None;
                }
                true
            }
            PixelFormat::Bgr565Be => {
                if pixels.len() != frame_bytes {
                    return None;
                }
                false
            }
            _ => return None,
        };

        self.ensure_fused_capacity(pixel_count);

        let input_buf = self.input_buf.as_ref().unwrap();
        let curr_buf = self.output_buf.as_ref().unwrap();
        let convert_params_buf = self.params_buf.as_ref().unwrap();
        let convert_bg = self.convert_bg.as_ref().unwrap();

        let prev_buf = self.fused_prev_buf.as_ref().unwrap();
        let dst_buf = self.fused_dst_buf.as_ref().unwrap();
        let staging_buf = self.fused_staging_buf.as_ref().unwrap();
        let bounds_buf = self.fused_bounds_buf.as_ref().unwrap();
        let bounds_staging = self.fused_bounds_staging.as_ref().unwrap();
        let diff_params_buf = self.fused_diff_params_buf.as_ref().unwrap();
        let extract_params_buf = self.fused_extract_params_buf.as_ref().unwrap();
        let diff_bg = self.fused_diff_bg.as_ref().unwrap();
        let extract_bg = self.fused_extract_bg.as_ref().unwrap();

        // Even horizontal alignment >= 2 guarantees u32 pixel-pair alignment in extract_bounds.wgsl.
        let xa = (x_align.max(2) as u32).next_multiple_of(2);
        let ya = y_align.max(1) as u32;

        if needs_convert {
            self.gpu.queue.write_buffer(input_buf, 0, pixels);
            let convert_params = ConvertParams {
                pixel_count: pixel_count as u32,
                width: width as u32,
                dither: super::dithering() as u32,
                _pad: 0,
            };
            self.gpu
                .queue
                .write_buffer(convert_params_buf, 0, bytemuck::bytes_of(&convert_params));
        } else {
            self.gpu.queue.write_buffer(curr_buf, 0, pixels);
        }

        if !force_full {
            let initial_bounds = GpuDiffBounds {
                min_x: width as u32,
                max_x: 0,
                min_y: height as u32,
                max_y: 0,
                dirty_count: 0,
            };
            self.gpu
                .queue
                .write_buffer(bounds_buf, 0, bytemuck::bytes_of(&initial_bounds));

            let diff_params = DiffParams {
                width: width as u32,
                height: height as u32,
                pixel_count: pixel_count as u32,
                _pad: 0,
            };
            self.gpu
                .queue
                .write_buffer(diff_params_buf, 0, bytemuck::bytes_of(&diff_params));
        }

        let extract_params = FusedExtractParams {
            width: width as u32,
            height: height as u32,
            x_align: xa,
            y_align: ya,
            force_full: u32::from(force_full),
            partial_supported: u32::from(partial_supported),
            full_threshold_pct: FULL_THRESHOLD_PCT,
            _pad: 0,
        };
        self.gpu
            .queue
            .write_buffer(extract_params_buf, 0, bytemuck::bytes_of(&extract_params));

        let mut encoder = self
            .gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("encdr_fused_frame_encoder"),
            });

        let pairs = (pixel_count as u32).div_ceil(2);
        let wg_1d = pairs.div_ceil(256);

        if needs_convert {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("encdr_fused_convert_pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.convert_pipeline);
            pass.set_bind_group(0, convert_bg, &[]);
            pass.dispatch_workgroups(wg_1d, 1, 1);
        }

        if !force_full {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("encdr_fused_diff_pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.diff_pipeline);
            pass.set_bind_group(0, diff_bg, &[]);
            pass.dispatch_workgroups(wg_1d, 1, 1);
        }

        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("encdr_fused_extract_pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.extract_bounds_pipeline);
            pass.set_bind_group(0, extract_bg, &[]);
            pass.dispatch_workgroups((width as u32 / 2).div_ceil(16), (height as u32).div_ceil(16), 1);
        }

        let frame_bytes_u64 = frame_bytes as u64;
        // Keep prev_buf updated directly in VRAM (zero CPU copy).
        encoder.copy_buffer_to_buffer(curr_buf, 0, prev_buf, 0, frame_bytes_u64);
        if !force_full {
            encoder.copy_buffer_to_buffer(
                bounds_buf,
                0,
                bounds_staging,
                0,
                std::mem::size_of::<GpuDiffBounds>() as u64,
            );
        }
        encoder.copy_buffer_to_buffer(dst_buf, 0, staging_buf, 0, frame_bytes_u64);

        self.gpu.queue.submit(std::iter::once(encoder.finish()));

        // Map staging buffers before a single synchronous poll.
        let bounds_rx = if !force_full {
            let (tx, rx) = std::sync::mpsc::channel();
            bounds_staging.slice(..).map_async(wgpu::MapMode::Read, move |res| {
                tx.send(res).ok();
            });
            Some(rx)
        } else {
            None
        };

        let (px_tx, px_rx) = std::sync::mpsc::channel();
        staging_buf
            .slice(..frame_bytes_u64)
            .map_async(wgpu::MapMode::Read, move |res| {
                px_tx.send(res).ok();
            });

        let _ = self.gpu.device.poll(wgpu::PollType::wait_indefinitely());

        let bounds = if let Some(rx) = bounds_rx {
            let ok = matches!(rx.recv(), Ok(Ok(())));
            let res = if ok {
                bounds_staging
                    .slice(..)
                    .get_mapped_range()
                    .ok()
                    .map(|data| *bytemuck::from_bytes::<GpuDiffBounds>(&data))
            } else {
                None
            };
            if ok {
                bounds_staging.unmap();
            }
            res
        } else {
            None
        };

        let px_mapped = matches!(px_rx.recv(), Ok(Ok(())));
        if !px_mapped {
            tracing::warn!("Fused GPU frame readback failed");
            return None;
        }

        let result = if !force_full {
            let Some(b) = bounds else {
                staging_buf.unmap();
                return None;
            };
            if b.dirty_count == 0 {
                // Static frame: unmap without copying any pixels.
                Some(GpuFrameOutput::Clean)
            } else if partial_supported {
                let ax = (b.min_x / xa) * xa;
                let ay = (b.min_y / ya) * ya;
                let ar = (((b.max_x + xa) / xa) * xa).min(width as u32);
                let ab = (((b.max_y + ya) / ya) * ya).min(height as u32);
                let aw = ar.saturating_sub(ax);
                let ah = ab.saturating_sub(ay);

                let use_full = aw == 0
                    || ah == 0
                    || (aw as usize * ah as usize * 100) > (pixel_count * FULL_THRESHOLD_PCT as usize);

                if use_full {
                    staging_buf
                        .slice(..frame_bytes_u64)
                        .get_mapped_range()
                        .ok()
                        .map(|data| GpuFrameOutput::Full {
                            pixels: data[..frame_bytes].to_vec(),
                        })
                } else {
                    let region_bytes = aw as usize * ah as usize * 2;
                    staging_buf
                        .slice(..frame_bytes_u64)
                        .get_mapped_range()
                        .ok()
                        .map(|data| GpuFrameOutput::Partial {
                            x: ax as u16,
                            y: ay as u16,
                            w: aw as u16,
                            h: ah as u16,
                            pixels: data[..region_bytes].to_vec(),
                        })
                }
            } else {
                staging_buf
                    .slice(..frame_bytes_u64)
                    .get_mapped_range()
                    .ok()
                    .map(|data| GpuFrameOutput::Full {
                        pixels: data[..frame_bytes].to_vec(),
                    })
            }
        } else {
            staging_buf
                .slice(..frame_bytes_u64)
                .get_mapped_range()
                .ok()
                .map(|data| GpuFrameOutput::Full {
                    pixels: data[..frame_bytes].to_vec(),
                })
        };

        staging_buf.unmap();
        result
    }

    /// Read the VRAM-resident `prev_frame` back to CPU memory on demand
    /// (used only when redrawing a full screen after regaining hardware control).
    pub fn read_prev_frame(&self, frame_bytes: usize) -> Option<Vec<u8>> {
        let prev_buf = self.fused_prev_buf.as_ref()?;
        let staging_buf = self.fused_staging_buf.as_ref()?;
        let bytes_u64 = frame_bytes as u64;
        if bytes_u64 == 0 || !bytes_u64.is_multiple_of(4) || prev_buf.size() < bytes_u64 {
            return None;
        }

        let mut encoder = self
            .gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("encdr_read_prev_encoder"),
            });
        encoder.copy_buffer_to_buffer(prev_buf, 0, staging_buf, 0, bytes_u64);
        self.gpu.queue.submit(std::iter::once(encoder.finish()));

        let slice = staging_buf.slice(..bytes_u64);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |res| {
            tx.send(res).ok();
        });
        let _ = self.gpu.device.poll(wgpu::PollType::wait_indefinitely());

        if matches!(rx.recv(), Ok(Ok(()))) {
            let out = slice
                .get_mapped_range()
                .ok()
                .map(|data| data[..frame_bytes].to_vec());
            staging_buf.unmap();
            out
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::screen::DirtyRect;

    #[test]
    fn test_gpu_pipeline_conversion_if_supported() {
        let gpu = futures_lite::future::block_on(GpuContext::new());
        assert!(gpu.is_some(), "GPU context should be created on this system");
        if let Some(gpu) = gpu {
            let mut pipeline = GpuConvertPipeline::new(Arc::new(gpu));
            let rgba = vec![
                255, 0, 0, 255,
                0, 255, 0, 255,
                0, 0, 255, 255,
                255, 255, 255, 255,
            ];
            let converted = futures_lite::future::block_on(pipeline.convert_rgba_to_bgr565(&rgba, 4));
            let cpu_expected = super::super::rgba8_to_bgr565_be(&rgba, 4, 1);
            assert_eq!(converted, cpu_expected);
        }
    }

    #[test]
    fn test_gpu_split_convert_dual() {
        let gpu = futures_lite::future::block_on(GpuContext::new());
        if let Some(gpu) = gpu {
            let mut pipeline = GpuConvertPipeline::new(Arc::new(gpu));
            let left_w = 480u32;
            let right_w = 480u32;
            let total_w = left_w + right_w;
            let h = 272u32;

            let total_pixels = (total_w * h) as usize;
            let mut rgba = vec![0u8; total_pixels * 4];
            for i in 0..total_pixels {
                rgba[i * 4] = (i % 256) as u8;
                rgba[i * 4 + 1] = ((i / 256) % 256) as u8;
                rgba[i * 4 + 2] = 128;
                rgba[i * 4 + 3] = 255;
            }

            let (gpu_left, gpu_right) = futures_lite::future::block_on(
                pipeline.split_convert_dual(&rgba, left_w, right_w, h),
            );

            let (cpu_left_rgba, cpu_right_rgba) = crate::screen::split_horizontal(
                &rgba,
                left_w as usize,
                right_w as usize,
                h as usize,
                4,
            )
            .unwrap();
            let cpu_left_expected = super::super::rgba8_to_bgr565_be(&cpu_left_rgba, left_w as usize, h as usize);
            let cpu_right_expected = super::super::rgba8_to_bgr565_be(&cpu_right_rgba, right_w as usize, h as usize);

            assert_eq!(gpu_left.len(), cpu_left_expected.len());
            assert_eq!(gpu_right.len(), cpu_right_expected.len());
            assert_eq!(gpu_left, cpu_left_expected);
            assert_eq!(gpu_right, cpu_right_expected);
        }
    }

    #[test]
    fn test_gpu_diff_and_extract() {
        let gpu = futures_lite::future::block_on(GpuContext::new());
        if let Some(gpu) = gpu {
            let mut pipeline = GpuConvertPipeline::new(Arc::new(gpu));
            let width = 480u16;
            let height = 272u16;
            let pixel_count = width as usize * height as usize;
            let byte_count = pixel_count * 2;

            let prev = vec![0x10u8; byte_count];
            let mut curr = prev.clone();

            // 1. Identical frames -> DirtyRect::Clean
            let dirty = futures_lite::future::block_on(pipeline.compute_dirty_rect_gpu(
                &curr,
                &prev,
                width,
                height,
                4,
                4,
            ));
            assert_eq!(dirty, DirtyRect::Clean);

            // 2. Modify a small 32x16 box at (40, 20)
            for y in 20..36 {
                for x in 40..72 {
                    let idx = (y * width as usize + x) * 2;
                    curr[idx] = 0xAA;
                    curr[idx + 1] = 0xBB;
                }
            }

            let dirty = futures_lite::future::block_on(pipeline.compute_dirty_rect_gpu(
                &curr,
                &prev,
                width,
                height,
                4,
                4,
            ));

            match dirty {
                DirtyRect::Partial { x, y, w, h } => {
                    assert_eq!(x, 40);
                    assert_eq!(y, 20);
                    assert_eq!(w, 32);
                    assert_eq!(h, 16);

                    // Test GPU region extraction
                    let gpu_extracted = futures_lite::future::block_on(
                        pipeline.extract_region_gpu(&curr, width, x, y, w, h),
                    );
                    let cpu_extracted = super::super::extract_region(&curr, width, 2, x, y, w, h);
                    assert_eq!(gpu_extracted, cpu_extracted);
                }
                _ => panic!("Expected Partial dirty rect, got {:?}", dirty),
            }
        }
    }

    #[test]
    fn test_fused_gpu_pipeline_clean_partial_full_and_readback() {
        let gpu = futures_lite::future::block_on(GpuContext::new());
        if let Some(gpu) = gpu {
            let mut pipeline = GpuConvertPipeline::new(Arc::new(gpu));
            let width = 64u16;
            let height = 32u16;
            let pixel_count = width as usize * height as usize;

            // Frame 1: solid blue RGBA, force_full = true
            let mut frame1 = vec![0u8; pixel_count * 4];
            for px in frame1.chunks_exact_mut(4) {
                px.copy_from_slice(&[0, 0, 255, 255]);
            }
            let expected1 = super::super::rgba8_to_bgr565_be(&frame1, width as usize, height as usize);
            let out1 = pipeline
                .submit_frame_fused(&frame1, PixelFormat::Rgba8888, width, height, 4, 4, true, true)
                .expect("fused frame 1");
            assert_eq!(out1, GpuFrameOutput::Full { pixels: expected1.clone() });

            // Frame 2: identical RGBA, force_full = false -> Clean (zero pixels read back)
            let out2 = pipeline
                .submit_frame_fused(&frame1, PixelFormat::Rgba8888, width, height, 4, 4, true, false)
                .expect("fused frame 2");
            assert_eq!(out2, GpuFrameOutput::Clean);

            // Frame 3: paint a red 12x8 rect at (16, 8) -> Partial
            let mut frame3 = frame1.clone();
            for y in 8..16 {
                for x in 16..28 {
                    let off = (y * width as usize + x) * 4;
                    frame3[off..off + 4].copy_from_slice(&[255, 0, 0, 255]);
                }
            }
            let expected3_full =
                super::super::rgba8_to_bgr565_be(&frame3, width as usize, height as usize);
            let expected3_region =
                super::super::extract_region(&expected3_full, width, 2, 16, 8, 12, 8);
            let out3 = pipeline
                .submit_frame_fused(&frame3, PixelFormat::Rgba8888, width, height, 4, 4, true, false)
                .expect("fused frame 3");
            assert_eq!(
                out3,
                GpuFrameOutput::Partial {
                    x: 16,
                    y: 8,
                    w: 12,
                    h: 8,
                    pixels: expected3_region,
                }
            );

            // Frame 4: identical to Frame 3 -> Clean (verifies VRAM prev_buf updated!)
            let out4 = pipeline
                .submit_frame_fused(&frame3, PixelFormat::Rgba8888, width, height, 4, 4, true, false)
                .expect("fused frame 4");
            assert_eq!(out4, GpuFrameOutput::Clean);

            // On-demand VRAM prev_frame readback matches expected3_full byte-for-byte
            let vram_prev = pipeline
                .read_prev_frame(expected3_full.len())
                .expect("read_prev_frame");
            assert_eq!(vram_prev, expected3_full);

            // Frame 5: full green change (>85% dirty) -> Full
            let mut frame5 = vec![0u8; pixel_count * 4];
            for px in frame5.chunks_exact_mut(4) {
                px.copy_from_slice(&[0, 255, 0, 255]);
            }
            let expected5 =
                super::super::rgba8_to_bgr565_be(&frame5, width as usize, height as usize);
            let out5 = pipeline
                .submit_frame_fused(&frame5, PixelFormat::Rgba8888, width, height, 4, 4, true, false)
                .expect("fused frame 5");
            assert_eq!(out5, GpuFrameOutput::Full { pixels: expected5 });
        }
    }
}
