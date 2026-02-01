extern crate proc_macro;

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Fields};

#[proc_macro_derive(FieldAccessorAsString, attributes(skip_field, deref_field))]
pub fn set_fields(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let struct_name = &input.ident;
    let fields = match input.data {
        Data::Struct(ref data) => match data.fields {
            Fields::Named(ref fields) => &fields.named,
            _ => panic!("Expected a struct with named fields"),
        },
        _ => panic!("Expected a struct"),
    };

    let mut field_names = vec![];
    let mut field_types = vec![];
    let mut deref_field_exists = false;
    let mut set_error_match = quote! {Err("Unknown field name".to_string())};
    let mut get_error_match = quote! {Err("Unknown field name".to_string())};
    let mut is_error_match = quote! {false};

    for field in fields {
        if should_skip(&field.attrs) {
            continue;
        }
        if is_deref(&field.attrs) {
            assert!(!deref_field_exists, "Only one field can be marked as deref_field");
            let deref_field = field.ident.as_ref().expect("Expected field name");
            get_error_match = quote! {self.#deref_field.get_field(field_name)};
            set_error_match = quote! {self.#deref_field.set_field(field_name, value)};
            is_error_match = quote! {false || self.#deref_field.is_field(field_name)};
            deref_field_exists = true;
            continue;
        }

        let field_name = field.ident.as_ref().expect("Expected field name");
        let field_type = &field.ty;

        let syn::Type::Path(field_type_path) = &field.ty else {
            continue;
        };

        match field_type_path.path.segments.last().unwrap().ident.to_string().as_str() {
            "String" | "i64" | "u64" | "f64" | "i32" | "u32" | "f32" | "i16" | "u16" | "i8" | "u8" | "bool" => {
                field_names.push(field_name);
                field_types.push(field_type);
            }
            _ => (),
        }
    }

    let expanded = quote! {
        impl field_accessor_as_string_trait::FieldAccessorAsStringTrait for #struct_name {
            fn set_field(&mut self, field_name: &str, value: &str) -> Result<(), String> {
                match field_name {
                    #(stringify!(#field_names) => {
                        match value.parse() {
                            Ok(value) => {
                                self.#field_names = value;
                                Ok(())
                            },
                            Err(err) => Err(format!("Failed to parse value: {}", err)),
                        }
                    },)*
                    _ => #set_error_match,
                }
            }
            fn get_field(&self, field_name: &str) -> Result<String, String> {
                match field_name {
                    #(stringify!(#field_names) => Ok(self.#field_names.to_string()),)*
                    _ => #get_error_match,
                }
            }
            fn is_field(&self, field_name: &str) -> bool {
                match field_name {
                    #(stringify!(#field_names) => true,)*
                    _ => #is_error_match,
                }
            }
        }
    };

    expanded.into()
}

fn should_skip(attrs: &[syn::Attribute]) -> bool {
    attrs
        .iter()
        .any(|attr| if let Some(ident) = attr.path().get_ident() { ident == "skip_field" } else { false })
}

fn is_deref(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|attr| {
        if let Some(ident) = attr.path().get_ident() {
            ident == "deref_field"
        } else {
            false
        }
    })
}
