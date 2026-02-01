use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, parse_quote, FnArg, Ident, ItemMod, ReturnType, Stmt};

struct DetourInfo {
    detour_name: Ident,
    function_name: Ident,
    function_signature: syn::Signature,
}

#[proc_macro_attribute]
pub fn detour_mod(_attr: TokenStream, input: TokenStream) -> TokenStream {
    let mut module = parse_macro_input!(input as ItemMod);

    let mut detour_infos = Vec::new();

    if let Some((_, items)) = &mut module.content {
        for item in items.iter_mut() {
            if let syn::Item::Fn(func) = item {
                if let Some(detour_attr) = func.attrs.iter().position(|attr| attr.path().is_ident("detour")) {
                    let attr = func.attrs.remove(detour_attr);

                    let detour_name = if let Ok(meta_list) = attr.meta.require_list() {
                        match syn::parse2::<Ident>(meta_list.tokens.clone()) {
                            Ok(ident) => ident,
                            Err(_) => panic!("detour attribute must contain a valid identifier"),
                        }
                    } else {
                        panic!("detour attribute must be in the form #[detour(DETOUR_NAME)]");
                    };

                    detour_infos.push(DetourInfo {
                        detour_name: detour_name.clone(),
                        function_name: func.sig.ident.clone(),
                        function_signature: func.sig.clone(),
                    });
                }
            }
        }

        for info in &detour_infos {
            let detour_name = &info.detour_name;
            let detour_static_name = Ident::new(&format!("{}_DETOUR", detour_name), detour_name.span());
            let function_name = &info.function_name;

            let fn_type = build_function_type(&info.function_signature);

            let detour_static: syn::Item = parse_quote! {
                static #detour_static_name: ::std::sync::LazyLock<::retour::GenericDetour<#fn_type>> =
                    ::std::sync::LazyLock::new(|| {
                        unsafe { #detour_name.detour(#function_name).unwrap() }
                    });
            };

            items.insert(0, detour_static);
        }

        if !detour_infos.is_empty() {
            let enables: Vec<Stmt> = detour_infos
                .iter()
                .map(|info| {
                    let detour_name = &info.detour_name;
                    let detour_static_name = Ident::new(&format!("{}_DETOUR", detour_name), detour_name.span());
                    parse_quote! {
                        #detour_static_name.enable()?;
                    }
                })
                .collect();

            let init_fn: syn::Item = parse_quote! {
                pub unsafe fn init_detours() -> ::retour::Result<()> {
                    #(#enables)*
                    Ok(())
                }
            };

            items.push(init_fn);
        }
    }

    TokenStream::from(quote! { #module })
}

fn build_function_type(sig: &syn::Signature) -> proc_macro2::TokenStream {
    let abi = &sig.abi;
    let inputs = &sig.inputs;
    let output = &sig.output;

    let input_types: Vec<_> = inputs
        .iter()
        .filter_map(|arg| match arg {
            FnArg::Typed(pat_type) => Some(&*pat_type.ty),
            FnArg::Receiver(_) => None,
        })
        .collect();

    let return_type = match output {
        ReturnType::Default => quote! { () },
        ReturnType::Type(_, ty) => quote! { #ty },
    };

    if sig.unsafety.is_some() {
        quote! { unsafe #abi fn(#(#input_types),*) -> #return_type }
    } else {
        quote! { #abi fn(#(#input_types),*) -> #return_type }
    }
}

#[proc_macro_attribute]
pub fn detour(_attr: TokenStream, input: TokenStream) -> TokenStream {
    input
}
