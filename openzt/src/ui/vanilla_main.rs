use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::{Mutex, OnceLock};

use egui::{
    pos2, vec2, Align2, Color32, ColorImage, Context, FontData, FontDefinitions, FontFamily, FontId, Painter, PointerButton, Pos2, Rect, Sense, TextureHandle,
    Ui, Vec2,
};
use openzt_configparser::ini::Ini;
use tracing::{info, warn};

use super::zt_image;

static TEXTURES: OnceLock<Mutex<TextureCache>> = OnceLock::new();
static BUTTONS: OnceLock<Mutex<ButtonState>> = OnceLock::new();
static HIT_REGIONS: OnceLock<Mutex<Vec<HitRegion>>> = OnceLock::new();
static BOLD_FONT_REGISTERED: AtomicBool = AtomicBool::new(false);
static BOLD_FONT_ACTIVE: AtomicBool = AtomicBool::new(false);

const BOLD_FONT_FAMILY: &str = "zt-bold";
const BOLD_FONT_NAME: &str = "arial-bold";
const BOLD_FONT_PATH: &str = r"C:\Windows\Fonts\arialbd.ttf";
const GREEN_TEXT: Color32 = Color32::from_rgb(83, 219, 83);

pub fn blocks_pointer_at(pos: Pos2, screen_size: Vec2) -> bool {
    if let Ok(regions) = HIT_REGIONS.get_or_init(|| Mutex::new(Vec::new())).lock()
        && !regions.is_empty()
    {
        return regions.iter().rev().any(|region| region.blocks(pos));
    }

    fallback_main_ui_block_rects(screen_size, |rect| rect.contains(pos))
}

struct TextureCache {
    animations: HashMap<TextureKey, CachedTexture>,
}

impl TextureCache {
    fn new() -> Self {
        Self { animations: HashMap::new() }
    }

    fn animation(&mut self, ctx: &Context, base: &'static str, visual_state: VisualState) -> Option<LoadedTexture> {
        let key = TextureKey { base, visual_state };
        let entry = self.animations.entry(key).or_default();
        entry.texture(ctx, base, visual_state)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct TextureKey {
    base: &'static str,
    visual_state: VisualState,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum VisualState {
    Normal,
    Hover,
    Selected,
}

impl VisualState {
    fn animation_name(self) -> &'static str {
        match self {
            Self::Normal => "N",
            Self::Hover => "H",
            Self::Selected => "S",
        }
    }
}

#[derive(Default)]
struct ButtonState {
    selected: HashSet<&'static str>,
}

#[derive(Clone, Copy)]
enum ButtonMode {
    Momentary,
    Selected,
}

#[derive(Default)]
struct CachedTexture {
    texture: Option<TextureHandle>,
    size: Vec2,
    mask: Option<Arc<HitMask>>,
    failed: bool,
    missing_logged: bool,
}

impl CachedTexture {
    fn texture(&mut self, ctx: &Context, base: &'static str, visual_state: VisualState) -> Option<LoadedTexture> {
        if let Some(texture) = &self.texture {
            return Some(LoadedTexture {
                texture: texture.clone(),
                size: self.size,
                mask: self.mask.clone()?,
            });
        }

        if self.failed {
            return None;
        }

        let Some(texture) = load_animation_texture(ctx, base, visual_state, &mut self.missing_logged) else {
            return None;
        };

        self.size = texture.size;
        self.texture = Some(texture.texture.clone());
        self.mask = Some(texture.mask.clone());
        Some(texture)
    }
}

#[derive(Clone)]
struct LoadedTexture {
    texture: TextureHandle,
    size: Vec2,
    mask: Arc<HitMask>,
}

#[derive(Clone, Copy)]
struct DrawnRect {
    rect: Rect,
    loaded: bool,
}

#[derive(Clone)]
struct HitRegion {
    rect: Rect,
    uv: Rect,
    mask: Arc<HitMask>,
}

impl HitRegion {
    fn blocks(&self, pos: Pos2) -> bool {
        if !self.rect.contains(pos) || self.rect.width() <= 0.0 || self.rect.height() <= 0.0 {
            return false;
        }

        let local_x = (pos.x - self.rect.left()) / self.rect.width();
        let local_y = (pos.y - self.rect.top()) / self.rect.height();
        let u = self.uv.left() + local_x * self.uv.width();
        let v = self.uv.top() + local_y * self.uv.height();
        self.mask.blocks_uv(u, v)
    }
}

#[derive(Clone)]
struct HitMask {
    width: usize,
    height: usize,
    alpha: Vec<bool>,
}

impl HitMask {
    fn from_image(image: &ColorImage) -> Self {
        Self {
            width: image.size[0],
            height: image.size[1],
            alpha: image.pixels.iter().map(|pixel| pixel.a() > 0).collect(),
        }
    }

    fn blocks_uv(&self, u: f32, v: f32) -> bool {
        if self.width == 0 || self.height == 0 || !u.is_finite() || !v.is_finite() {
            return false;
        }

        let x = (u.clamp(0.0, 1.0) * self.width as f32).floor() as usize;
        let y = (v.clamp(0.0, 1.0) * self.height as f32).floor() as usize;
        let x = x.min(self.width - 1);
        let y = y.min(self.height - 1);
        self.alpha.get(y * self.width + x).copied().unwrap_or(false)
    }
}

pub fn show(ctx: &Context, screen_size: Vec2) {
    prepare_bold_font(ctx);

    egui::Area::new("openzt_vanilla_main_ui".into())
        .order(egui::Order::Background)
        .fixed_pos(Pos2::ZERO)
        .interactable(true)
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

            let buttons = BUTTONS.get_or_init(|| Mutex::new(ButtonState::default()));
            let mut buttons = match buttons.lock() {
                Ok(buttons) => buttons,
                Err(err) => {
                    warn!("egui overlay: vanilla UI button state lock poisoned: {err}");
                    return;
                }
            };

            let mut hit_regions = Vec::new();
            draw_main_ui(ctx, ui, &painter, &mut cache, &mut buttons, &mut hit_regions, screen_size);
            remember_hit_regions(hit_regions);
        });
}

