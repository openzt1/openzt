use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use egui::{Align2, Color32, Context, FontId, Painter, Pos2, Rect, TextureHandle, Vec2, pos2, vec2};
use openzt_configparser::ini::Ini;
use tracing::{info, warn};

use super::zt_image;

static TEXTURES: OnceLock<Mutex<TextureCache>> = OnceLock::new();

const GOLD_TEXT: Color32 = Color32::from_rgb(255, 212, 60);
const GREEN_TEXT: Color32 = Color32::from_rgb(83, 219, 83);
const PANEL_BROWN: Color32 = Color32::from_rgb(68, 36, 20);

struct TextureCache {
    animations: HashMap<&'static str, CachedTexture>,
}

impl TextureCache {
    fn new() -> Self {
        Self {
            animations: HashMap::new(),
        }
    }

    fn animation(&mut self, ctx: &Context, base: &'static str) -> Option<LoadedTexture> {
        let entry = self.animations.entry(base).or_default();
        entry.texture(ctx, base)
    }
}

#[derive(Default)]
struct CachedTexture {
    texture: Option<TextureHandle>,
    size: Vec2,
    failed: bool,
    missing_logged: bool,
}

impl CachedTexture {
    fn texture(&mut self, ctx: &Context, base: &'static str) -> Option<LoadedTexture> {
        if let Some(texture) = &self.texture {
            return Some(LoadedTexture {
                texture: texture.clone(),
                size: self.size,
            });
        }

        if self.failed {
            return None;
        }

        let Some(texture) = load_animation_texture(ctx, base, &mut self.missing_logged) else {
            return None;
        };

        self.size = texture.size;
        self.texture = Some(texture.texture.clone());
        Some(texture)
    }
}

#[derive(Clone)]
struct LoadedTexture {
    texture: TextureHandle,
    size: Vec2,
}

#[derive(Clone, Copy)]
struct DrawnRect {
    rect: Rect,
    loaded: bool,
}

pub fn show(ctx: &Context, screen_size: Vec2) {
    egui::Area::new("openzt_vanilla_main_ui".into())
        .order(egui::Order::Background)
        .fixed_pos(Pos2::ZERO)
        .interactable(false)
        .show(ctx, |ui| {
            ui.set_min_size(screen_size);
            let painter = ui.painter().clone();
            let cache = TEXTURES.get_or_init(|| Mutex::new(TextureCache::new()));
            let mut cache = match cache.lock() {
                Ok(cache) => cache,
                Err(err) => {
                    warn!("egui overlay: vanilla UI texture cache lock poisoned: {err}");
                    return;
                }
            };

            draw_main_ui(ctx, &painter, &mut cache, screen_size);
        });
}

