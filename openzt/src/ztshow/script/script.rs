use openzt_detour::generated::standalone::DEALLOCATE;

use crate::encoding_utils::{decode_game_text, encode_to_ansi};
#[cfg(test)]
use crate::util::ZTBufferString;
use super::item::ShowScriptItem;
#[cfg(test)]
use super::item::ZTShowScriptItemRaw;

pub(crate) const SYNTHETIC_SCRIPT_HANDLE_BASE: u32 = 0x7300_0000;

pub(crate) fn synthetic_script_handle(id: u16) -> u32 {
    SYNTHETIC_SCRIPT_HANDLE_BASE | id as u32
}

#[derive(Debug, Clone)]
pub(crate) struct ShowScriptData {
    pub(crate) sentinel: u32,
    pub(crate) script_type: u32,
    pub(crate) items: Vec<ShowScriptItem>,
}

#[cfg(test)]
pub(crate) fn test_item(item_type: u32, trick_id: u16) -> ZTShowScriptItemRaw {
    let empty = ZTBufferString::from_raw_parts(0, 0, 0);
    ZTShowScriptItemRaw {
        _vtable: 0,
        default_available: 0,
        visible: 1,
        id: trick_id,
        item_type,
        sentinel: 0xffff_ffff,
        name: empty.clone(),
        anim: empty.clone(),
        keeper_pre_trick: empty.clone(),
        keeper_post_trick: empty.clone(),
        building: 0,
        complexity: 1,
        return_to_keeper: 0,
        _pad: [0; 3],
        satisfaction: 1,
        satisfaction_delta: 1,
        satisfaction_mirror: 1,
        minimum_depth: 1,
        normal_help_id: 0,
        grayed_help_id: 0,
        normal_icon: empty.clone(),
        grayed_icon: empty,
    }
}

pub(crate) fn next_script_id(counter: &mut u16) -> u16 {
    *counter = counter.wrapping_add(1);
    *counter % 0xffff
}

pub(crate) const STRING_LENGTH_CAP: u32 = 0x1000;
pub(crate) const MAX_SCRIPT_ITEM_COUNT: u32 = 0x1000;
pub(crate) const MAX_SCRIPT_COUNT: u32 = 0x1_0000;

pub(crate) fn write_string(buf: &mut Vec<u8>, s: &str) {
    let encoded = encode_to_ansi(s);
    let len = (encoded.len() as u32).min(STRING_LENGTH_CAP - 1);
    buf.extend_from_slice(&len.to_le_bytes());
    buf.extend_from_slice(&encoded[..len as usize]);
}

pub(crate) fn encode_item(item: &ShowScriptItem) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.push(item.default_available as u8);
    buf.push(item.visible as u8);
    buf.extend_from_slice(&item.id.to_le_bytes());
    buf.extend_from_slice(&item.item_type.to_le_bytes());
    buf.extend_from_slice(&item.sentinel.to_le_bytes());
    write_string(&mut buf, &item.name);
    write_string(&mut buf, &item.anim);
    write_string(&mut buf, &item.keeper_pre_trick);
    write_string(&mut buf, &item.keeper_post_trick);
    buf.extend_from_slice(&item.building.to_le_bytes());
    buf.extend_from_slice(&item.complexity.to_le_bytes());
    buf.push(item.return_to_keeper as u8);
    buf.extend_from_slice(&item.satisfaction.to_le_bytes());
    buf.extend_from_slice(&item.satisfaction_delta.to_le_bytes());
    buf.extend_from_slice(&item.satisfaction_mirror.to_le_bytes());
    buf.extend_from_slice(&item.minimum_depth.to_le_bytes());
    buf.extend_from_slice(&item.normal_help_id.to_le_bytes());
    buf.extend_from_slice(&item.grayed_help_id.to_le_bytes());
    write_string(&mut buf, &item.normal_icon);
    write_string(&mut buf, &item.grayed_icon);
    buf
}

pub(crate) fn encode_script(id: u16, script: &ShowScriptData) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(&id.to_le_bytes());
    buf.extend_from_slice(&script.sentinel.to_le_bytes());
    buf.extend_from_slice(&script.script_type.to_le_bytes());
    buf.extend_from_slice(&(script.items.len() as u32).to_le_bytes());
    for item in &script.items {
        buf.extend_from_slice(&encode_item(item));
    }
    buf
}

pub(crate) fn read_bytes(file: *const u32, buf: &mut [u8]) -> bool {
    unsafe { DEALLOCATE.hooked()(buf.as_mut_ptr() as *const u32, buf.len() as u32, 1, file as *const u8) == 1 }
}

pub(crate) fn read_u8(file: *const u32) -> Option<u8> {
    let mut b = [0u8; 1];
    read_bytes(file, &mut b).then_some(b[0])
}

pub(crate) fn read_u16(file: *const u32) -> Option<u16> {
    let mut b = [0u8; 2];
    read_bytes(file, &mut b).then(|| u16::from_le_bytes(b))
}

pub(crate) fn read_u32(file: *const u32) -> Option<u32> {
    let mut b = [0u8; 4];
    read_bytes(file, &mut b).then(|| u32::from_le_bytes(b))
}

pub(crate) fn read_string(file: *const u32) -> Option<String> {
    let len = read_u32(file)?;
    if len >= STRING_LENGTH_CAP {
        return None;
    }
    if len == 0 {
        return Some(String::new());
    }
    let mut buf = vec![0u8; len as usize];
    read_bytes(file, &mut buf).then(|| decode_game_text(&buf))
}

pub(crate) fn read_item(file: *const u32, version: u32) -> Option<ShowScriptItem> {
    let mut item = ShowScriptItem::default();
    if version > 0x58 {
        item.default_available = read_u8(file)? != 0;
        item.visible = read_u8(file)? != 0;
        item.id = read_u16(file)?;
        item.item_type = read_u32(file)?;
        item.sentinel = read_u32(file)?;
        item.name = read_string(file)?;
        item.anim = read_string(file)?;
        item.keeper_pre_trick = read_string(file)?;
        item.keeper_post_trick = read_string(file)?;
        item.building = read_u32(file)?;
        item.complexity = read_u32(file)?;
        item.return_to_keeper = read_u8(file)? != 0;
        item.satisfaction = read_u32(file)?;
        item.satisfaction_delta = read_u32(file)?;
        item.satisfaction_mirror = read_u32(file)?;
        item.minimum_depth = read_u32(file)?;
    }
    if version > 0x66 {
        item.normal_help_id = read_u32(file)?;
        item.grayed_help_id = read_u32(file)?;
        item.normal_icon = read_string(file)?;
        item.grayed_icon = read_string(file)?;
    }
    Some(item)
}

pub(crate) fn read_script(file: *const u32, version: u32) -> Option<(u16, ShowScriptData)> {
    if version <= 0x58 {
        return Some((0, ShowScriptData { sentinel: 0xffff_ffff, script_type: 0, items: Vec::new() }));
    }
    let id = read_u16(file)?;
    let sentinel = read_u32(file)?;
    let script_type = read_u32(file)?;
    let count = read_u32(file)?;
    if count > MAX_SCRIPT_ITEM_COUNT {
        return None;
    }
    let mut items = Vec::with_capacity(count as usize);
    for _ in 0..count {
        items.push(read_item(file, version)?);
    }
    Some((id, ShowScriptData { sentinel, script_type, items }))
}
