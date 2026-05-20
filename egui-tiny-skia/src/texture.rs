use std::collections::HashMap;
use std::fmt;

use egui::{Color32, ImageData, TextureId, TexturesDelta};

#[derive(Debug, Default)]
pub struct TextureStore {
    textures: HashMap<TextureId, CpuTexture>,
}

#[derive(Debug, Clone)]
pub struct CpuTexture {
    width: usize,
    height: usize,
    pixels: Vec<Color32>,
    alpha_profile: AlphaProfile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlphaKind {
    FullyOpaque,
    BinaryAlpha,
    MixedAlpha,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AlphaProfile {
    kind: AlphaKind,
    transparent_count: usize,
    opaque_count: usize,
    mixed_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextureStoreError {
    InvalidPixelCount {
        width: usize,
        height: usize,
        pixels: usize,
    },
    MissingTexture(TextureId),
    PartialUpdateOutOfBounds {
        texture: TextureId,
        x: usize,
        y: usize,
        width: usize,
        height: usize,
        texture_width: usize,
        texture_height: usize,
    },
}

impl fmt::Display for TextureStoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPixelCount { width, height, pixels } => write!(f, "texture has dimensions {width}x{height}, but {pixels} pixels"),
            Self::MissingTexture(texture_id) => {
                write!(f, "partial update for missing texture {texture_id:?}")
            }
            Self::PartialUpdateOutOfBounds {
                texture,
                x,
                y,
                width,
                height,
                texture_width,
                texture_height,
            } => write!(
                f,
                "partial update for {texture:?} at {x},{y} with size {width}x{height} exceeds texture size {texture_width}x{texture_height}"
            ),
        }
    }
}

impl std::error::Error for TextureStoreError {}

impl TextureStore {
    pub fn apply_delta(&mut self, delta: TexturesDelta) -> Result<(), TextureStoreError> {
        for (texture_id, image_delta) in delta.set {
            let image = CpuTexture::from_image_data(&image_delta.image)?;

            if let Some([x, y]) = image_delta.pos {
                let texture = self.textures.get_mut(&texture_id).ok_or(TextureStoreError::MissingTexture(texture_id))?;
                texture.update_region(texture_id, x, y, &image)?;
            } else {
                self.textures.insert(texture_id, image);
            }
        }

        for texture_id in delta.free {
            self.textures.remove(&texture_id);
        }

        Ok(())
    }

    pub fn get(&self, texture_id: TextureId) -> Option<&CpuTexture> {
        self.textures.get(&texture_id)
    }
}

impl CpuTexture {
    fn from_image_data(image_data: &ImageData) -> Result<Self, TextureStoreError> {
        match image_data {
            ImageData::Color(image) => {
                let [width, height] = image.size;
                Self::new(width, height, image.pixels.clone())
            }
        }
    }

    fn new(width: usize, height: usize, pixels: Vec<Color32>) -> Result<Self, TextureStoreError> {
        if pixels.len() != width * height {
            return Err(TextureStoreError::InvalidPixelCount {
                width,
                height,
                pixels: pixels.len(),
            });
        }

        let alpha_profile = classify_alpha(&pixels);
        Ok(Self { width, height, pixels, alpha_profile })
    }

    fn update_region(&mut self, texture_id: TextureId, x: usize, y: usize, update: &CpuTexture) -> Result<(), TextureStoreError> {
        if x + update.width > self.width || y + update.height > self.height {
            return Err(TextureStoreError::PartialUpdateOutOfBounds {
                texture: texture_id,
                x,
                y,
                width: update.width,
                height: update.height,
                texture_width: self.width,
                texture_height: self.height,
            });
        }

        for row in 0..update.height {
            let destination_start = (y + row) * self.width + x;
            let source_start = row * update.width;
            let destination = &mut self.pixels[destination_start..destination_start + update.width];
            let source = &update.pixels[source_start..source_start + update.width];
            destination.copy_from_slice(source);
        }

        self.alpha_profile = classify_alpha(&self.pixels);
        Ok(())
    }

    pub fn alpha_kind(&self) -> AlphaKind {
        self.alpha_profile.kind
    }

    pub fn alpha_profile(&self) -> AlphaProfile {
        self.alpha_profile
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.height
    }

    pub fn sample_pixel(&self, x: usize, y: usize) -> Color32 {
        self.pixels[y.min(self.height.saturating_sub(1)) * self.width + x.min(self.width.saturating_sub(1))]
    }

    pub fn sample(&self, u: f32, v: f32) -> Color32 {
        let x = (u.clamp(0.0, 1.0) * (self.width.saturating_sub(1) as f32)).round() as usize;
        let y = (v.clamp(0.0, 1.0) * (self.height.saturating_sub(1) as f32)).round() as usize;
        self.sample_pixel(x, y)
    }
}

fn classify_alpha(pixels: &[Color32]) -> AlphaProfile {
    let mut transparent_count = 0;
    let mut opaque_count = 0;
    let mut mixed_count = 0;

    for pixel in pixels {
        match pixel.a() {
            0 => transparent_count += 1,
            255 => opaque_count += 1,
            _ => mixed_count += 1,
        }
    }

    let kind = if mixed_count > 0 {
        AlphaKind::MixedAlpha
    } else if transparent_count > 0 {
        AlphaKind::BinaryAlpha
    } else {
        AlphaKind::FullyOpaque
    };

    AlphaProfile {
        kind,
        transparent_count,
        opaque_count,
        mixed_count,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn color_image(pixels: Vec<Color32>) -> egui::ImageData {
        egui::ImageData::Color(egui::ColorImage::new([pixels.len(), 1], pixels).into())
    }

    #[test]
    fn classifies_fully_opaque_textures() {
        let texture = CpuTexture::from_image_data(&color_image(vec![Color32::RED, Color32::GREEN])).unwrap();

        assert_eq!(texture.alpha_kind(), AlphaKind::FullyOpaque);
    }

    #[test]
    fn classifies_binary_alpha_textures() {
        let texture = CpuTexture::from_image_data(&color_image(vec![Color32::TRANSPARENT, Color32::RED])).unwrap();

        assert_eq!(texture.alpha_kind(), AlphaKind::BinaryAlpha);
    }

    #[test]
    fn classifies_mixed_alpha_textures() {
        let texture = CpuTexture::from_image_data(&color_image(vec![Color32::from_rgba_unmultiplied(10, 20, 30, 128)])).unwrap();

        assert_eq!(texture.alpha_kind(), AlphaKind::MixedAlpha);
    }

    #[test]
    fn partial_update_recomputes_alpha_classification() {
        let mut texture = CpuTexture::from_image_data(&color_image(vec![Color32::RED, Color32::GREEN])).unwrap();
        let update = CpuTexture::from_image_data(&color_image(vec![Color32::TRANSPARENT])).unwrap();

        texture.update_region(TextureId::Managed(0), 1, 0, &update).unwrap();

        assert_eq!(texture.alpha_kind(), AlphaKind::BinaryAlpha);
    }
}