fn draw_main_ui(
    ctx: &Context,
    ui: &mut Ui,
    painter: &Painter,
    cache: &mut TextureCache,
    buttons: &mut ButtonState,
    hit_regions: &mut Vec<HitRegion>,
    screen_size: Vec2,
) {
    let bg1_size = texture_size(ctx, cache, "ui/main/backgnd1/backgnd1", VisualState::Normal).unwrap_or(vec2(64.0, 248.0));
    let bg2_size = texture_size(ctx, cache, "ui/main/backgnd2/backgnd2", VisualState::Normal).unwrap_or(vec2(170.0, 128.0));
    let bg3_size = texture_size(ctx, cache, "ui/main/backgnd3/backgnd3", VisualState::Normal).unwrap_or(vec2(200.0, 112.0));
    let bg4_size = texture_size(ctx, cache, "ui/main/backgnd4/backgnd4", VisualState::Normal).unwrap_or(vec2(330.0, 38.0));
    let bg5_size = texture_size(ctx, cache, "ui/main/backgnd5/backgnd5", VisualState::Normal).unwrap_or(vec2(256.0, 38.0));

    let bg1_rect = rect_from_pos_size(pos2(0.0, 0.0), bg1_size);
    let bg2_pos = pos2(0.0, (screen_size.y - bg2_size.y).max(0.0));
    let bg3_pos = pos2(0.0, (screen_size.y - bg3_size.y).max(0.0));
    let bg4_pos = pos2(((screen_size.x - bg4_size.x) * 0.5).max(0.0), (screen_size.y - bg4_size.y).max(0.0));
    let bg5_pos = pos2((screen_size.x - bg5_size.x).max(0.0), (screen_size.y - bg5_size.y).max(0.0));

    let bg2 = rect_from_pos_size(bg2_pos, bg2_size);
    let bg3 = rect_from_pos_size(bg3_pos, bg3_size);
    let bg4 = rect_from_pos_size(bg4_pos, bg4_size);
    let bg5 = rect_from_pos_size(bg5_pos, bg5_size);

    if bg2.top() > bg1_rect.bottom() {
        draw_tiled_y(ctx, painter, cache, hit_regions, "ui/main/bg2/bg2", pos2(0.0, bg1_rect.bottom()), bg2.top() - bg1_rect.bottom());
    }
    if bg4.left() > bg3.right() {
        draw_tiled_x_bottom(ctx, painter, cache, hit_regions, "ui/main/bg3/bg3", bg3.right(), screen_size.y, bg4.left() - bg3.right());
    }
    if bg5.left() > bg4.right() {
        draw_tiled_x_bottom(ctx, painter, cache, hit_regions, "ui/main/bg4/bg4", bg4.right(), screen_size.y, bg5.left() - bg4.right());
    }

    draw_anim(ctx, painter, cache, hit_regions, "ui/main/backgnd4/backgnd4", bg4_pos, bg4_size);
    let bg1 = draw_anim(ctx, painter, cache, hit_regions, "ui/main/backgnd1/backgnd1", pos2(0.0, 0.0), bg1_size);
    draw_anim(ctx, painter, cache, hit_regions, "ui/main/backgnd2/backgnd2", bg2_pos, bg2_size);
    draw_anim(ctx, painter, cache, hit_regions, "ui/main/backgnd3/backgnd3", bg3_pos, bg3_size);
    draw_anim(ctx, painter, cache, hit_regions, "ui/main/backgnd5/backgnd5", bg5_pos, bg5_size);

    draw_left_buttons(ctx, ui, painter, cache, buttons, hit_regions, bg1.rect);
    draw_minimap_cluster(ctx, ui, painter, cache, buttons, hit_regions, bg2);
    draw_time_and_money(ctx, ui, painter, cache, buttons, hit_regions, bg3, bg4);
    draw_status_cluster(ctx, ui, painter, cache, buttons, hit_regions, bg4, bg5);
}

