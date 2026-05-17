use egui::epaint::{Mesh, Primitive, Vertex};
use egui::{Color32, TextureId};
use tiny_skia::{Pixmap, PremultipliedColorU8};

use crate::texture::CpuTexture;
use crate::TextureStore;

#[derive(Debug, Default, Clone, Copy)]
pub struct PaintStats {
    pub mesh_count: usize,
    pub callback_count: usize,
    pub triangle_count: usize,
    pub candidate_pixel_count: u64,
    pub shaded_pixel_count: u64,
    pub blended_pixel_count: u64,
    pub texture_sample_count: u64,
    pub fast_rect_count: usize,
    pub fast_rect_pixels: u64,
    pub generic_triangle_count: usize,
    pub fast_rect_miss_missing_texture: usize,
    pub fast_rect_miss_not_quad: usize,
    pub fast_rect_miss_non_uniform_color: usize,
    pub fast_rect_miss_non_rect_geometry: usize,
    pub fast_rect_miss_empty_clip: usize,
}

pub fn paint_primitives(pixmap: &mut Pixmap, primitives: &[egui::ClippedPrimitive], textures: &TextureStore) -> PaintStats {
    let mut stats = PaintStats::default();

    for primitive in primitives {
        match &primitive.primitive {
            Primitive::Mesh(mesh) => {
                stats.mesh_count += 1;
                paint_mesh(pixmap, primitive.clip_rect, mesh, textures, &mut stats);
            }
            Primitive::Callback(_) => {
                stats.callback_count += 1;
            }
        }
    }

    stats
}

fn paint_mesh(pixmap: &mut Pixmap, clip_rect: egui::Rect, mesh: &Mesh, textures: &TextureStore, stats: &mut PaintStats) {
    match detect_fast_textured_rect(mesh, clip_rect, textures) {
        FastRectMatch::Matched { texture, rect } => {
            paint_textured_rect(pixmap, texture, rect, stats);
            return;
        }
        FastRectMatch::Missed(reason) => stats.record_fast_rect_miss(reason),
    }

    for indices in mesh.indices.chunks_exact(3) {
        let a = mesh.vertices[indices[0] as usize];
        let b = mesh.vertices[indices[1] as usize];
        let c = mesh.vertices[indices[2] as usize];
        paint_triangle(pixmap, clip_rect, mesh.texture_id, textures, a, b, c, stats);
    }
}

impl PaintStats {
    fn record_fast_rect_miss(&mut self, reason: FastRectMissReason) {
        match reason {
            FastRectMissReason::MissingTexture => self.fast_rect_miss_missing_texture += 1,
            FastRectMissReason::NotQuad => self.fast_rect_miss_not_quad += 1,
            FastRectMissReason::NonUniformColor => self.fast_rect_miss_non_uniform_color += 1,
            FastRectMissReason::NonRectGeometry => self.fast_rect_miss_non_rect_geometry += 1,
            FastRectMissReason::EmptyClip => self.fast_rect_miss_empty_clip += 1,
        }
    }
}

