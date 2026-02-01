use openzt_configparser::ini::Ini;

use crate::util::{get_from_memory, save_to_memory};

/// A setting that is stored at a specific memory address
pub struct Setting<T> {
    pub header: &'static str,
    pub key: &'static str,
    pub address: Address,
    pub default: T,
}

pub enum Address {
    Global(u32),
    Indirect(u32, u32),
}

pub trait GettableSettable {
    fn get(&self) -> String;
    fn set(&self, value: &str) -> Result<(), String>;
    fn check(&self, section: &str, key: &str) -> bool;
    fn name(&self) -> String;
}

impl<T> Setting<T> {
    fn write(&self, value: T) {
        let address = match self.address {
            Address::Global(address) => address,
            Address::Indirect(base, offset) => get_from_memory::<u32>(base) + offset,
        };
        save_to_memory::<T>(address, value);
    }

    fn read(&self) -> T {
        let address = match self.address {
            Address::Global(address) => address,
            Address::Indirect(base, offset) => get_from_memory::<u32>(base) + offset,
        };
        get_from_memory::<T>(address)
    }
}

impl Setting<bool> {
    fn load_from_ini(&self, ini: &Ini) -> bool {
        match ini.get(self.header, self.key) {
            Some(value) => match value.as_str() {
                "true" | "1" => true,
                "false" | "0" => false,
                _ => self.default,
            },
            None => self.default,
        }
    }
    fn load(&self, ini: &Ini) {
        self.write(self.load_from_ini(ini));
    }
}

impl Setting<i32> {
    fn load_from_ini(&self, ini: &Ini) -> i32 {
        value_or_default(ini.get_parse(self.header, self.key), self.default)
    }
    fn load(&self, ini: &Ini) {
        self.write(self.load_from_ini(ini));
    }
}

impl GettableSettable for Setting<bool> {
    fn get(&self) -> String {
        self.read().to_string()
    }

    fn set(&self, value: &str) -> Result<(), String> {
        self.write(match value {
            "true" | "1" => true,
            "false" | "0" => false,
            _ => return Err("Invalid value for bool setting".to_string()),
        });
        Ok(())
    }

    fn check(&self, section: &str, key: &str) -> bool {
        self.header == section && (self.key == key || key.is_empty())
    }

    fn name(&self) -> String {
        format!("{}:{}", self.header, self.key)
    }
}

impl GettableSettable for Setting<i32> {
    fn get(&self) -> String {
        self.read().to_string()
    }

    fn set(&self, value: &str) -> Result<(), String> {
        match value.to_owned().parse() {
            Ok(v) => {
                self.write(v);
                Ok(())
            }
            Err(_) => Err("Invalid value for i32 setting".to_string()),
        }
    }

    fn check(&self, section: &str, key: &str) -> bool {
        self.header == section && (self.key == key || key.is_empty())
    }

    fn name(&self) -> String {
        format!("{}:{}", self.header, self.key)
    }
}

pub fn value_or_default<T>(value: Result<Option<T>, String>, default: T) -> T {
    match value {
        Ok(Some(v)) => v,
        Ok(None) => default,
        Err(_) => default,
    }
}