fn remember_hit_regions(hit_regions: Vec<HitRegion>) {
    if let Ok(mut stored) = HIT_REGIONS.get_or_init(|| Mutex::new(Vec::new())).lock() {
        *stored = hit_regions;
    }
}

fn fallback_main_ui_block_rects(screen_size: Vec2, mut visit: impl FnMut(Rect) -> bool) -> bool {
    let bg1 = Rect::from_min_size(pos2(0.0, 0.0), vec2(64.0, 248.0));
    let bg2 = Rect::from_min_size(pos2(0.0, (screen_size.y - 128.0).max(0.0)), vec2(170.0, 128.0));
    let bg3 = Rect::from_min_size(pos2(0.0, (screen_size.y - 112.0).max(0.0)), vec2(200.0, 112.0));
    let bg4 = Rect::from_min_size(pos2(((screen_size.x - 330.0) * 0.5).max(0.0), (screen_size.y - 38.0).max(0.0)), vec2(330.0, 38.0));
    let bg5 = Rect::from_min_size(pos2((screen_size.x - 256.0).max(0.0), (screen_size.y - 38.0).max(0.0)), vec2(256.0, 38.0));

    if [bg1, bg2, bg3, bg4, bg5].into_iter().any(&mut visit) {
        return true;
    }
    if bg2.top() > bg1.bottom() {
        if visit(Rect::from_min_max(pos2(0.0, bg1.bottom()), pos2(64.0, bg2.top()))) {
            return true;
        }
    }
    if bg4.left() > bg3.right() {
        if visit(Rect::from_min_max(
            pos2(bg3.right(), (screen_size.y - 112.0).max(0.0)),
            pos2(bg4.left(), screen_size.y),
        )) {
            return true;
        }
    }
    if bg5.left() > bg4.right() {
        if visit(Rect::from_min_max(
            pos2(bg4.right(), (screen_size.y - 38.0).max(0.0)),
            pos2(bg5.left(), screen_size.y),
        )) {
            return true;
        }
    }

    false
}