fn paint_triangle(
    pixmap: &mut Pixmap,
    clip_rect: egui::Rect,
    texture_id: TextureId,
    textures: &TextureStore,
    a: Vertex,
    b: Vertex,
    c: Vertex,
    stats: &mut PaintStats,
) {
    stats.triangle_count += 1;
    stats.generic_triangle_count += 1;

    let width = pixmap.width() as i32;
    let height = pixmap.height() as i32;

    let min_x = a.pos.x.min(b.pos.x).min(c.pos.x).floor().max(clip_rect.min.x.floor()).max(0.0) as i32;
    let max_x = a.pos.x.max(b.pos.x).max(c.pos.x).ceil().min(clip_rect.max.x.ceil()).min(width as f32) as i32;
    let min_y = a.pos.y.min(b.pos.y).min(c.pos.y).floor().max(clip_rect.min.y.floor()).max(0.0) as i32;
    let max_y = a.pos.y.max(b.pos.y).max(c.pos.y).ceil().min(clip_rect.max.y.ceil()).min(height as f32) as i32;

    if min_x >= max_x || min_y >= max_y {
        return;
    }

    stats.candidate_pixel_count += ((max_x - min_x) as u64) * ((max_y - min_y) as u64);

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

            stats.shaded_pixel_count += 1;
            let vertex_color = interpolate_color(a.color, b.color, c.color, w0, w1, w2);
            let texture_color = textures
                .get(texture_id)
                .map(|texture| {
                    stats.texture_sample_count += 1;
                    let u = a.uv.x * w0 + b.uv.x * w1 + c.uv.x * w2;
                    let v = a.uv.y * w0 + b.uv.y * w1 + c.uv.y * w2;
                    texture.sample(u, v)
                })
                .unwrap_or(Color32::WHITE);
            let source = multiply_color(vertex_color, texture_color);

            if blend_pixel(pixmap, x as u32, y as u32, source) {
                stats.blended_pixel_count += 1;
            }
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

fn blend_pixel(pixmap: &mut Pixmap, x: u32, y: u32, source: Color32) -> bool {
    if source.a() == 0 {
        return false;
    }

    let index = y as usize * pixmap.width() as usize + x as usize;
    if source.a() == 255 {
        pixmap.pixels_mut()[index] = PremultipliedColorU8::from_rgba(source.r(), source.g(), source.b(), 255).expect("opaque color must be valid premultiplied rgba");
        return true;
    }

    let destination = pixmap.pixels_mut()[index];
    let source_alpha = source.a() as u16;
    let inverse_alpha = 255 - source_alpha;

    let red = source.r() as u16 + destination.red() as u16 * inverse_alpha / 255;
    let green = source.g() as u16 + destination.green() as u16 * inverse_alpha / 255;
    let blue = source.b() as u16 + destination.blue() as u16 * inverse_alpha / 255;
    let alpha = source_alpha + destination.alpha() as u16 * inverse_alpha / 255;

    pixmap.pixels_mut()[index] = PremultipliedColorU8::from_rgba(red.min(255) as u8, green.min(255) as u8, blue.min(255) as u8, alpha.min(255) as u8)
        .expect("blended color must be valid premultiplied rgba");
    true
}

#[derive(Clone, Copy)]
struct TexturedRect {
    pos: egui::Rect,
    uv: egui::Rect,
    color: Color32,
}

enum FastRectMatch<'a> {
    Matched { texture: &'a CpuTexture, rect: TexturedRect },
    Missed(FastRectMissReason),
}

#[derive(Clone, Copy)]
enum FastRectMissReason {
    MissingTexture,
    NotQuad,
    NonUniformColor,
    NonRectGeometry,
    EmptyClip,
}

fn detect_fast_textured_rect<'a>(mesh: &Mesh, clip_rect: egui::Rect, textures: &'a TextureStore) -> FastRectMatch<'a> {
    let Some(texture) = textures.get(mesh.texture_id) else {
        return FastRectMatch::Missed(FastRectMissReason::MissingTexture);
    };

    if mesh.vertices.len() != 4 || mesh.indices.len() != 6 {
        return FastRectMatch::Missed(FastRectMissReason::NotQuad);
    }

    let color = mesh.vertices[0].color;
    if color.a() != 255 || !mesh.vertices.iter().all(|vertex| vertex.color == color) {
        return FastRectMatch::Missed(FastRectMissReason::NonUniformColor);
    }

    let min_x = mesh.vertices.iter().map(|vertex| vertex.pos.x).fold(f32::INFINITY, f32::min);
    let max_x = mesh.vertices.iter().map(|vertex| vertex.pos.x).fold(f32::NEG_INFINITY, f32::max);
    let min_y = mesh.vertices.iter().map(|vertex| vertex.pos.y).fold(f32::INFINITY, f32::min);
    let max_y = mesh.vertices.iter().map(|vertex| vertex.pos.y).fold(f32::NEG_INFINITY, f32::max);
    let min_u = mesh.vertices.iter().map(|vertex| vertex.uv.x).fold(f32::INFINITY, f32::min);
    let max_u = mesh.vertices.iter().map(|vertex| vertex.uv.x).fold(f32::NEG_INFINITY, f32::max);
    let min_v = mesh.vertices.iter().map(|vertex| vertex.uv.y).fold(f32::INFINITY, f32::min);
    let max_v = mesh.vertices.iter().map(|vertex| vertex.uv.y).fold(f32::NEG_INFINITY, f32::max);

    if min_x >= max_x || min_y >= max_y || min_u > max_u || min_v > max_v {
        return FastRectMatch::Missed(FastRectMissReason::NonRectGeometry);
    }

    for vertex in &mesh.vertices {
        let on_x_edge = approx_eq(vertex.pos.x, min_x) || approx_eq(vertex.pos.x, max_x);
        let on_y_edge = approx_eq(vertex.pos.y, min_y) || approx_eq(vertex.pos.y, max_y);
        let on_u_edge = approx_eq(vertex.uv.x, min_u) || approx_eq(vertex.uv.x, max_u);
        let on_v_edge = approx_eq(vertex.uv.y, min_v) || approx_eq(vertex.uv.y, max_v);
        if !(on_x_edge && on_y_edge && on_u_edge && on_v_edge) {
            return FastRectMatch::Missed(FastRectMissReason::NonRectGeometry);
        }
    }

    let pos = egui::Rect::from_min_max(egui::pos2(min_x, min_y), egui::pos2(max_x, max_y)).intersect(clip_rect);
    if !pos.is_positive() {
        return FastRectMatch::Missed(FastRectMissReason::EmptyClip);
    }

    FastRectMatch::Matched {
        texture,
        rect: TexturedRect {
            pos,
            uv: egui::Rect::from_min_max(egui::pos2(min_u, min_v), egui::pos2(max_u, max_v)),
            color,
        },
    }
}

