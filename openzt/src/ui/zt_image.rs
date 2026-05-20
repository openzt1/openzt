use anyhow::{Context, anyhow, ensure};
use egui::{Color32, ColorImage};

use crate::animation::{Animation, Frame};

pub fn decode_palette(data: &[u8]) -> anyhow::Result<Vec<Color32>> {
    ensure!(data.len() >= 4, "palette data is too short");

    let count = u32::from_le_bytes(data[0..4].try_into().expect("slice length checked")) as usize;
    let expected_len = 4usize
        .checked_add(count.checked_mul(4).ok_or_else(|| anyhow!("palette size overflow"))?)
        .ok_or_else(|| anyhow!("palette size overflow"))?;
    ensure!(data.len() >= expected_len, "palette data is truncated");

    let mut colors = Vec::with_capacity(count);
    for chunk in data[4..expected_len].chunks_exact(4) {
        colors.push(Color32::from_rgba_unmultiplied(chunk[0], chunk[1], chunk[2], chunk[3]));
    }

    Ok(colors)
}

pub fn decode_animation_frames(animation_data: &[u8], palette_data: &[u8]) -> anyhow::Result<(Animation, Vec<ColorImage>)> {
    let animation = Animation::parse(animation_data).context("failed to parse Zoo Tycoon animation")?;
    let palette = decode_palette(palette_data).context("failed to parse Zoo Tycoon palette")?;
    ensure!(!animation.frames.is_empty(), "animation contains no frames");
    ensure!(!palette.is_empty(), "palette contains no colors");

    let mut frames = Vec::with_capacity(animation.frames.len());
    for frame in &animation.frames {
        frames.push(frame_to_color_image(frame, &palette)?);
    }

    Ok((animation, frames))
}

pub fn frame_to_color_image(frame: &Frame, palette: &[Color32]) -> anyhow::Result<ColorImage> {
    let width = frame.pixel_width as usize;
    let height = frame.pixel_height as usize;
    ensure!(width > 0 && height > 0, "animation frame dimensions must be non-zero");
    ensure!(
        frame.lines.len() == height,
        "animation frame has {} lines but height is {}",
        frame.lines.len(),
        height
    );

    let mut pixels = vec![Color32::TRANSPARENT; width * height];

    for (y, line) in frame.lines.iter().enumerate() {
        let mut x = 0usize;

        for instruction in &line.draw_instructions {
            x = x
                .checked_add(instruction.offset as usize)
                .ok_or_else(|| anyhow!("animation draw instruction x offset overflow"))?;

            for palette_index in &instruction.colors {
                ensure!(x < width, "animation draw instruction exceeds frame width at line {}", y);

                let color = palette
                    .get(*palette_index as usize)
                    .copied()
                    .ok_or_else(|| anyhow!("animation references missing palette index {}", palette_index))?;
                pixels[y * width + x] = color;
                x += 1;
            }
        }
    }

    Ok(ColorImage::new([width, height], pixels))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::animation::{DrawInstruction, Frame, Line};

    fn palette_bytes(count: u32, colors: &[[u8; 4]]) -> Vec<u8> {
        let mut data = count.to_le_bytes().to_vec();
        for color in colors {
            data.extend_from_slice(color);
        }
        data
    }

    fn test_frame() -> Frame {
        Frame {
            num_bytes: 0,
            pixel_height: 1,
            pixel_width: 5,
            vertical_offset_y: 0,
            horizontal_offset_x: 0,
            mystery_u16: 0,
            lines: vec![Line {
                num_draw_instructions: 2,
                draw_instructions: vec![
                    DrawInstruction {
                        offset: 1,
                        num_colors: 2,
                        colors: vec![1, 2],
                    },
                    DrawInstruction {
                        offset: 1,
                        num_colors: 1,
                        colors: vec![3],
                    },
                ],
            }],
        }
    }

    #[test]
    fn decodes_rgba_palette() {
        let data = palette_bytes(2, &[[10, 20, 30, 40], [50, 60, 70, 80]]);

        let palette = decode_palette(&data).unwrap();

        assert_eq!(
            palette,
            vec![Color32::from_rgba_unmultiplied(10, 20, 30, 40), Color32::from_rgba_unmultiplied(50, 60, 70, 80),]
        );
    }

    #[test]
    fn rejects_truncated_palette() {
        let data = palette_bytes(2, &[[10, 20, 30, 40]]);

        assert!(decode_palette(&data).is_err());
    }

    #[test]
    fn rasterizes_offsets_and_palette_indices() {
        let palette = vec![Color32::BLACK, Color32::RED, Color32::GREEN, Color32::BLUE];

        let image = frame_to_color_image(&test_frame(), &palette).unwrap();

        assert_eq!(image.size, [5, 1]);
        assert_eq!(image.pixels[0], Color32::TRANSPARENT);
        assert_eq!(image.pixels[1], Color32::RED);
        assert_eq!(image.pixels[2], Color32::GREEN);
        assert_eq!(image.pixels[3], Color32::TRANSPARENT);
        assert_eq!(image.pixels[4], Color32::BLUE);
    }

    #[test]
    fn rejects_missing_palette_index() {
        let palette = vec![Color32::BLACK, Color32::RED];

        assert!(frame_to_color_image(&test_frame(), &palette).is_err());
    }
}
