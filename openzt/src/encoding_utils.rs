use encoding_rs::Encoding;
use std::sync::LazyLock;
use tracing::{info, warn};

/// The system's ANSI code page encoding, detected at runtime
static SYSTEM_ENCODING: LazyLock<&'static Encoding> = LazyLock::new(|| {
    #[cfg(windows)]
    {
        get_system_ansi_encoding()
    }
    #[cfg(not(windows))]
    {
        encoding_rs::WINDOWS_1252
    }
});

/// 256-entry lowercase lookup table for the system's ANSI code page
///
/// This table extends the vanilla game's 128-entry ASCII-only lowercase table
/// to support all 256 possible byte values in the system's code page.
///
/// Generated at runtime based on the detected system code page.
pub static LOWERCASE_TABLE: LazyLock<[u8; 256]> = LazyLock::new(|| generate_lowercase_table());

/// Get the memory address of the lowercase lookup table
///
/// This is used when patching the vanilla game to use our extended 256-entry
/// lowercase table instead of its built-in 128-entry table.
pub fn get_lowercase_table_ptr() -> u32 {
    LOWERCASE_TABLE.as_ptr() as u32
}

/// Get the Windows ANSI code page encoding for the current system locale
#[cfg(windows)]
fn get_system_ansi_encoding() -> &'static Encoding {
    use windows::Win32::Globalization::GetACP;

    let code_page = unsafe { GetACP() };

    info!("Detected system ANSI code page: {}", code_page);

    match code_page {
        1250 => encoding_rs::WINDOWS_1250, // Central/Eastern Europe
        1251 => encoding_rs::WINDOWS_1251, // Cyrillic
        1252 => encoding_rs::WINDOWS_1252, // Western Europe
        1253 => encoding_rs::WINDOWS_1253, // Greek
        1254 => encoding_rs::WINDOWS_1254, // Turkish
        1255 => encoding_rs::WINDOWS_1255, // Hebrew
        1256 => encoding_rs::WINDOWS_1256, // Arabic
        1257 => encoding_rs::WINDOWS_1257, // Baltic
        1258 => encoding_rs::WINDOWS_1258, // Vietnamese
        874 => encoding_rs::WINDOWS_874,   // Thai
        932 => encoding_rs::SHIFT_JIS,     // Japanese
        936 => encoding_rs::GBK,           // Simplified Chinese
        949 => encoding_rs::EUC_KR,        // Korean
        950 => encoding_rs::BIG5,          // Traditional Chinese
        _ => {
            warn!("Unknown code page {}, defaulting to Windows-1252", code_page);
            encoding_rs::WINDOWS_1252
        }
    }
}

/// Decode bytes from game files, trying UTF-8 first, then system ANSI encoding
///
/// This handles both modern UTF-8 files (mods, patches) and legacy game files
/// encoded in the system's ANSI code page (which varies by locale).
pub fn decode_game_text(bytes: &[u8]) -> String {
    // Try UTF-8 first (for modern files/mods)
    if let Ok(s) = std::str::from_utf8(bytes) {
        return s.to_string();
    }

    // Fall back to system ANSI encoding (for legacy Zoo Tycoon files)
    let (decoded, _encoding, _had_errors) = SYSTEM_ENCODING.decode(bytes);
    decoded.into_owned()
}

/// Decode with explicit encoding (for when you know the source encoding)
pub fn decode_with_encoding(bytes: &[u8], encoding: &'static Encoding) -> String {
    let (decoded, _encoding, _had_errors) = encoding.decode(bytes);
    decoded.into_owned()
}

/// Decode specifically as the system's ANSI encoding (skip UTF-8 check)
pub fn decode_ansi(bytes: &[u8]) -> String {
    let (decoded, _encoding, _had_errors) = SYSTEM_ENCODING.decode(bytes);
    decoded.into_owned()
}

/// Get the system's ANSI code page encoding
pub fn get_system_encoding() -> &'static Encoding {
    *SYSTEM_ENCODING
}

/// Encode a Unicode string to the system's ANSI code page
///
/// Characters that cannot be represented in the target encoding will be replaced
/// with '?' or the encoding's default replacement character.
pub fn encode_to_ansi(text: &str) -> Vec<u8> {
    let (encoded, _encoding, _had_errors) = SYSTEM_ENCODING.encode(text);
    encoded.into_owned()
}

/// Encode a Unicode string with explicit encoding
pub fn encode_with_encoding(text: &str, encoding: &'static Encoding) -> Vec<u8> {
    let (encoded, _encoding, _had_errors) = encoding.encode(text);
    encoded.into_owned()
}