fn paint_textured_rect(pixmap: &mut Pixmap, texture: &CpuTexture, rect: TexturedRect, stats: &mut PaintStats) {
    let min_x = rect.pos.min.x.floor().max(0.0) as i32;
    let max_x = rect.pos.max.x.ceil().min(pixmap.width() as f32) as i32;
    let min_y = rect.pos.min.y.floor().max(0.0) as i32;
    let max_y = rect.pos.max.y.ceil().min(pixmap.height() as f32) as i32;
    if min_x >= max_x || min_y >= max_y {
        return;
    }

    stats.fast_rect_count += 1;
    stats.triangle_count += 2;

    let pos_width = rect.pos.width().max(f32::EPSILON);
    let pos_height = rect.pos.height().max(f32::EPSILON);
    for y in min_y..max_y {
        let ty = ((y as f32 + 0.5) - rect.pos.min.y) / pos_height;
        let v = lerp(rect.uv.min.y, rect.uv.max.y, ty);
        for x in min_x..max_x {
            stats.fast_rect_pixels += 1;
            stats.shaded_pixel_count += 1;
            stats.texture_sample_count += 1;

            let tx = ((x as f32 + 0.5) - rect.pos.min.x) / pos_width;
            let u = lerp(rect.uv.min.x, rect.uv.max.x, tx);
            let texture_color = texture.sample(u, v);
            let source = multiply_color(rect.color, texture_color);
            if blend_pixel(pixmap, x as u32, y as u32, source) {
                stats.blended_pixel_count += 1;
            }
        }
    }
}

fn lerp(min: f32, max: f32, t: f32) -> f32 {
    min + (max - min) * t
}