fn draw_main_ui(ctx: &Context, painter: &Painter, cache: &mut TextureCache, screen_size: Vec2) {
    let bg1 = draw_anim(ctx, painter, cache, "ui/main/backgnd1/backgnd1", pos2(0.0, 0.0), vec2(64.0, 248.0));
    let bg2_size = texture_size(ctx, cache, "ui/main/backgnd2/backgnd2").unwrap_or(vec2(170.0, 128.0));
    let bg3_size = texture_size(ctx, cache, "ui/main/backgnd3/backgnd3").unwrap_or(vec2(200.0, 112.0));
    let bg4_size = texture_size(ctx, cache, "ui/main/backgnd4/backgnd4").unwrap_or(vec2(330.0, 38.0));
    let bg5_size = texture_size(ctx, cache, "ui/main/backgnd5/backgnd5").unwrap_or(vec2(256.0, 38.0));

    let bg2_pos = pos2(0.0, (screen_size.y - bg2_size.y).max(0.0));
    let bg3_pos = pos2(0.0, (screen_size.y - bg3_size.y).max(0.0));
    let bg4_pos = pos2(((screen_size.x - bg4_size.x) * 0.5).max(0.0), (screen_size.y - bg4_size.y).max(0.0));
    let bg5_pos = pos2((screen_size.x - bg5_size.x).max(0.0), (screen_size.y - bg5_size.y).max(0.0));

    let bg2 = rect_from_pos_size(bg2_pos, bg2_size);
    let bg3 = rect_from_pos_size(bg3_pos, bg3_size);
    let bg4 = rect_from_pos_size(bg4_pos, bg4_size);
    let bg5 = rect_from_pos_size(bg5_pos, bg5_size);

    if bg2.top() > bg1.rect.bottom() {
        draw_tiled(
            ctx,
            painter,
            cache,
            "ui/main/bg2/bg2",
            Rect::from_min_max(pos2(0.0, bg1.rect.bottom()), pos2(bg2_size.x, bg2.top())),
        );
    }
    if bg4.left() > bg3.right() {
        draw_tiled(ctx, painter, cache, "ui/main/bg3/bg3", Rect::from_min_max(pos2(bg3.right(), bg3.top()), pos2(bg4.left(), screen_size.y)));
    }
    if bg5.left() > bg4.right() {
        draw_tiled(ctx, painter, cache, "ui/main/bg4/bg4", Rect::from_min_max(pos2(bg4.right(), bg4.top()), pos2(bg5.left(), screen_size.y)));
    }

    draw_anim(ctx, painter, cache, "ui/main/backgnd2/backgnd2", bg2_pos, bg2_size);
    draw_anim(ctx, painter, cache, "ui/main/backgnd3/backgnd3", bg3_pos, bg3_size);
    draw_anim(ctx, painter, cache, "ui/main/backgnd4/backgnd4", bg4_pos, bg4_size);
    draw_anim(ctx, painter, cache, "ui/main/backgnd5/backgnd5", bg5_pos, bg5_size);

    draw_left_buttons(ctx, painter, cache, bg1.rect);
    draw_minimap_cluster(ctx, painter, cache, bg2);
    draw_time_and_money(ctx, painter, cache, bg3, bg4);
    draw_status_cluster(ctx, painter, cache, bg4, bg5);
}

fn draw_left_buttons(ctx: &Context, painter: &Painter, cache: &mut TextureCache, bg1: Rect) {
    let buttons = [
        ("ui/main/habitat/habitat", 4.0, 13.0),
        ("ui/main/buyanim/buyanim", 4.0, 60.0),
        ("ui/main/buyobj/buyobj", 4.0, 108.0),
        ("ui/main/person/person", 4.0, 154.0),
        ("ui/main/bdoz/bdoz", 1.0, 293.0),
        ("ui/main/msgs/msgs", 1.0, 328.0),
        ("ui/main/resr/resr", 1.0, 363.0),
        ("ui/scenario/scenbut/scenbut", 1.0, 398.0),
        ("ui/main/gameopt/gameopt", 1.0, 433.0),
    ];

    for (resource, x, y) in buttons {
        draw_anim(ctx, painter, cache, resource, bg1.min + vec2(x, y), vec2(40.0, 40.0));
    }
}

fn draw_minimap_cluster(ctx: &Context, painter: &Painter, cache: &mut TextureCache, bg2: Rect) {
    draw_anim(ctx, painter, cache, "ui/sharedui/snap/snap", bg2.min + vec2(5.0, 86.0), vec2(34.0, 34.0));
    draw_anim(ctx, painter, cache, "ui/main/zoomin/zoomin", bg2.min + vec2(14.0, 17.0), vec2(28.0, 28.0));
    draw_anim(ctx, painter, cache, "ui/main/rotr/rotr", bg2.min + vec2(6.0, 40.0), vec2(28.0, 28.0));
    draw_anim(ctx, painter, cache, "ui/main/rotl/rotl", bg2.min + vec2(26.0, 27.0), vec2(28.0, 28.0));
    draw_anim(ctx, painter, cache, "ui/main/trees/trees", bg2.min + vec2(147.0, 81.0), vec2(28.0, 28.0));
    draw_anim(ctx, painter, cache, "ui/main/guests/guests", bg2.min + vec2(127.0, 90.0), vec2(28.0, 28.0));
    draw_anim(ctx, painter, cache, "ui/main/builds/builds", bg2.min + vec2(106.0, 100.0), vec2(28.0, 28.0));

    let minimap = Rect::from_min_size(bg2.min + vec2(10.0, 44.0), vec2(139.0, 69.0));
    painter.rect_filled(minimap, 0.0, Color32::from_rgb(31, 74, 53));
    painter.rect_stroke(minimap, 0.0, egui::Stroke::new(1.0, Color32::from_rgb(13, 32, 23)), egui::StrokeKind::Inside);
}

