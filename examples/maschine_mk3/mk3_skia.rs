//! Maschine Mk3 GPU Skia Demo: 60 FPS hardware-accelerated 2D graphics
//! rendered across both 480x272 screens via a single wide 960x272 Skia canvas.
//!
//! Run with: cargo run -p encdr-examples --bin mk3_skia

use std::time::{Duration, Instant};

use encdr::{Encdr, EncdrConfig, Event, LedValue};
use encdr_skia::{
    Color, Font, Paint, PaintStyle, PathBuilder, RRect, Rect, SkiaDualScreenView,
};

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".parse().unwrap()),
        )
        .init();

    let mut encdr = Encdr::new(EncdrConfig::default()).expect("Failed to initialize encdr");

    println!("=== Maschine Mk3 GPU Skia Renderer Demo ===");
    println!("Scanning for devices...");

    let ids = encdr.scan().expect("Scan failed");
    if ids.is_empty() {
        eprintln!("No devices found. Is the Maschine Mk3 connected?");
        return;
    }

    let device_id = ids[0];
    let desc = encdr.device_descriptor(device_id).unwrap().clone();
    println!("Connected to: {} ({} controls)", desc.name, desc.control_count());

    std::thread::sleep(Duration::from_millis(500));

    // Create dual-screen Skia renderer
    let mut dual_screen = match SkiaDualScreenView::new(
        &encdr,
        device_id,
        "left_screen",
        "right_screen",
    ) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Failed to create SkiaDualScreenView: {}", e);
            return;
        }
    };

    println!(
        "Skia canvas initialized: {}x{} (GPU accelerated: {})",
        dual_screen.width(),
        dual_screen.height(),
        dual_screen.is_gpu_accelerated()
    );

    let mut font_title = Font::default();
    font_title.set_size(14.0);

    let mut font_body = Font::default();
    font_body.set_size(11.0);

    let mut font_small = Font::default();
    font_small.set_size(9.0);

    // Initial hardware LED state
    encdr.set_led(device_id, "play", LedValue::Bright);
    encdr.set_led(device_id, "channel", LedValue::Bright);

    let start_time = Instant::now();
    let mut frame_count = 0u64;
    let mut fps = 60.0f32;
    let mut fps_timer = Instant::now();

    // Demo state
    let mut freq_param = 3.0f32;
    let mut amp_param = 40.0f32;
    let mut active_pad = 0usize;
    let mut pad_trigger_time = 0.0f32;
    let mut touchstrip_val = 0.5f32;

    let target_frame_duration = Duration::from_nanos(16_666_667); // 60 FPS (16.6ms)

    println!("Rendering 60 FPS GPU graphics. Press Ctrl+C to exit.");

    loop {
        let frame_start = Instant::now();
        let elapsed = start_time.elapsed().as_secs_f32();

        // Process hardware events
        while let Ok(event) = encdr.events().try_recv() {
            match event {
                Event::Encoder { name, delta, .. } => match name {
                    "encoder_1" => {
                        freq_param = (freq_param + delta as f32 * 0.2).clamp(0.5, 12.0);
                    }
                    "encoder_2" => {
                        amp_param = (amp_param + delta as f32 * 2.0).clamp(10.0, 80.0);
                    }
                    _ => {}
                },
                Event::Grid { index, pressure, .. } if pressure > 0.0 => {
                    active_pad = index as usize;
                    pad_trigger_time = elapsed;
                    let vel = (pressure * 255.0) as u8;
                    encdr.set_led(
                        device_id,
                        &format!("pad_{}", index + 1),
                        LedValue::Rgb {
                            r: vel,
                            g: vel / 2,
                            b: 255,
                        },
                    );
                }
                Event::Slider { value, .. } => {
                    touchstrip_val = value;
                }
                _ => {}
            }
        }

        // Render frame across the full double-width 960x272 surface
        let is_gpu = dual_screen.is_gpu_accelerated();
        let current_fps = fps;
        let pad_pulse = ((elapsed - pad_trigger_time) * 4.0).clamp(0.0, 1.0);

        let draw_res = dual_screen.draw(&encdr, |canvas| {
            // Background
            canvas.clear(Color::from_rgb(14, 16, 22));

            let mut paint = Paint::default();
            paint.set_anti_alias(true);

            // ══════════════════════════════════════════════════
            // LEFT SCREEN (0..480): Oscilloscope / Waveform
            // ══════════════════════════════════════════════════

            // Header bar
            paint.set_color(Color::from_rgb(24, 28, 38));
            canvas.draw_rect(Rect::from_xywh(0.0, 0.0, 480.0, 28.0), &paint);

            // Left title
            paint.set_color(Color::from_rgb(220, 225, 235));
            canvas.draw_str("ENCDR :: SKIA GPU OSCILLOSCOPE", (12.0, 19.0), &font_title, &paint);

            // Backend badge
            let badge_color = if is_gpu {
                Color::from_rgb(34, 197, 94) // Green
            } else {
                Color::from_rgb(234, 179, 8) // Amber
            };
            paint.set_color(badge_color);
            let badge_text = if is_gpu { "VULKAN GPU" } else { "CPU FALLBACK" };
            canvas.draw_str(badge_text, (370.0, 19.0), &font_title, &paint);

            // Oscilloscope display frame
            paint.set_color(Color::from_rgb(20, 24, 32));
            let scope_rect = Rect::from_xywh(12.0, 36.0, 456.0, 180.0);
            let scope_rrect = RRect::new_rect_xy(scope_rect, 6.0, 6.0);
            canvas.draw_rrect(scope_rrect, &paint);

            // Grid lines
            paint.set_color(Color::from_argb(40, 255, 255, 255));
            paint.set_style(PaintStyle::Stroke);
            paint.set_stroke_width(1.0);
            for i in 1..4 {
                let y = 36.0 + i as f32 * 45.0;
                canvas.draw_line((12.0, y), (468.0, y), &paint);
            }
            for i in 1..8 {
                let x = 12.0 + i as f32 * 57.0;
                canvas.draw_line((x, 36.0), (x, 216.0), &paint);
            }

            // Animated waveform 1 (Sine wave with modulation)
            paint.set_style(PaintStyle::Stroke);
            paint.set_stroke_width(2.5);
            paint.set_color(Color::from_rgb(56, 189, 248)); // Cyan

            let mut builder = PathBuilder::new();
            let center_y = 126.0f32;
            for x in 0..456 {
                let norm_x = x as f32 / 456.0;
                let phase = elapsed * 3.5;
                let y = center_y
                    + (norm_x * freq_param * 6.283 + phase).sin()
                        * amp_param
                        * (1.0 - (norm_x - 0.5).abs() * 0.4);
                let px = 12.0 + x as f32;
                if x == 0 {
                    builder.move_to((px, y));
                } else {
                    builder.line_to((px, y));
                }
            }
            let wave_path = builder.detach();
            canvas.draw_path(&wave_path, &paint);

            // Animated waveform 2 (Harmonic wave)
            paint.set_stroke_width(1.8);
            paint.set_color(Color::from_argb(180, 244, 63, 94)); // Pink/Rose
            let mut builder2 = PathBuilder::new();
            for x in 0..456 {
                let norm_x = x as f32 / 456.0;
                let phase = -elapsed * 2.2;
                let y = center_y
                    + ((norm_x * freq_param * 12.566 + phase).sin() * 0.5
                        + (norm_x * freq_param * 3.141 + phase * 0.5).cos() * 0.5)
                        * (amp_param * 0.6)
                        * touchstrip_val;
                let px = 12.0 + x as f32;
                if x == 0 {
                    builder2.move_to((px, y));
                } else {
                    builder2.line_to((px, y));
                }
            }
            let wave_path2 = builder2.detach();
            canvas.draw_path(&wave_path2, &paint);

            // Pad ripple animation
            if pad_pulse < 1.0 {
                let ripple_radius = pad_pulse * 90.0;
                let ripple_alpha = ((1.0 - pad_pulse) * 200.0) as u8;
                paint.set_style(PaintStyle::Stroke);
                paint.set_stroke_width(2.0);
                paint.set_color(Color::from_argb(ripple_alpha, 168, 85, 247));
                canvas.draw_circle((240.0, center_y), ripple_radius, &paint);
            }

            // Bottom parameters bar
            paint.set_style(PaintStyle::Fill);
            paint.set_color(Color::from_rgb(148, 163, 184));
            let param_text = format!(
                "ENC 1 (FREQ): {:.1}x   ENC 2 (AMP): {:.0}px   PAD: #{}   STRIP: {:.2}",
                freq_param, amp_param, active_pad + 1, touchstrip_val
            );
            canvas.draw_str(&param_text, (16.0, 248.0), &font_body, &paint);

            // ══════════════════════════════════════════════════
            // RIGHT SCREEN (480..960): Spectrum & VU Meters
            // ══════════════════════════════════════════════════

            // Header bar
            paint.set_color(Color::from_rgb(24, 28, 38));
            canvas.draw_rect(Rect::from_xywh(480.0, 0.0, 480.0, 28.0), &paint);

            // Right title
            paint.set_color(Color::from_rgb(220, 225, 235));
            canvas.draw_str("MULTICHANNEL PEAK METERS", (492.0, 19.0), &font_title, &paint);

            // FPS indicator
            paint.set_color(Color::from_rgb(56, 189, 248));
            let fps_str = format!("{:.1} FPS", current_fps);
            canvas.draw_str(&fps_str, (880.0, 19.0), &font_title, &paint);

            // Divider line between screens
            paint.set_color(Color::from_rgb(45, 52, 68));
            paint.set_style(PaintStyle::Stroke);
            paint.set_stroke_width(1.0);
            canvas.draw_line((480.0, 0.0), (480.0, 272.0), &paint);

            // 8 Spectrum Analyzer / VU Bars
            paint.set_style(PaintStyle::Fill);
            let bar_width = 38.0f32;
            let bar_gap = 16.0f32;
            let start_x = 502.0f32;
            let max_height = 150.0f32;
            let base_y = 210.0f32;

            for i in 0..8 {
                let bx = start_x + i as f32 * (bar_width + bar_gap);

                // Simulated dynamic frequency band amplitude
                let band_freq = (i + 1) as f32 * 1.8;
                let raw_val = (elapsed * band_freq + i as f32 * 0.8).sin() * 0.5 + 0.5;
                let meter_val = (raw_val * 0.85 + (touchstrip_val * 0.3)).min(1.0);
                let bar_h = meter_val * max_height;

                // Background slot
                paint.set_color(Color::from_rgb(24, 28, 38));
                canvas.draw_rect(Rect::from_xywh(bx, base_y - max_height, bar_width, max_height), &paint);

                // Active meter bar with gradient-like levels
                let bar_color = if meter_val > 0.85 {
                    Color::from_rgb(239, 68, 68) // Red peak
                } else if meter_val > 0.65 {
                    Color::from_rgb(245, 158, 11) // Amber warn
                } else {
                    Color::from_rgb(34, 197, 94) // Green normal
                };
                paint.set_color(bar_color);
                canvas.draw_rect(Rect::from_xywh(bx, base_y - bar_h, bar_width, bar_h), &paint);

                // Peak hold line
                paint.set_color(Color::from_rgb(255, 255, 255));
                let peak_y = (base_y - bar_h - 3.0).max(base_y - max_height);
                canvas.draw_rect(Rect::from_xywh(bx, peak_y, bar_width, 2.0), &paint);

                // Channel label
                paint.set_color(Color::from_rgb(148, 163, 184));
                let ch_str = format!("CH{}", i + 1);
                canvas.draw_str(&ch_str, (bx + 8.0, base_y + 16.0), &font_small, &paint);
            }

            // Vector master dials
            let dial_center_x = 910.0f32;
            let dial_center_y = 100.0f32;
            paint.set_style(PaintStyle::Stroke);
            paint.set_stroke_width(3.0);
            paint.set_color(Color::from_rgb(51, 65, 85));
            canvas.draw_circle((dial_center_x, dial_center_y), 24.0, &paint);

            // Dial value arc
            paint.set_color(Color::from_rgb(56, 189, 248));
            let dial_angle = (elapsed * 2.0).sin() * 1.5;
            let end_x = dial_center_x + dial_angle.sin() * 20.0;
            let end_y = dial_center_y - dial_angle.cos() * 20.0;
            canvas.draw_line((dial_center_x, dial_center_y), (end_x, end_y), &paint);

            // Master label
            paint.set_style(PaintStyle::Fill);
            paint.set_color(Color::from_rgb(148, 163, 184));
            canvas.draw_str("MST", (dial_center_x - 12.0, dial_center_y + 38.0), &font_small, &paint);
        });

        if let Err(e) = draw_res {
            tracing::warn!("Skia render failed: {}", e);
        }

        // FPS calculation
        frame_count += 1;
        if fps_timer.elapsed() >= Duration::from_millis(500) {
            let elapsed_secs = fps_timer.elapsed().as_secs_f32();
            fps = frame_count as f32 / elapsed_secs;
            frame_count = 0;
            fps_timer = Instant::now();
        }

        // Throttle to 60 FPS
        let frame_duration = frame_start.elapsed();
        if frame_duration < target_frame_duration {
            std::thread::sleep(target_frame_duration - frame_duration);
        }
    }
}
