use std::cell::{Cell, RefCell};

use crate::util::{ZTBufferString, ZTString};

#[derive(Debug, Clone, PartialEq)]
pub struct ShowScriptItem {
    pub(crate) default_available: bool,
    pub(crate) visible: bool,
    pub(crate) id: u16,
    pub(crate) item_type: u32,
    pub(crate) sentinel: u32,
    pub(crate) name: String,
    pub(crate) anim: String,
    pub(crate) keeper_pre_trick: String,
    pub(crate) keeper_post_trick: String,
    pub(crate) building: u32,
    pub(crate) complexity: u32,
    pub(crate) return_to_keeper: bool,
    pub(crate) satisfaction: u32,
    pub(crate) satisfaction_delta: u32,
    pub(crate) satisfaction_mirror: u32,
    pub(crate) minimum_depth: u32,
    pub(crate) normal_help_id: u32,
    pub(crate) grayed_help_id: u32,
    pub(crate) normal_icon: String,
    pub(crate) grayed_icon: String,
}

impl Default for ShowScriptItem {
    fn default() -> Self {
        ShowScriptItem {
            default_available: false,
            visible: true,
            id: 0,
            item_type: 0,
            sentinel: 0xffff_ffff,
            name: String::new(),
            anim: String::new(),
            keeper_pre_trick: String::new(),
            keeper_post_trick: String::new(),
            building: 0,
            complexity: 1,
            return_to_keeper: false,
            satisfaction: 1,
            satisfaction_delta: 1,
            satisfaction_mirror: 1,
            minimum_depth: 1,
            normal_help_id: 0,
            grayed_help_id: 0,
            normal_icon: String::new(),
            grayed_icon: String::new(),
        }
    }
}

#[repr(C)]
pub struct ZTShowScriptItemRaw {
    pub(crate) _vtable: u32,
    pub(crate) default_available: u8,
    pub(crate) visible: u8,
    pub(crate) id: u16,
    pub(crate) item_type: u32,
    pub(crate) sentinel: u32,
    pub(crate) name: ZTBufferString,
    pub(crate) anim: ZTBufferString,
    pub(crate) keeper_pre_trick: ZTBufferString,
    pub(crate) keeper_post_trick: ZTBufferString,
    pub(crate) building: u32,
    pub(crate) complexity: u32,
    pub(crate) return_to_keeper: u8,
    pub(crate) _pad: [u8; 3],
    pub(crate) satisfaction: u32,
    pub(crate) satisfaction_delta: u32,
    pub(crate) satisfaction_mirror: u32,
    pub(crate) minimum_depth: u32,
    pub(crate) normal_help_id: u32,
    pub(crate) grayed_help_id: u32,
    pub(crate) normal_icon: ZTBufferString,
    pub(crate) grayed_icon: ZTBufferString,
}

const _: () = assert!(std::mem::size_of::<ZTShowScriptItemRaw>() == 0x7c);

impl ZTShowScriptItemRaw {
    pub(crate) fn to_owned_item(&self) -> ShowScriptItem {
        ShowScriptItem {
            default_available: self.default_available != 0,
            visible: self.visible != 0,
            id: self.id,
            item_type: self.item_type,
            sentinel: self.sentinel,
            name: self.name.copy_to_string(),
            anim: self.anim.copy_to_string(),
            keeper_pre_trick: self.keeper_pre_trick.copy_to_string(),
            keeper_post_trick: self.keeper_post_trick.copy_to_string(),
            building: self.building,
            complexity: self.complexity,
            return_to_keeper: self.return_to_keeper != 0,
            satisfaction: self.satisfaction,
            satisfaction_delta: self.satisfaction_delta,
            satisfaction_mirror: self.satisfaction_mirror,
            minimum_depth: self.minimum_depth,
            normal_help_id: self.normal_help_id,
            grayed_help_id: self.grayed_help_id,
            normal_icon: self.normal_icon.copy_to_string(),
            grayed_icon: self.grayed_icon.copy_to_string(),
        }
    }

    pub(crate) fn from_owned(item: &ShowScriptItem) -> Self {
        let empty = ZTBufferString::from_raw_parts(0, 0, 0);
        ZTShowScriptItemRaw {
            _vtable: 0,
            default_available: item.default_available as u8,
            visible: item.visible as u8,
            id: item.id,
            item_type: item.item_type,
            sentinel: item.sentinel,
            name: empty.clone(),
            anim: empty.clone(),
            keeper_pre_trick: empty.clone(),
            keeper_post_trick: empty.clone(),
            building: item.building,
            complexity: item.complexity,
            return_to_keeper: item.return_to_keeper as u8,
            _pad: [0; 3],
            satisfaction: item.satisfaction,
            satisfaction_delta: item.satisfaction_delta,
            satisfaction_mirror: item.satisfaction_mirror,
            minimum_depth: item.minimum_depth,
            normal_help_id: item.normal_help_id,
            grayed_help_id: item.grayed_help_id,
            normal_icon: empty.clone(),
            grayed_icon: empty,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ItemSnapshot {
    pub id: u16,
    pub item_type: u32,
    pub satisfaction: u32,
    pub satisfaction_mirror: u32,
}

pub(crate) const ITEM_BUFFER_POOL_SIZE: usize = 4;

pub(crate) struct ItemBufferPool {
    slots: RefCell<Vec<*mut ZTShowScriptItemRaw>>,
    cursor: Cell<usize>,
}

impl ItemBufferPool {
    pub(crate) const fn new() -> Self {
        ItemBufferPool { slots: RefCell::new(Vec::new()), cursor: Cell::new(0) }
    }

    pub(crate) fn write(&self, item: &ShowScriptItem) -> u32 {
        let mut slots = self.slots.borrow_mut();
        if slots.is_empty() {
            slots.extend((0..ITEM_BUFFER_POOL_SIZE).map(|_| Box::leak(Box::new(ZTShowScriptItemRaw::from_owned(item))) as *mut ZTShowScriptItemRaw));
        }
        let i = self.cursor.get();
        self.cursor.set((i + 1) % ITEM_BUFFER_POOL_SIZE);
        unsafe { *slots[i] = ZTShowScriptItemRaw::from_owned(item) };
        slots[i] as u32
    }
}

thread_local! {
    pub(crate) static GET_ITEM_POOL: ItemBufferPool = const { ItemBufferPool::new() };
    pub(crate) static GET_ITEM_BY_TRICK_ID_POOL: ItemBufferPool = const { ItemBufferPool::new() };
}
