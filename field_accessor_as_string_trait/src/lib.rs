pub trait FieldAccessorAsStringTrait {
    fn set_field(&mut self, field_name: &str, value: &str) -> Result<(), String>;
    fn get_field(&self, field_name: &str) -> Result<String, String>;
    fn is_field(&self, field_name: &str) -> bool;
}