fn draw_time_and_money(ctx: &Context, painter: &Painter, cache: &mut TextureCache, bg3: Rect, bg4: Rect) {
    let pause = draw_anim(ctx, painter, cache, "ui/main/pause/pause", bg3.min + vec2(170.0, 80.0), vec2(34.0, 34.0));
    let date_rect = Rect::from_min_size(pause.rect.min + vec2(25.0, 7.0), vec2(108.0, 18.0));
    painter.rect_filled(date_rect, 0.0, PANEL_BROWN.gamma_multiply(0.55));
    painter.text(date_rect.center(), Align2::CENTER_CENTER, "Jan 1, Year 1", FontId::proportional(14.0), GREEN_TEXT);

    let money_rect = Rect::from_min_size(bg4.min + vec2(90.0, 9.0), vec2(125.0, 18.0));
    painter.rect_filled(money_rect, 0.0, PANEL_BROWN.gamma_multiply(0.55));
    painter.text(money_rect.center(), Align2::CENTER_CENTER, "$50,000", FontId::proportional(14.0), GOLD_TEXT);
}

fn draw_status_cluster(ctx: &Context, painter: &Painter, cache: &mut TextureCache, bg4: Rect, bg5: Rect) {
    draw_status(ctx, painter, cache, "ui/main/zstat/zstat", bg4.min + vec2(231.0, 3.0), true);
    draw_status(ctx, painter, cache, "ui/main/astat/astat", bg5.min + vec2(0.0, 3.0), true);
    draw_status(ctx, painter, cache, "ui/main/gstat/gstat", bg5.min + vec2(85.0, 3.0), true);
    draw_anim(ctx, painter, cache, "ui/main/hstat/hstat", bg5.min + vec2(170.0, 3.0), vec2(34.0, 34.0));
    draw_anim(ctx, painter, cache, "ui/main/staff/staff", bg5.min + vec2(206.0, 3.0), vec2(34.0, 34.0));
}

fn draw_status(ctx: &Context, painter: &Painter, cache: &mut TextureCache, button: &'static str, pos: Pos2, with_meter: bool) {
    let button_rect = draw_anim(ctx, painter, cache, button, pos, vec2(34.0, 34.0)).rect;
    if !with_meter {
        return;
    }

    draw_anim(ctx, painter, cache, "ui/main/progbck/progbck", button_rect.min + vec2(26.0, 5.0), vec2(56.0, 22.0));
    let meter = Rect::from_min_size(button_rect.min + vec2(32.0, 9.0), vec2(45.0, 9.0));
    painter.rect_filled(meter, 0.0, Color32::from_rgb(48, 88, 31));
    painter.rect_filled(Rect::from_min_size(meter.min, vec2(34.0, meter.height())), 0.0, Color32::from_rgb(66, 196, 59));
}

fn draw_anim(
    ctx: &Context,
    painter: &Painter,
    cache: &mut TextureCache,
    resource: &'static str,
    pos: Pos2,
    fallback_size: Vec2,
) -> DrawnRect {
    let loaded = cache.animation(ctx, resource);
    let size = loaded.as_ref().map(|texture| texture.size).unwrap_or(fallback_size);
    let rect = rect_from_pos_size(pos, size);
    let loaded_image = loaded.is_some();
    if let Some(texture) = loaded {
        painter.image(texture.texture.id(), rect, unit_uv(), Color32::WHITE);
    }

    DrawnRect {
        rect,
        loaded: loaded_image,
    }
}

