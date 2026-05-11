use egui::epaint::{Mesh, Primitive, Vertex};
use egui::{Color32, TextureId};
use tiny_skia::{Pixmap, PremultipliedColorU8};

use crate::TextureStore;

pub fn paint_primitives(pixmap: &mut Pixmap, primitives: &[egui::ClippedPrimitive], textures: &TextureStore) {
    for primitive in primitives {
        match &primitive.primitive {
            Primitive::Mesh(mesh) => paint_mesh(pixmap, primitive.clip_rect, mesh, textures),
            Primitive::Callback(_) => {}
        }
    }
}

fn paint_mesh(pixmap: &mut Pixmap, clip_rect: egui::Rect, mesh: &Mesh, textures: &TextureStore) {
    for indices in mesh.indices.chunks_exact(3) {
        let a = mesh.vertices[indices[0] as usize];
        let b = mesh.vertices[indices[1] as usize];
        let c = mesh.vertices[indices[2] as usize];
        paint_triangle(pixmap, clip_rect, mesh.texture_id, textures, a, b, c);
    }
}

fn paint_triangle(pixmap: &mut Pixmap, clip_rect: egui::Rect, texture_id: TextureId, textures: &TextureStore, a: Vertex, b: Vertex, c: Vertex) {
    let width = pixmap.width() as i32;
    let height = pixmap.height() as i32;

    let min_x = a.pos.x.min(b.pos.x).min(c.pos.x).floor().max(clip_rect.min.x.floor()).max(0.0) as i32;
    let max_x = a.pos.x.max(b.pos.x).max(c.pos.x).ceil().min(clip_rect.max.x.ceil()).min(width as f32) as i32;
    let min_y = a.pos.y.min(b.pos.y).min(c.pos.y).floor().max(clip_rect.min.y.floor()).max(0.0) as i32;
    let max_y = a.pos.y.max(b.pos.y).max(c.pos.y).ceil().min(clip_rect.max.y.ceil()).min(height as f32) as i32;

    if min_x >= max_x || min_y >= max_y {
        return;
    }

    let area = edge(a.pos, b.pos, c.pos);
    if area.abs() <= f32::EPSILON {
        return;
    }

    for y in min_y..max_y {
        for x in min_x..max_x {
            let p = egui::pos2(x as f32 + 0.5, y as f32 + 0.5);
            let w0 = edge(b.pos, c.pos, p) / area;
            let w1 = edge(c.pos, a.pos, p) / area;
            let w2 = edge(a.pos, b.pos, p) / area;

            let epsilon = -0.0001;
            if w0 < epsilon || w1 < epsilon || w2 < epsilon {
                continue;
            }

            let vertex_color = interpolate_color(a.color, b.color, c.color, w0, w1, w2);
            let texture_color = textures
                .get(texture_id)
                .map(|texture| {
                    let u = a.uv.x * w0 + b.uv.x * w1 + c.uv.x * w2;
                    let v = a.uv.y * w0 + b.uv.y * w1 + c.uv.y * w2;
                    texture.sample(u, v)
                })
                .unwrap_or(Color32::WHITE);
            let source = multiply_color(vertex_color, texture_color);

            blend_pixel(pixmap, x as u32, y as u32, source);
        }
    }
}

fn edge(a: egui::Pos2, b: egui::Pos2, c: egui::Pos2) -> f32 {
    (c.x - a.x) * (b.y - a.y) - (c.y - a.y) * (b.x - a.x)
}

fn interpolate_color(a: Color32, b: Color32, c: Color32, w0: f32, w1: f32, w2: f32) -> Color32 {
    Color32::from_rgba_premultiplied(
        interpolate_channel(a.r(), b.r(), c.r(), w0, w1, w2),
        interpolate_channel(a.g(), b.g(), c.g(), w0, w1, w2),
        interpolate_channel(a.b(), b.b(), c.b(), w0, w1, w2),
        interpolate_channel(a.a(), b.a(), c.a(), w0, w1, w2),
    )
}

fn interpolate_channel(a: u8, b: u8, c: u8, w0: f32, w1: f32, w2: f32) -> u8 {
    ((a as f32 * w0 + b as f32 * w1 + c as f32 * w2).round()).clamp(0.0, 255.0) as u8
}

fn multiply_color(a: Color32, b: Color32) -> Color32 {
    Color32::from_rgba_premultiplied(
        multiply_channel(a.r(), b.r()),
        multiply_channel(a.g(), b.g()),
        multiply_channel(a.b(), b.b()),
        multiply_channel(a.a(), b.a()),
    )
}

fn multiply_channel(a: u8, b: u8) -> u8 {
    ((a as u16 * b as u16 + 127) / 255) as u8
}

fn blend_pixel(pixmap: &mut Pixmap, x: u32, y: u32, source: Color32) {
    if source.a() == 0 {
        return;
    }

    let index = y as usize * pixmap.width() as usize + x as usize;
    let destination = pixmap.pixels_mut()[index];
    let source_alpha = source.a() as u16;
    let inverse_alpha = 255 - source_alpha;

    let red = source.r() as u16 + destination.red() as u16 * inverse_alpha / 255;
    let green = source.g() as u16 + destination.green() as u16 * inverse_alpha / 255;
    let blue = source.b() as u16 + destination.blue() as u16 * inverse_alpha / 255;
    let alpha = source_alpha + destination.alpha() as u16 * inverse_alpha / 255;

    pixmap.pixels_mut()[index] = PremultipliedColorU8::from_rgba(red.min(255) as u8, green.min(255) as u8, blue.min(255) as u8, alpha.min(255) as u8)
        .expect("blended color must be valid premultiplied rgba");
}
