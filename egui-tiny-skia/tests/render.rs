use egui_tiny_skia::TinySkiaBackend;

#[test]
fn renders_window_with_label_to_non_magenta_pixels() {
    let mut backend = TinySkiaBackend::new(800, 600);
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0))),
        ..Default::default()
    };

    let output = backend.run_frame(input, |ctx| {
        egui::Window::new("test")
            .fixed_pos(egui::pos2(20.0, 20.0))
            .fixed_size(egui::vec2(200.0, 120.0))
            .show(ctx, |ui| {
                ui.label("hello");
            });
    });

    let pixmap = backend.paint(output);
    let has_non_magenta = pixmap.pixels().iter().enumerate().any(|(index, pixel)| {
        let x = index % 800;
        let y = index / 800;
        (0..400).contains(&x) && (0..300).contains(&y) && !(pixel.red() == 255 && pixel.green() == 0 && pixel.blue() == 255 && pixel.alpha() == 255)
    });

    assert!(has_non_magenta);
}
