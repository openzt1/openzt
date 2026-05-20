use std::path::PathBuf;

use egui_tiny_skia::TinySkiaBackend;

fn main() {
    let mut backend = TinySkiaBackend::new(800, 600);
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(800.0, 600.0),
        )),
        ..Default::default()
    };

    let output = backend.run_frame(input, |ctx| {
        egui::Window::new("test")
            .fixed_pos(egui::pos2(20.0, 20.0))
            .fixed_size(egui::vec2(220.0, 120.0))
            .show(ctx, |ui| {
                ui.heading("OpenZT");
                ui.label("egui rendered through tiny-skia");
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label("Status:");
                    ui.colored_label(egui::Color32::LIGHT_GREEN, "native render OK");
                });
            });
    });

    let pixmap = backend.paint(output);
    let output_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("egui-tiny-skia-render.png");
    std::fs::create_dir_all(output_path.parent().expect("png path must have a parent"))
        .expect("failed to create output directory");
    pixmap.save_png(&output_path).expect("failed to save png");
    println!("{}", output_path.display());
}