fn draw_tiled(ctx: &Context, painter: &Painter, cache: &mut TextureCache, resource: &'static str, rect: Rect) {
    if rect.width() <= 0.0 || rect.height() <= 0.0 {
        return;
    }

    let Some(texture) = cache.animation(ctx, resource) else {
        return;
    };
    let tile_size = texture.size;
    if tile_size.x <= 0.0 || tile_size.y <= 0.0 {
        return;
    }

    let mut y = rect.top();
    while y < rect.bottom() {
        let mut x = rect.left();
        let height = tile_size.y.min(rect.bottom() - y);
        while x < rect.right() {
            let width = tile_size.x.min(rect.right() - x);
            let dest = Rect::from_min_size(pos2(x, y), vec2(width, height));
            let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(width / tile_size.x, height / tile_size.y));
            painter.image(texture.texture.id(), dest, uv, Color32::WHITE);
            x += tile_size.x;
        }
        y += tile_size.y;
    }
}

fn texture_size(ctx: &Context, cache: &mut TextureCache, resource: &'static str) -> Option<Vec2> {
    cache.animation(ctx, resource).map(|texture| texture.size)
}

fn load_animation_texture(ctx: &Context, base: &'static str, missing_logged: &mut bool) -> Option<LoadedTexture> {
    let descriptor_name = format!("{base}.ani");
    let Some((descriptor_source, descriptor_data)) = crate::resource_manager::lazyresourcemap::get_file(&descriptor_name) else {
        log_missing(missing_logged, &descriptor_name);
        return None;
    };

    let descriptor = String::from_utf8_lossy(&descriptor_data).into_owned();
    let mut ini = Ini::new_cs();
    if let Err(err) = ini.read(descriptor) {
        warn!("egui overlay: failed to parse animation descriptor {descriptor_name}: {err}");
        return None;
    }

    let resource_name = match animation_resource_name(&ini) {
        Some(resource_name) => resource_name,
        None => {
            warn!("egui overlay: animation descriptor {descriptor_name} has no animation entries");
            return None;
        }
    };
    let palette_name = format!("{base}.pal");

    let Some((animation_source, animation_data)) = crate::resource_manager::lazyresourcemap::get_file(&resource_name) else {
        log_missing(missing_logged, &resource_name);
        return None;
    };
    let Some((palette_source, palette_data)) = crate::resource_manager::lazyresourcemap::get_file(&palette_name) else {
        log_missing(missing_logged, &palette_name);
        return None;
    };

    match zt_image::decode_animation_frames(&animation_data, &palette_data) {
        Ok((_animation, frames)) => {
            let Some(image) = frames.into_iter().next() else {
                warn!("egui overlay: animation {resource_name} decoded with no frames");
                return None;
            };
            let size = vec2(image.size[0] as f32, image.size[1] as f32);
            let texture = ctx.load_texture(format!("vanilla-main:{base}"), image, egui::TextureOptions::NEAREST);
            info!(
                "egui overlay: loaded vanilla UI asset {base} using {descriptor_source}, {animation_source}, {palette_source} as {}x{}",
                size.x, size.y
            );
            Some(LoadedTexture { texture, size })
        }
        Err(err) => {
            warn!("egui overlay: failed to decode vanilla UI asset {base}: {err}");
            None
        }
    }
}

fn animation_resource_name(ini: &Ini) -> Option<String> {
    let mut dirs = Vec::new();
    for index in 0.. {
        let Some(dir) = ini.get("animation", &format!("dir{index}")) else {
            break;
        };
        dirs.push(dir);
    }

    let animations = ini.get_vec("animation", "animation")?;
    let animation = animations
        .iter()
        .find(|animation| animation.eq_ignore_ascii_case("N"))
        .or_else(|| animations.first())?;

    dirs.push(animation.clone());
    Some(dirs.join("/"))
}

fn log_missing(missing_logged: &mut bool, resource: &str) {
    if !*missing_logged {
        info!("egui overlay: vanilla UI resource not available yet: {resource}");
        *missing_logged = true;
    }
}

fn rect_from_pos_size(pos: Pos2, size: Vec2) -> Rect {
    Rect::from_min_size(pos, vec2(size.x.max(0.0), size.y.max(0.0)))
}

fn unit_uv() -> Rect {
    Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0))
}