fn draw_left_buttons(
    ctx: &Context,
    ui: &mut Ui,
    painter: &Painter,
    cache: &mut TextureCache,
    buttons_state: &mut ButtonState,
    hit_regions: &mut Vec<HitRegion>,
    bg1: Rect,
) {
    let buttons = [
        ("ui/main/habitat/habitat", 4.0, 13.0),
        ("ui/main/buyanim/buyanim", 4.0, 60.0),
        ("ui/main/buyobj/buyobj", 4.0, 108.0),
        ("ui/main/person/person", 4.0, 154.0),
        ("ui/main/undo/undo", 1.0, 258.0),
        ("ui/main/bdoz/bdoz", 1.0, 293.0),
        ("ui/main/msgs/msgs", 1.0, 328.0),
        ("ui/main/resr/resr", 1.0, 363.0),
        ("ui/scenario/scenbut/scenbut", 1.0, 398.0),
        ("ui/main/gameopt/gameopt", 1.0, 433.0),
    ];

    for (resource, x, y) in buttons {
        draw_button(
            ctx,
            ui,
            painter,
            cache,
            buttons_state,
            hit_regions,
            resource,
            bg1.min + vec2(x, y),
            vec2(40.0, 40.0),
            ButtonMode::Selected,
        );
    }
}

fn draw_minimap_cluster(
    ctx: &Context,
    ui: &mut Ui,
    painter: &Painter,
    cache: &mut TextureCache,
    buttons: &mut ButtonState,
    hit_regions: &mut Vec<HitRegion>,
    bg2: Rect,
) {
    draw_button(
        ctx,
        ui,
        painter,
        cache,
        buttons,
        hit_regions,
        "ui/sharedui/snap/snap",
        bg2.min + vec2(5.0, 86.0),
        vec2(34.0, 34.0),
        ButtonMode::Momentary,
    );
    draw_button(
        ctx,
        ui,
        painter,
        cache,
        buttons,
        hit_regions,
        "ui/main/zoomin/zoomin",
        bg2.min + vec2(14.0, 17.0),
        vec2(28.0, 28.0),
        ButtonMode::Momentary,
    );
    draw_button(
        ctx,
        ui,
        painter,
        cache,
        buttons,
        hit_regions,
        "ui/main/zoomout/zoomout",
        bg2.min + vec2(5.0, 24.0),
        vec2(28.0, 28.0),
        ButtonMode::Momentary,
    );
    draw_button(
        ctx,
        ui,
        painter,
        cache,
        buttons,
        hit_regions,
        "ui/main/rotr/rotr",
        bg2.min + vec2(6.0, 40.0),
        vec2(28.0, 28.0),
        ButtonMode::Momentary,
    );
    draw_button(
        ctx,
        ui,
        painter,
        cache,
        buttons,
        hit_regions,
        "ui/main/rotl/rotl",
        bg2.min + vec2(26.0, 27.0),
        vec2(28.0, 28.0),
        ButtonMode::Momentary,
    );
    draw_button(
        ctx,
        ui,
        painter,
        cache,
        buttons,
        hit_regions,
        "ui/main/trees/trees",
        bg2.min + vec2(147.0, 81.0),
        vec2(28.0, 28.0),
        ButtonMode::Selected,
    );
    draw_button(
        ctx,
        ui,
        painter,
        cache,
        buttons,
        hit_regions,
        "ui/main/guests/guests",
        bg2.min + vec2(127.0, 90.0),
        vec2(28.0, 28.0),
        ButtonMode::Selected,
    );
    draw_button(
        ctx,
        ui,
        painter,
        cache,
        buttons,
        hit_regions,
        "ui/main/builds/builds",
        bg2.min + vec2(106.0, 100.0),
        vec2(28.0, 28.0),
        ButtonMode::Selected,
    );

    let _minimap = Rect::from_min_size(bg2.min + vec2(10.0, 44.0), vec2(139.0, 69.0));
}

