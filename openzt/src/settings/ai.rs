use super::util::{Address, GettableSettable, Setting};

const ZTAIMGR_ADDRESS_PTR: u32 = 0x00638098;

const SHOW_BUILDING_AI_INFO: Setting<bool> = Setting {
    header: "AI",
    key: "ShowBuildingAIInfo",
    address: Address::Global(0x00638fc8),
    default: false,
};
const SHOW_AI_INFO: Setting<bool> = Setting {
    header: "AI",
    key: "ShowAIInfo",
    address: Address::Indirect(ZTAIMGR_ADDRESS_PTR, 0xf4),
    default: false,
};
const SHOW_NAME: Setting<bool> = Setting {
    header: "AI",
    key: "ShowName",
    address: Address::Indirect(ZTAIMGR_ADDRESS_PTR, 0xf8),
    default: false,
};
const SHOW_POSITION: Setting<bool> = Setting {
    header: "AI",
    key: "ShowPosition",
    address: Address::Indirect(ZTAIMGR_ADDRESS_PTR, 0xfc),
    default: false,
};
const SHOW_STATUS_VARS: Setting<bool> = Setting {
    header: "AI",
    key: "ShowStatusVars",
    address: Address::Indirect(ZTAIMGR_ADDRESS_PTR, 0x100),
    default: false,
};
const SHOW_FUNCTION_CALL: Setting<bool> = Setting {
    header: "AI",
    key: "ShowFunctionCall",
    address: Address::Indirect(ZTAIMGR_ADDRESS_PTR, 0x108),
    default: false,
};
const SHOW_EVENTS: Setting<bool> = Setting {
    header: "AI",
    key: "ShowEvents",
    address: Address::Indirect(ZTAIMGR_ADDRESS_PTR, 0x10c),
    default: false,
};
const SHOW_SELECTED: Setting<bool> = Setting {
    header: "AI",
    key: "ShowSelected",
    address: Address::Indirect(ZTAIMGR_ADDRESS_PTR, 0x104),
    default: false,
};
const SHOW_FRAME: Setting<bool> = Setting {
    header: "AI",
    key: "ShowFrame",
    address: Address::Indirect(ZTAIMGR_ADDRESS_PTR, 0x114),
    default: false,
};
const SHOW_GOAL: Setting<bool> = Setting {
    header: "AI",
    key: "ShowGoal",
    address: Address::Indirect(ZTAIMGR_ADDRESS_PTR, 0x118),
    default: false,
};
const AI_INFO_NTH: Setting<bool> = Setting {
    header: "AI",
    key: "AIInfoNth",
    address: Address::Indirect(ZTAIMGR_ADDRESS_PTR, 0x110),
    default: false,
};

pub fn get_settings() -> Vec<Box<dyn GettableSettable>> {
    vec![
        Box::new(SHOW_BUILDING_AI_INFO),
        Box::new(SHOW_AI_INFO),
        Box::new(SHOW_NAME),
        Box::new(SHOW_POSITION),
        Box::new(SHOW_STATUS_VARS),
        Box::new(SHOW_FUNCTION_CALL),
        Box::new(SHOW_EVENTS),
        Box::new(SHOW_SELECTED),
        Box::new(SHOW_FRAME),
        Box::new(SHOW_GOAL),
        Box::new(AI_INFO_NTH),
    ]
}