fn approx_eq(a: f32, b: f32) -> bool {
    (a - b).abs() <= 0.001
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::epaint::ImageDelta;
    use egui::{TextureOptions, TexturesDelta};
    use tiny_skia::Color;

    fn test_mesh(texture_id: TextureId, color: Color32) -> Mesh {
        Mesh {
            indices: vec![0, 1, 2, 0, 2, 3],
            vertices: vec![
                Vertex {
                    pos: egui::pos2(1.0, 1.0),
                    uv: egui::pos2(0.0, 0.0),
                    color,
                },
                Vertex {
                    pos: egui::pos2(3.0, 1.0),
                    uv: egui::pos2(1.0, 0.0),
                    color,
                },
                Vertex {
                    pos: egui::pos2(3.0, 3.0),
                    uv: egui::pos2(1.0, 1.0),
                    color,
                },
                Vertex {
                    pos: egui::pos2(1.0, 3.0),
                    uv: egui::pos2(0.0, 1.0),
                    color,
                },
            ],
            texture_id,
        }
    }

    fn texture_store(pixels: Vec<Color32>) -> TextureStore {
        let texture_id = TextureId::Managed(0);
        let image = egui::ColorImage::new([2, 2], pixels);
        let delta = TexturesDelta {
            set: vec![(texture_id, ImageDelta::full(image, TextureOptions::NEAREST))],
            free: Vec::new(),
        };
        let mut textures = TextureStore::default();
        textures.apply_delta(delta).unwrap();
        textures
    }

    fn transparent_pixmap() -> Pixmap {
        let mut pixmap = Pixmap::new(4, 4).unwrap();
        pixmap.fill(Color::from_rgba8(0, 0, 0, 0));
        pixmap
    }

    #[test]
    fn blend_pixel_skips_transparent_and_overwrites_opaque() {
        let mut pixmap = transparent_pixmap();

        assert!(!blend_pixel(&mut pixmap, 1, 1, Color32::TRANSPARENT));
        assert_eq!(pixmap.pixels()[5].alpha(), 0);

        assert!(blend_pixel(&mut pixmap, 1, 1, Color32::RED));
        let pixel = pixmap.pixels()[5];
        assert_eq!((pixel.red(), pixel.green(), pixel.blue(), pixel.alpha()), (255, 0, 0, 255));
    }

    #[test]
    fn detects_strict_axis_aligned_textured_rect() {
        let mesh = test_mesh(TextureId::Managed(0), Color32::WHITE);
        let textures = texture_store(vec![Color32::RED, Color32::TRANSPARENT, Color32::GREEN, Color32::BLUE]);
        let FastRectMatch::Matched { rect, .. } = detect_fast_textured_rect(&mesh, egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(4.0, 4.0)), &textures) else {
            panic!("expected fast rect match");
        };

        assert_eq!(rect.pos.min, egui::pos2(1.0, 1.0));
        assert_eq!(rect.pos.max, egui::pos2(3.0, 3.0));
        assert_eq!(rect.uv.min, egui::pos2(0.0, 0.0));
        assert_eq!(rect.uv.max, egui::pos2(1.0, 1.0));
    }

    #[test]
    fn rejects_non_uniform_vertex_color_for_fast_rect() {
        let mut mesh = test_mesh(TextureId::Managed(0), Color32::WHITE);
        mesh.vertices[0].color = Color32::RED;

        let textures = texture_store(vec![Color32::RED, Color32::TRANSPARENT, Color32::GREEN, Color32::BLUE]);
        assert!(matches!(
            detect_fast_textured_rect(&mesh, egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(4.0, 4.0)), &textures),
            FastRectMatch::Missed(FastRectMissReason::NonUniformColor)
        ));
    }

    #[test]
    fn binary_alpha_fast_rect_matches_generic_triangle_output() {
        let texture_id = TextureId::Managed(0);
        let textures = texture_store(vec![Color32::RED, Color32::TRANSPARENT, Color32::GREEN, Color32::BLUE]);
        let mesh = test_mesh(texture_id, Color32::WHITE);
        let clip_rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(4.0, 4.0));

        let mut fast = transparent_pixmap();
        let fast_stats = paint_primitives(
            &mut fast,
            &[egui::ClippedPrimitive {
                clip_rect,
                primitive: Primitive::Mesh(mesh.clone()),
            }],
            &textures,
        );

        let mut generic = transparent_pixmap();
        let mut generic_stats = PaintStats::default();
        for indices in mesh.indices.chunks_exact(3) {
            paint_triangle(
                &mut generic,
                clip_rect,
                mesh.texture_id,
                &textures,
                mesh.vertices[indices[0] as usize],
                mesh.vertices[indices[1] as usize],
                mesh.vertices[indices[2] as usize],
                &mut generic_stats,
            );
        }

        assert_eq!(fast_stats.fast_rect_count, 1);
        assert_eq!(fast_stats.generic_triangle_count, 0);
        assert_eq!(fast.pixels(), generic.pixels());
    }

    #[test]
    fn mixed_alpha_texture_uses_fast_rect_with_blending() {
        let texture_id = TextureId::Managed(0);
        let textures = texture_store(vec![Color32::RED, Color32::from_rgba_unmultiplied(0, 255, 0, 128), Color32::GREEN, Color32::BLUE]);
        let mesh = test_mesh(texture_id, Color32::WHITE);
        let clip_rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(4.0, 4.0));
        let mut pixmap = transparent_pixmap();

        let stats = paint_primitives(
            &mut pixmap,
            &[egui::ClippedPrimitive {
                clip_rect,
                primitive: Primitive::Mesh(mesh),
            }],
            &textures,
        );

        assert_eq!(stats.fast_rect_count, 1);
        assert_eq!(stats.generic_triangle_count, 0);
        assert!(stats.blended_pixel_count > 0);
    }
}