fn draw_time_and_money(
    ctx: &Context,
    ui: &mut Ui,
    painter: &Painter,
    cache: &mut TextureCache,
    buttons: &mut ButtonState,
    hit_regions: &mut Vec<HitRegion>,
    bg3: Rect,
    bg4: Rect,
) {
    let pause = draw_button(
        ctx,
        ui,
        painter,
        cache,
        buttons,
        hit_regions,
        "ui/main/pause/pause",
        bg3.min + vec2(170.0, 80.0),
        vec2(34.0, 34.0),
        ButtonMode::Selected,
    );
    let date_rect = Rect::from_min_size(pause.rect.min + vec2(25.0, 7.0), vec2(108.0, 18.0));
    painter.text(date_rect.center(), Align2::CENTER_CENTER, "Jan 1, Year 1", bold_font(14.0), GREEN_TEXT);

    let money_rect = Rect::from_min_size(bg4.min + vec2(90.0, 9.0), vec2(125.0, 18.0));
    painter.text(money_rect.center(), Align2::CENTER_CENTER, "$50,000", bold_font(14.0), GREEN_TEXT);
}

fn draw_status_cluster(
    ctx: &Context,
    ui: &mut Ui,
    painter: &Painter,
    cache: &mut TextureCache,
    buttons: &mut ButtonState,
    hit_regions: &mut Vec<HitRegion>,
    bg4: Rect,
    bg5: Rect,
) {
    draw_status(ctx, ui, painter, cache, buttons, hit_regions, "ui/main/zstat/zstat", bg4.min + vec2(231.0, 3.0), true);
    draw_status(ctx, ui, painter, cache, buttons, hit_regions, "ui/main/astat/astat", bg5.min + vec2(0.0, 3.0), true);
    draw_status(ctx, ui, painter, cache, buttons, hit_regions, "ui/main/gstat/gstat", bg5.min + vec2(85.0, 3.0), true);
    draw_button(
        ctx,
        ui,
        painter,
        cache,
        buttons,
        hit_regions,
        "ui/main/hstat/hstat",
        bg5.min + vec2(170.0, 3.0),
        vec2(34.0, 34.0),
        ButtonMode::Selected,
    );
    draw_button(
        ctx,
        ui,
        painter,
        cache,
        buttons,
        hit_regions,
        "ui/main/staff/staff",
        bg5.min + vec2(206.0, 3.0),
        vec2(34.0, 34.0),
        ButtonMode::Selected,
    );
}

fn draw_status(
    ctx: &Context,
    ui: &mut Ui,
    painter: &Painter,
    cache: &mut TextureCache,
    buttons: &mut ButtonState,
    hit_regions: &mut Vec<HitRegion>,
    button: &'static str,
    pos: Pos2,
    with_meter: bool,
) {
    if with_meter {
        draw_anim(ctx, painter, cache, hit_regions, "ui/main/progbck/progbck", pos + vec2(26.0, 5.0), vec2(56.0, 22.0));
        let meter = Rect::from_min_size(pos + vec2(32.0, 9.0), vec2(45.0, 9.0));
        painter.rect_filled(meter, 0.0, Color32::from_rgb(48, 88, 31));
        painter.rect_filled(Rect::from_min_size(meter.min, vec2(34.0, meter.height())), 0.0, Color32::from_rgb(66, 196, 59));
    }
    draw_button(ctx, ui, painter, cache, buttons, hit_regions, button, pos, vec2(34.0, 34.0), ButtonMode::Selected);
}

fn draw_button(
    ctx: &Context,
    ui: &mut Ui,
    painter: &Painter,
    cache: &mut TextureCache,
    buttons: &mut ButtonState,
    hit_regions: &mut Vec<HitRegion>,
    resource: &'static str,
    pos: Pos2,
    fallback_size: Vec2,
    mode: ButtonMode,
) -> DrawnRect {
    let size = texture_size(ctx, cache, resource, VisualState::Normal).unwrap_or(fallback_size);
    let rect = rect_from_pos_size(pos, size);
    let response = ui.interact(rect, ui.make_persistent_id(resource), Sense::click());

    if matches!(mode, ButtonMode::Selected) && response.clicked_by(PointerButton::Primary) {
        buttons.selected.insert(resource);
    }

    let selected = buttons.selected.contains(resource);
    let visual_state = if matches!(mode, ButtonMode::Momentary) && response.is_pointer_button_down_on() {
        VisualState::Selected
    } else if selected {
        VisualState::Selected
    } else if response.hovered() {
        VisualState::Hover
    } else {
        VisualState::Normal
    };

    draw_anim_state(ctx, painter, cache, hit_regions, resource, pos, size, visual_state)
}

