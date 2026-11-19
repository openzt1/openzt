//! Shared fixtures: fake entity types and vtables, the virtual-call log, fake `BFFunctionCall` objects.

use super::*;

pub(super) const RVA_TAG_ZTGUEST: u32 = 0x0023_8700;
pub(super) const RVA_TAG_ZTSTAFF: u32 = 0x0023_8710;
pub(super) const RVA_TAG_ZTUNITTYPE: u32 = 0x0023_86b0;
pub(super) const RVA_TAG_ZTGUIDETYPE: u32 = 0x0023_8770;

#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) enum Kind {
    Guest,
    Staff,
    Guide,
    Animal,
    Scenery,
}

pub(super) const ALL_KINDS: [Kind; 5] = [Kind::Guest, Kind::Staff, Kind::Guide, Kind::Animal, Kind::Scenery];

/// A fake `BFEntityType`: dword 0 is the vtable pointer, dword 1 the [`Kind`], and the rest is filled with
/// distinct values so a wrong offset shows up as a wrong number.
#[repr(C)]
pub(super) struct FakeType {
    pub(super) words: [u32; 0x100],
}

#[repr(C)]
pub(super) struct FakeVtable {
    pub(super) slots: [u32; 0x20],
}

pub(super) fn tag(rva: u32) -> u32 {
    get_module_base("zoo.exe") as u32 + rva
}

pub(super) extern "thiscall" fn fake_is_class(this: u32, tag_arg: u32) -> bool {
    let kind = unsafe { *((this + 4) as *const u32) };
    (tag_arg == tag(RVA_TAG_ZTGUEST) && kind == Kind::Guest as u32)
        || (tag_arg == tag(RVA_TAG_ZTSTAFF) && kind == Kind::Staff as u32)
}

/// Every fake type except scenery is a `ZTUnitType`; only the guide kind is a `ZTGuideType`.
pub(super) extern "thiscall" fn fake_is_cast_class(this: u32, tag_arg: u32) -> bool {
    let kind = unsafe { *((this + 4) as *const u32) };
    (tag_arg == tag(RVA_TAG_ZTUNITTYPE) && kind != Kind::Scenery as u32) || (tag_arg == tag(RVA_TAG_ZTGUIDETYPE) && kind == Kind::Guide as u32)
}

/// Entity-vtable calls made by the fake handlers below: each call appends its slot id as a nibble, and the
/// last `this` is kept, so a comparison sees both which virtual ran and on which object.
pub(super) static VIRTUAL_LOG: AtomicU32 = AtomicU32::new(0);
pub(super) static VIRTUAL_THIS: AtomicU32 = AtomicU32::new(0);

pub(super) fn record_virtual(id: u32, this: u32) {
    VIRTUAL_LOG.store(VIRTUAL_LOG.load(Ordering::Relaxed) * 16 + id, Ordering::Relaxed);
    VIRTUAL_THIS.store(this, Ordering::Relaxed);
}

pub(super) extern "thiscall" fn fake_enter_building(this: u32) {
    record_virtual(0xa, this);
}
pub(super) extern "thiscall" fn fake_return_to_building(this: u32) {
    record_virtual(0xb, this);
}
pub(super) extern "thiscall" fn fake_end_trick(this: u32) {
    record_virtual(0xc, this);
}
pub(super) extern "thiscall" fn fake_exit_building(this: u32) {
    record_virtual(0xd, this);
}

pub(super) fn reset_virtual_log() {
    VIRTUAL_LOG.store(0, Ordering::Relaxed);
    VIRTUAL_THIS.store(0, Ordering::Relaxed);
}

pub(super) fn virtual_log() -> (u32, u32) {
    (VIRTUAL_LOG.load(Ordering::Relaxed), VIRTUAL_THIS.load(Ordering::Relaxed))
}

pub(super) struct Fixture {
    pub(super) _entity_vtable: Box<[u32; 0x100]>,
    pub(super) ai_mgr: Box<[u32; 0x60]>,
    pub(super) _vtable: Box<FakeVtable>,
    pub(super) _types: Vec<(Kind, Box<FakeType>)>,
    pub(super) entities: Vec<(Kind, Box<[u32; 0x60]>)>,
}

