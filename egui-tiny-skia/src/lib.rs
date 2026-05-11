mod painter;
mod texture;

pub use texture::{TextureStore, TextureStoreError};

use egui::epaint::{ClippedShape, Shape};
use egui::{Context, FullOutput, RawInput};
use tiny_skia::{Color, Pixmap};

pub struct TinySkiaBackend {
    ctx: Context,
    pixmap: Pixmap,
    textures: TextureStore,
}

impl TinySkiaBackend {
    pub fn new(width: u32, height: u32) -> Self {
        let pixmap = Pixmap::new(width, height).expect("pixmap dimensions must be non-zero");
        let ctx = Context::default();
        ctx.set_fonts(egui::FontDefinitions::default());
        let mut textures = TextureStore::default();
        let warmup_input = RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width as f32, height as f32))),
            ..Default::default()
        };
        let warmup_output = ctx.run(warmup_input, |_| {});
        textures.apply_delta(warmup_output.textures_delta).expect("failed to apply egui font texture delta");

        Self { ctx, pixmap, textures }
    }

    pub fn context(&self) -> &Context {
        &self.ctx
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.pixmap = Pixmap::new(width, height).expect("pixmap dimensions must be non-zero");
    }

    pub fn run_frame(&mut self, input: RawInput, mut ui: impl FnMut(&Context)) -> FullOutput {
        let retry_input = input.clone();
        let output = self.ctx.run(input, |ctx| ui(ctx));

        if should_retry_after_texture_upload(&output) {
            self.textures
                .apply_delta(output.textures_delta.clone())
                .expect("failed to apply egui texture delta before retry");
            self.ctx.run(retry_input, |ctx| ui(ctx))
        } else {
            output
        }
    }

    pub fn paint(&mut self, output: FullOutput) -> &Pixmap {
        self.pixmap.fill(Color::from_rgba8(255, 0, 255, 255));

        self.textures.apply_delta(output.textures_delta).expect("failed to apply egui texture delta");

        let primitives = self.ctx.tessellate(output.shapes, output.pixels_per_point);
        painter::paint_primitives(&mut self.pixmap, &primitives, &self.textures);

        &self.pixmap
    }

    pub fn pixmap(&self) -> &Pixmap {
        &self.pixmap
    }
}

fn should_retry_after_texture_upload(output: &FullOutput) -> bool {
    !output.textures_delta.set.is_empty() && !output.shapes.iter().any(shape_has_paint)
}

fn shape_has_paint(shape: &ClippedShape) -> bool {
    unclipped_shape_has_paint(&shape.shape)
}

fn unclipped_shape_has_paint(shape: &Shape) -> bool {
    match shape {
        Shape::Noop => false,
        Shape::Vec(shapes) => shapes.iter().any(unclipped_shape_has_paint),
        _ => true,
    }
}