fn draw_anim(
    ctx: &Context,
    painter: &Painter,
    cache: &mut TextureCache,
    hit_regions: &mut Vec<HitRegion>,
    resource: &'static str,
    pos: Pos2,
    fallback_size: Vec2,
) -> DrawnRect {
    draw_anim_state(ctx, painter, cache, hit_regions, resource, pos, fallback_size, VisualState::Normal)
}

fn draw_anim_state(
    ctx: &Context,
    painter: &Painter,
    cache: &mut TextureCache,
    hit_regions: &mut Vec<HitRegion>,
    resource: &'static str,
    pos: Pos2,
    fallback_size: Vec2,
    visual_state: VisualState,
) -> DrawnRect {
    let loaded = cache
        .animation(ctx, resource, visual_state)
        .or_else(|| cache.animation(ctx, resource, VisualState::Normal));
    let size = loaded.as_ref().map(|texture| texture.size).unwrap_or(fallback_size);
    let rect = rect_from_pos_size(pos, size);
    let loaded_image = loaded.is_some();
    if let Some(texture) = loaded {
        let uv = unit_uv();
        painter.image(texture.texture.id(), rect, uv, Color32::WHITE);
        hit_regions.push(HitRegion {
            rect,
            uv,
            mask: texture.mask.clone(),
        });
    }

    DrawnRect { rect, loaded: loaded_image }
}

fn draw_tiled_y(
    ctx: &Context,
    painter: &Painter,
    cache: &mut TextureCache,
    hit_regions: &mut Vec<HitRegion>,
    resource: &'static str,
    pos: Pos2,
    height: f32,
) {
    if height <= 0.0 {
        return;
    }

    let Some(texture) = cache.animation(ctx, resource, VisualState::Normal) else {
        return;
    };
    let tile_size = texture.size;
    if tile_size.x <= 0.0 || tile_size.y <= 0.0 {
        return;
    }

    let bottom = pos.y + height;
    let mut y = pos.y;
    while y < bottom {
        let tile_height = tile_size.y.min(bottom - y);
        let dest = Rect::from_min_size(pos2(pos.x, y), vec2(tile_size.x, tile_height));
        let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, tile_height / tile_size.y));
        painter.image(texture.texture.id(), dest, uv, Color32::WHITE);
        hit_regions.push(HitRegion {
            rect: dest,
            uv,
            mask: texture.mask.clone(),
        });
        y += tile_size.y;
    }
}

fn draw_tiled_x_bottom(
    ctx: &Context,
    painter: &Painter,
    cache: &mut TextureCache,
    hit_regions: &mut Vec<HitRegion>,
    resource: &'static str,
    left: f32,
    bottom: f32,
    width: f32,
) {
    if width <= 0.0 {
        return;
    }

    let Some(texture) = cache.animation(ctx, resource, VisualState::Normal) else {
        return;
    };
    let tile_size = texture.size;
    if tile_size.x <= 0.0 || tile_size.y <= 0.0 {
        return;
    }

    let top = bottom - tile_size.y;
    let right = left + width;
    let mut x = left;
    while x < right {
        let tile_width = tile_size.x.min(right - x);
        let dest = Rect::from_min_size(pos2(x, top), vec2(tile_width, tile_size.y));
        let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(tile_width / tile_size.x, 1.0));
        painter.image(texture.texture.id(), dest, uv, Color32::WHITE);
        hit_regions.push(HitRegion {
            rect: dest,
            uv,
            mask: texture.mask.clone(),
        });
        x += tile_size.x;
    }
}

fn texture_size(ctx: &Context, cache: &mut TextureCache, resource: &'static str, visual_state: VisualState) -> Option<Vec2> {
    cache.animation(ctx, resource, visual_state).map(|texture| texture.size)
}