/// Generate a 256-entry lowercase lookup table for the system's ANSI code page
///
/// This creates a byte-to-byte mapping where each index (0-255) maps to its
/// lowercase equivalent in the system's code page. This is needed to patch
/// the vanilla game's 128-entry ASCII-only lowercase table.
///
/// # Returns
/// A 256-byte array where `array[i]` is the lowercase version of byte `i`
///
/// # Example
/// ```ignore
/// let table = generate_lowercase_table();
/// // On Windows-1252 system:
/// assert_eq!(table[0xD6], 0xF6); // Ö -> ö
/// assert_eq!(table[0xC4], 0xE4); // Ä -> ä
/// ```
pub fn generate_lowercase_table() -> [u8; 256] {
    let mut table = [0u8; 256];

    for byte_value in 0u8..=255 {
        // Decode single byte to Unicode string
        let byte_slice = &[byte_value];
        let (decoded, _, _) = SYSTEM_ENCODING.decode(byte_slice);

        // Convert to lowercase
        let lowercase = decoded.to_lowercase();

        // Encode back to ANSI
        let (encoded, _, _) = SYSTEM_ENCODING.encode(&lowercase);

        // Use the first byte of the result, or keep original if encoding failed/changed length
        table[byte_value as usize] = if encoded.len() == 1 {
            encoded[0]
        } else {
            // Multi-byte result or encoding error - keep original
            byte_value
        };
    }

    table
}

/// Generate a lowercase lookup table for a specific encoding
pub fn generate_lowercase_table_for_encoding(encoding: &'static Encoding) -> [u8; 256] {
    let mut table = [0u8; 256];

    for byte_value in 0u8..=255 {
        let byte_slice = &[byte_value];
        let (decoded, _, _) = encoding.decode(byte_slice);
        let lowercase = decoded.to_lowercase();
        let (encoded, _, _) = encoding.encode(&lowercase);

        table[byte_value as usize] = if encoded.len() == 1 { encoded[0] } else { byte_value };
    }

    table
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_utf8() {
        let utf8_bytes = "Hello, 世界!".as_bytes();
        let result = decode_game_text(utf8_bytes);
        assert_eq!(result, "Hello, 世界!");
    }

    #[test]
    fn test_decode_windows_1252() {
        // Windows-1252 encoding of "Café" (C3 A9 is UTF-8, E9 is Win-1252)
        let win1252_bytes = vec![0x43, 0x61, 0x66, 0xE9]; // "Café" in Windows-1252
        let result = decode_with_encoding(&win1252_bytes, encoding_rs::WINDOWS_1252);
        assert_eq!(result, "Café");
    }

    #[test]
    fn test_german_characters() {
        // "Größe" in Windows-1252
        let bytes = vec![0x47, 0x72, 0xF6, 0xDF, 0x65];
        let result = decode_with_encoding(&bytes, encoding_rs::WINDOWS_1252);
        assert_eq!(result, "Größe");
    }

    #[test]
    #[cfg(windows)]
    fn test_system_encoding_detected() {
        let encoding = get_system_encoding();
        println!("Detected system encoding: {:?}", encoding.name());
        // Just verify it doesn't panic and returns something
        assert!(!encoding.name().is_empty());
    }

    #[test]
    fn test_lowercase_table_ascii() {
        let table = generate_lowercase_table_for_encoding(encoding_rs::WINDOWS_1252);

        // Test ASCII range (should work in all encodings)
        assert_eq!(table[b'A' as usize], b'a');
        assert_eq!(table[b'Z' as usize], b'z');
        assert_eq!(table[b'a' as usize], b'a'); // Already lowercase
        assert_eq!(table[b'0' as usize], b'0'); // Numbers unchanged
    }

    #[test]
    fn test_lowercase_table_windows_1252() {
        let table = generate_lowercase_table_for_encoding(encoding_rs::WINDOWS_1252);

        // Western European characters
        assert_eq!(table[0xC4], 0xE4); // Ä -> ä
        assert_eq!(table[0xD6], 0xF6); // Ö -> ö
        assert_eq!(table[0xDC], 0xFC); // Ü -> ü
        assert_eq!(table[0xC9], 0xE9); // É -> é
        assert_eq!(table[0xD1], 0xF1); // Ñ -> ñ

        // Already lowercase should stay the same
        assert_eq!(table[0xE4], 0xE4); // ä -> ä
        assert_eq!(table[0xDF], 0xDF); // ß -> ß (German sharp s has no uppercase)
    }

    #[test]
    fn test_lowercase_table_windows_1251() {
        let table = generate_lowercase_table_for_encoding(encoding_rs::WINDOWS_1251);

        // Cyrillic characters
        assert_eq!(table[0xC0], 0xE0); // А -> а
        assert_eq!(table[0xCF], 0xEF); // П -> п
        assert_eq!(table[0xDF], 0xFF); // Я -> я
    }

    #[test]
    fn test_lowercase_table_windows_1250() {
        let table = generate_lowercase_table_for_encoding(encoding_rs::WINDOWS_1250);

        // Central/Eastern European characters
        assert_eq!(table[0xC4], 0xE4); // Ä -> ä
        assert_eq!(table[0xA3], 0xB3); // Ł -> ł (Polish)
    }

    #[test]
    fn test_lowercase_table_control_chars() {
        let table = generate_lowercase_table_for_encoding(encoding_rs::WINDOWS_1252);

        // Control characters and special bytes should remain unchanged
        assert_eq!(table[0x00], 0x00);
        assert_eq!(table[0x0A], 0x0A); // Newline
        assert_eq!(table[0x0D], 0x0D); // Carriage return
    }
}
