//! Renders a few primitives and a line of text, then writes a PNG.
//!
//! This is the visual smoke test: if it produces a sane image, the whole
//! Direct2D + DirectWrite + DIB pipeline works, including headless rendering.

use statline_render::{Color, Palette, Renderer, type_scale};

#[test]
fn renders_shapes_and_text_to_a_png() {
    let mut renderer = Renderer::new().expect("Direct2D/DirectWrite available");
    let mut surface = renderer.surface(420, 140).expect("surface");

    let palette = Palette::dark(Color::rgb(0x4C, 0xC2, 0xFF));
    surface.begin(palette.surface);
    surface.fill_round_rect(16.0, 16.0, 150.0, 44.0, 8.0, palette.accent);
    surface.fill_rect(16.0, 76.0, 260.0, 6.0, palette.meter_track);
    surface.fill_rect(16.0, 76.0, 160.0, 6.0, palette.accent);
    surface.fill_round_rect(300.0, 16.0, 100.0, 44.0, 12.0, palette.hover);
    surface.fill_rect(16.0, 104.0, 120.0, 2.0, palette.warning);
    surface.fill_rect(150.0, 104.0, 120.0, 2.0, palette.critical);

    renderer
        .text
        .draw(
            &mut surface,
            "CPU 23%",
            (24.0, 24.0, 140.0, 30.0),
            type_scale::title(),
            palette.text_primary,
        )
        .expect("draw text");
    renderer
        .text
        .draw(
            &mut surface,
            "9.8 / 16.0 GiB",
            (16.0, 112.0, 300.0, 20.0),
            type_scale::small(),
            palette.text_muted,
        )
        .expect("draw text");

    surface.end();

    let pixels = surface.pixels_rgba();
    assert_eq!(pixels.len(), 420 * 140 * 4);
    assert!(
        pixels.chunks(4).any(|pixel| pixel[3] > 0),
        "surface must not be fully transparent"
    );

    let path = std::env::temp_dir().join("statline-render-smoke.png");
    surface.save_png(&path).expect("write png");
    let size = std::fs::metadata(&path).expect("png exists").len();
    assert!(
        size > 500,
        "png should contain image data, got {size} bytes"
    );

    println!("snapshot: {}", path.display());
}