fn load_animation_texture(ctx: &Context, base: &'static str, visual_state: VisualState, missing_logged: &mut bool) -> Option<LoadedTexture> {
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

    let resource_name = match animation_resource_name(&ini, visual_state) {
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
            let mask = Arc::new(HitMask::from_image(&image));
            let texture = ctx.load_texture(format!("vanilla-main:{base}:{}", visual_state.animation_name()), image, egui::TextureOptions::NEAREST);
            info!(
                "egui overlay: loaded vanilla UI asset {base} using {descriptor_source}, {animation_source}, {palette_source} as {}x{}",
                size.x, size.y
            );
            Some(LoadedTexture { texture, size, mask })
        }
        Err(err) => {
            warn!("egui overlay: failed to decode vanilla UI asset {base}: {err}");
            None
        }
    }
}

fn animation_resource_name(ini: &Ini, visual_state: VisualState) -> Option<String> {
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
        .find(|animation| animation.eq_ignore_ascii_case(visual_state.animation_name()))
        .or_else(|| animations.iter().find(|animation| animation.eq_ignore_ascii_case("N")))
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

fn bold_font(size: f32) -> FontId {
    if BOLD_FONT_ACTIVE.load(Ordering::Acquire) {
        FontId::new(size, FontFamily::Name(BOLD_FONT_FAMILY.into()))
    } else {
        FontId::proportional(size)
    }
}

fn prepare_bold_font(ctx: &Context) {
    if BOLD_FONT_ACTIVE.load(Ordering::Acquire) {
        return;
    }

    if BOLD_FONT_REGISTERED.load(Ordering::Acquire) {
        BOLD_FONT_ACTIVE.store(true, Ordering::Release);
        return;
    }

    if register_bold_font(ctx) {
        BOLD_FONT_REGISTERED.store(true, Ordering::Release);
        ctx.request_repaint();
    }
}

fn register_bold_font(ctx: &Context) -> bool {
    let font_bytes = match std::fs::read(BOLD_FONT_PATH) {
        Ok(bytes) => bytes,
        Err(err) => {
            warn!("egui overlay: failed to read bold UI font {BOLD_FONT_PATH}: {err}");
            return false;
        }
    };

    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert(BOLD_FONT_NAME.to_string(), Arc::new(FontData::from_owned(font_bytes)));
    fonts.families.insert(FontFamily::Name(BOLD_FONT_FAMILY.into()), vec![BOLD_FONT_NAME.to_string()]);

    ctx.set_fonts(fonts);
    info!("egui overlay: registered bold UI font from {BOLD_FONT_PATH}");
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image(width: usize, height: usize, pixels: Vec<Color32>) -> ColorImage {
        ColorImage::new([width, height], pixels)
    }

    #[test]
    fn hit_mask_blocks_opaque_pixels() {
        let mask = HitMask::from_image(&image(2, 1, vec![Color32::TRANSPARENT, Color32::WHITE]));

        assert!(!mask.blocks_uv(0.25, 0.5));
        assert!(mask.blocks_uv(0.75, 0.5));
    }

    #[test]
    fn hit_region_rejects_out_of_bounds_positions() {
        let region = HitRegion {
            rect: Rect::from_min_size(pos2(10.0, 20.0), vec2(5.0, 5.0)),
            uv: unit_uv(),
            mask: Arc::new(HitMask::from_image(&image(1, 1, vec![Color32::WHITE]))),
        };

        assert!(!region.blocks(pos2(9.0, 22.0)));
        assert!(!region.blocks(pos2(12.0, 26.0)));
    }

    #[test]
    fn hit_region_maps_cropped_uv_to_source_pixels() {
        let region = HitRegion {
            rect: Rect::from_min_size(pos2(0.0, 0.0), vec2(20.0, 10.0)),
            uv: Rect::from_min_max(pos2(0.5, 0.0), pos2(1.0, 1.0)),
            mask: Arc::new(HitMask::from_image(&image(
                4,
                1,
                vec![Color32::TRANSPARENT, Color32::TRANSPARENT, Color32::WHITE, Color32::TRANSPARENT],
            ))),
        };

        assert!(region.blocks(pos2(1.0, 5.0)));
        assert!(!region.blocks(pos2(19.0, 5.0)));
    }
}
