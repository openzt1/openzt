// This is an example of how the macro transforms the code

// INPUT:
/*
use openzt_detour_macro::{detour_mod, detour};

#[detour_mod]
mod detour_mod {
    #[detour(LOAD_LANG_DLLS)]
    unsafe extern "thiscall" fn detour_target(_: u32) -> u32 {
        info!("Detour success");
        1
    }
}
*/

// OUTPUT (what the macro generates):
mod detour_mod {
    static LOAD_LANG_DLLS_DETOUR: ::std::sync::LazyLock<::retour::GenericDetour<unsafe extern "thiscall" fn(u32) -> u32>> =
        ::std::sync::LazyLock::new(|| unsafe { LOAD_LANG_DLLS.detour(detour_target).unwrap() });

    unsafe extern "thiscall" fn detour_target(_: u32) -> u32 {
        info!("Detour success");
        1
    }

    pub fn init() {
        LOAD_LANG_DLLS_DETOUR.enable().unwrap();
    }
}

fn main() {
    println!("This example shows the macro transformation");
}
