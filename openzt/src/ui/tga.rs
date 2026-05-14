use anyhow::{Context, anyhow, bail, ensure};
use egui::{Color32, ColorImage};

#[derive(Debug, Clone, Copy)]
struct Header {
    id_length: usize,
    color_map_type: u8,
    image_type: u8,
    color_map_first_entry: u16,
    color_map_length: usize,
    color_map_entry_depth: u8,
    width: usize,
    height: usize,
    pixel_depth: u8,
    image_descriptor: u8,
}

pub fn decode_tga(data: &[u8]) -> anyhow::Result<ColorImage> {
    ensure!(data.len() >= 18, "TGA data is too short");

    let header = Header {
        id_length: data[0] as usize,
        color_map_type: data[1],
        image_type: data[2],
        color_map_first_entry: u16::from_le_bytes([data[3], data[4]]),
        color_map_length: u16::from_le_bytes([data[5], data[6]]) as usize,
        color_map_entry_depth: data[7],
        width: u16::from_le_bytes([data[12], data[13]]) as usize,
        height: u16::from_le_bytes([data[14], data[15]]) as usize,
        pixel_depth: data[16],
        image_descriptor: data[17],
    };

    ensure!(header.width > 0 && header.height > 0, "TGA dimensions must be non-zero");

    let mut index = 18usize.checked_add(header.id_length).ok_or_else(|| anyhow!("TGA header offset overflow"))?;
    ensure!(index <= data.len(), "TGA image ID exceeds data length");

    let color_map = read_color_map(data, &mut index, header)?;
    let pixel_count = header.width * header.height;
    let mut source_pixels = Vec::with_capacity(pixel_count);

    match header.image_type {
        1 | 2 | 3 => {
            for _ in 0..pixel_count {
                source_pixels.push(read_pixel(data, &mut index, header, color_map.as_deref())?);
            }
        }
        9..=11 => {
            while source_pixels.len() < pixel_count {
                let packet = read_u8(data, &mut index).context("TGA RLE packet is truncated")?;
                let count = (packet & 0x7f) as usize + 1;
                ensure!(source_pixels.len() + count <= pixel_count, "TGA RLE packet exceeds image bounds");

                if packet & 0x80 != 0 {
                    let pixel = read_pixel(data, &mut index, header, color_map.as_deref())?;
                    source_pixels.extend(std::iter::repeat(pixel).take(count));
                } else {
                    for _ in 0..count {
                        source_pixels.push(read_pixel(data, &mut index, header, color_map.as_deref())?);
                    }
                }
            }
        }
        _ => bail!("unsupported TGA image type {}", header.image_type),
    }

    let mut pixels = vec![Color32::TRANSPARENT; pixel_count];
    let top_origin = header.image_descriptor & 0x20 != 0;
    let right_origin = header.image_descriptor & 0x10 != 0;

    for y in 0..header.height {
        for x in 0..header.width {
            let source_index = y * header.width + x;
            let dest_x = if right_origin { header.width - 1 - x } else { x };
            let dest_y = if top_origin { y } else { header.height - 1 - y };
            pixels[dest_y * header.width + dest_x] = source_pixels[source_index];
        }
    }

    Ok(ColorImage::new([header.width, header.height], pixels))
}

fn read_color_map(data: &[u8], index: &mut usize, header: Header) -> anyhow::Result<Option<Vec<Color32>>> {
    if header.color_map_type == 0 {
        if matches!(header.image_type, 1 | 9) {
            bail!("TGA uses indexed pixels but has no embedded color map");
        }
        return Ok(None);
    }

    ensure!(header.color_map_type == 1, "unsupported TGA color map type {}", header.color_map_type);
    ensure!(matches!(header.image_type, 1 | 9), "TGA includes an unused color map");

    let bytes_per_entry = bytes_per_pixel(header.color_map_entry_depth).context("unsupported TGA color map entry depth")?;
    let total_bytes = header
        .color_map_length
        .checked_mul(bytes_per_entry)
        .ok_or_else(|| anyhow!("TGA color map size overflow"))?;
    ensure!(*index + total_bytes <= data.len(), "TGA color map is truncated");

    let mut colors = Vec::with_capacity(header.color_map_length);
    for _ in 0..header.color_map_length {
        colors.push(read_color(data, index, header.color_map_entry_depth)?);
    }

    Ok(Some(colors))
}

