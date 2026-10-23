use encdr_skia::{Color, Paint, PaintStyle, Rect, SkiaGpuContext};

#[test]
fn test_cpu_surface_rendering() {
    let mut ctx = SkiaGpuContext::cpu();
    assert!(!ctx.is_gpu_accelerated());

    let mut surface = ctx.create_surface(100, 100).expect("create surface");
    let canvas = surface.canvas();
    canvas.clear(Color::BLACK);

    let mut paint = Paint::default();
    paint.set_color(Color::from_rgb(255, 0, 0));
    paint.set_style(PaintStyle::Fill);

    canvas.draw_rect(Rect::from_xywh(10.0, 10.0, 20.0, 20.0), &paint);

    let mut pixels = vec![0u8; 100 * 100 * 4];
    let image_info = skia_safe::ImageInfo::new(
        (100, 100),
        skia_safe::ColorType::RGBA8888,
        skia_safe::AlphaType::Premul,
        Some(skia_safe::ColorSpace::new_srgb()),
    );
    assert!(surface.read_pixels(&image_info, &mut pixels, 100 * 4, (0, 0)));

    // Check pixel at (0, 0) is black (0, 0, 0, 255)
    let idx_bg = 0;
    assert_eq!(pixels[idx_bg + 0], 0);
    assert_eq!(pixels[idx_bg + 1], 0);
    assert_eq!(pixels[idx_bg + 2], 0);
    assert_eq!(pixels[idx_bg + 3], 255);

    // Check pixel inside rect (15, 15) is red (255, 0, 0, 255)
    let idx_rect = (15 * 100 + 15) * 4;
    assert_eq!(pixels[idx_rect + 0], 255);
    assert_eq!(pixels[idx_rect + 1], 0);
    assert_eq!(pixels[idx_rect + 2], 0);
    assert_eq!(pixels[idx_rect + 3], 255);
}

#[test]
fn test_gpu_context_fallback_or_vulkan() {
    let mut ctx = SkiaGpuContext::new();
    println!("GPU accelerated: {}", ctx.is_gpu_accelerated());

    let mut surface = ctx.create_surface(480, 272).expect("create 480x272 surface");
    let canvas = surface.canvas();
    canvas.clear(Color::from_rgb(40, 50, 60));

    let mut paint = Paint::default();
    paint.set_color(Color::from_rgb(0, 255, 128));
    canvas.draw_circle((240.0, 136.0), 50.0, &paint);

    ctx.flush_and_submit(&mut surface);

    let mut pixels = vec![0u8; 480 * 272 * 4];
    let image_info = skia_safe::ImageInfo::new(
        (480, 272),
        skia_safe::ColorType::RGBA8888,
        skia_safe::AlphaType::Premul,
        Some(skia_safe::ColorSpace::new_srgb()),
    );
    assert!(surface.read_pixels(&image_info, &mut pixels, 480 * 4, (0, 0)));

    // Check center pixel (240, 136) is green-ish
    let center_idx = (136 * 480 + 240) * 4;
    assert_eq!(pixels[center_idx + 0], 0);
    assert_eq!(pixels[center_idx + 1], 255);
    assert_eq!(pixels[center_idx + 2], 128);
    assert_eq!(pixels[center_idx + 3], 255);
}