impl Fixture {
    pub(super) fn new() -> Self {
        let mut ai_mgr = Box::new([0u32; 0x60]);
        for (i, word) in ai_mgr.iter_mut().enumerate() {
            *word = 0x1000 + i as u32;
        }
        let mut vtable = Box::new(FakeVtable { slots: [0; 0x20] });
        vtable.slots[0x18 / 4] = fake_is_class as *const () as usize as u32;
        vtable.slots[0x1c / 4] = fake_is_cast_class as *const () as usize as u32;
        let vtable_ptr = vtable.slots.as_ptr() as u32;

        let mut entity_vtable = Box::new([0u32; 0x100]);
        entity_vtable[0x2a8 / 4] = fake_enter_building as *const () as usize as u32;
        entity_vtable[0x2b0 / 4] = fake_return_to_building as *const () as usize as u32;
        entity_vtable[0x2bc / 4] = fake_end_trick as *const () as usize as u32;
        entity_vtable[0x2ac / 4] = fake_exit_building as *const () as usize as u32;
        let entity_vtable_ptr = entity_vtable.as_ptr() as u32;

        let mut types = Vec::new();
        let mut entities = Vec::new();
        for kind in ALL_KINDS {
            let mut ty = Box::new(FakeType { words: [0; 0x100] });
            for (i, word) in ty.words.iter_mut().enumerate() {
                *word = 0x5000 + (kind as u32) * 0x100 + i as u32;
            }
            ty.words[0] = vtable_ptr;
            ty.words[1] = kind as u32;
            let mut entity = Box::new([0u32; 0x60]);
            entity[0] = entity_vtable_ptr;
            entity[0x128 / 4] = ty.words.as_ptr() as u32;
            types.push((kind, ty));
            entities.push((kind, entity));
        }
        Self { _entity_vtable: entity_vtable, ai_mgr, _vtable: vtable, _types: types, entities }
    }

    pub(super) fn this(&self) -> *const u32 {
        self.ai_mgr.as_ptr()
    }

    pub(super) fn entity(&self, kind: Kind) -> *const u32 {
        self.entities.iter().find(|(k, _)| *k == kind).unwrap().1.as_ptr()
    }
}

/// A `{begin, end, capacity}` string backed by a NUL-terminated buffer (the numeric fallback parses by
/// pointer), keeping the buffer alive alongside it.
pub(super) struct CallString {
    pub(super) raw: [u32; 3],
    pub(super) _bytes: Vec<u8>,
}

impl CallString {
    pub(super) fn new(text: &str) -> Self {
        let mut bytes = text.as_bytes().to_vec();
        bytes.push(0);
        let begin = bytes.as_ptr() as u32;
        let end = begin + text.len() as u32;
        Self { raw: [begin, end, end + 1], _bytes: bytes }
    }
}

/// A fake `BFFunctionCall`: name string, then a parameter vector of 12-byte `{begin,end,cap}` entries.
pub(super) struct Call {
    /// Six words of name and parameter vector, then the result flag byte at `+0x18`.
    pub(super) raw: Box<[u32; 7]>,
    pub(super) _name: CallString,
    pub(super) _params: Vec<CallString>,
    pub(super) _param_words: Vec<u32>,
}

impl Call {
    pub(super) fn new(name: &str, params: &[&str]) -> Self {
        let name = CallString::new(name);
        let params: Vec<CallString> = params.iter().map(|p| CallString::new(p)).collect();
        let param_words: Vec<u32> = params.iter().flat_map(|p| p.raw).collect();
        let begin = param_words.as_ptr() as u32;
        let end = begin + (param_words.len() * 4) as u32;
        let raw = Box::new([name.raw[0], name.raw[1], name.raw[2], begin, end, end, 0]);
        Self { raw, _name: name, _params: params, _param_words: param_words }
    }

    pub(super) fn ptr(&self) -> *const u32 {
        self.raw.as_ptr()
    }

    /// The result flag byte at `+0x18`.
    pub(super) fn flag(&self) -> u8 {
        unsafe { *((self.raw.as_ptr() as u32 + 0x18) as *const u8) }
    }

    pub(super) fn raw_mut_flag(&self, value: u8) {
        unsafe { *((self.raw.as_ptr() as u32 + 0x18) as *mut u8) = value };
    }
}