fn read_pixel(data: &[u8], index: &mut usize, header: Header, color_map: Option<&[Color32]>) -> anyhow::Result<Color32> {
    match header.image_type {
        1 | 9 => {
            let raw_index = match header.pixel_depth {
                8 => read_u8(data, index)? as u16,
                16 => read_u16(data, index)?,
                _ => bail!("unsupported indexed TGA pixel depth {}", header.pixel_depth),
            };
            let palette_index = raw_index
                .checked_sub(header.color_map_first_entry)
                .ok_or_else(|| anyhow!("TGA palette index {} is before first entry {}", raw_index, header.color_map_first_entry))?
                as usize;
            color_map
                .and_then(|colors| colors.get(palette_index).copied())
                .ok_or_else(|| anyhow!("TGA palette index {} is out of bounds", raw_index))
        }
        2 | 10 => read_color(data, index, header.pixel_depth),
        3 | 11 => match header.pixel_depth {
            8 => {
                let value = read_u8(data, index)?;
                Ok(Color32::from_gray(value))
            }
            _ => bail!("unsupported grayscale TGA pixel depth {}", header.pixel_depth),
        },
        _ => bail!("unsupported TGA image type {}", header.image_type),
    }
}

fn read_color(data: &[u8], index: &mut usize, pixel_depth: u8) -> anyhow::Result<Color32> {
    match pixel_depth {
        15 | 16 => {
            let value = read_u16(data, index)?;
            let b = ((value & 0x1f) as u8) << 3;
            let g = (((value >> 5) & 0x1f) as u8) << 3;
            let r = (((value >> 10) & 0x1f) as u8) << 3;
            let a = if pixel_depth == 16 && value & 0x8000 == 0 { 0 } else { 255 };
            Ok(Color32::from_rgba_unmultiplied(r, g, b, a))
        }
        24 => {
            let b = read_u8(data, index)?;
            let g = read_u8(data, index)?;
            let r = read_u8(data, index)?;
            Ok(Color32::from_rgb(r, g, b))
        }
        32 => {
            let b = read_u8(data, index)?;
            let g = read_u8(data, index)?;
            let r = read_u8(data, index)?;
            let a = read_u8(data, index)?;
            Ok(Color32::from_rgba_unmultiplied(r, g, b, a))
        }
        _ => bail!("unsupported TGA color depth {}", pixel_depth),
    }
}

fn bytes_per_pixel(pixel_depth: u8) -> anyhow::Result<usize> {
    match pixel_depth {
        8 => Ok(1),
        15 | 16 => Ok(2),
        24 => Ok(3),
        32 => Ok(4),
        _ => bail!("unsupported pixel depth {}", pixel_depth),
    }
}

fn read_u8(data: &[u8], index: &mut usize) -> anyhow::Result<u8> {
    let value = *data.get(*index).ok_or_else(|| anyhow!("unexpected end of TGA data"))?;
    *index += 1;
    Ok(value)
}

fn read_u16(data: &[u8], index: &mut usize) -> anyhow::Result<u16> {
    let lo = read_u8(data, index)?;
    let hi = read_u8(data, index)?;
    Ok(u16::from_le_bytes([lo, hi]))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header(image_type: u8, width: u16, height: u16, pixel_depth: u8, descriptor: u8) -> Vec<u8> {
        let mut data = vec![0; 18];
        data[2] = image_type;
        data[12..14].copy_from_slice(&width.to_le_bytes());
        data[14..16].copy_from_slice(&height.to_le_bytes());
        data[16] = pixel_depth;
        data[17] = descriptor;
        data
    }

    #[test]
    fn decodes_uncompressed_24_bit_true_color() {
        let mut data = header(2, 2, 2, 24, 0x20);
        data.extend_from_slice(&[0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255]);

        let image = decode_tga(&data).unwrap();

        assert_eq!(image.size, [2, 2]);
        assert_eq!(image.pixels[0], Color32::RED);
        assert_eq!(image.pixels[1], Color32::GREEN);
        assert_eq!(image.pixels[2], Color32::BLUE);
        assert_eq!(image.pixels[3], Color32::WHITE);
    }

    #[test]
    fn decodes_uncompressed_32_bit_true_color_with_alpha() {
        let mut data = header(2, 1, 1, 32, 0x20);
        data.extend_from_slice(&[20, 30, 40, 128]);

        let image = decode_tga(&data).unwrap();

        assert_eq!(image.pixels[0], Color32::from_rgba_unmultiplied(40, 30, 20, 128));
    }

    #[test]
    fn flips_bottom_left_origin_to_egui_top_left() {
        let mut data = header(2, 1, 2, 24, 0);
        data.extend_from_slice(&[255, 0, 0, 0, 0, 255]);

        let image = decode_tga(&data).unwrap();

        assert_eq!(image.pixels[0], Color32::RED);
        assert_eq!(image.pixels[1], Color32::BLUE);
    }

    #[test]
    fn rejects_unsupported_image_type() {
        let data = header(0, 1, 1, 24, 0);

        assert!(decode_tga(&data).is_err());
    }
}
