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
            if let syn::Item::Fn(func) = item && let Some(detour_attr) = func.attrs.iter().position(|attr| attr.path().is_ident("detour")) {
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

fn extract_bare_fn_type(ty: &syn::Type) -> Option<syn::TypeBareFn> {
    if let syn::Type::Path(p) = ty {
        if let Some(seg) = p.path.segments.last() {
            if seg.ident == "FunctionDef" {
                if let syn::PathArguments::AngleBracketed(args) = &seg.arguments {
                    if let Some(syn::GenericArgument::Type(syn::Type::BareFn(f))) = args.args.first() {
                        return Some(f.clone());
                    }
                }
            }
        }
    }
    None
}

#[proc_macro_attribute]
pub fn validate_detour(attr: TokenStream, input: TokenStream) -> TokenStream {
    let name_lit = parse_macro_input!(attr as syn::LitStr);
    let name = name_lit.value();

    let const_item = parse_macro_input!(input as syn::ItemConst);

    let bare_fn = extract_bare_fn_type(&const_item.ty)
        .expect("validate_detour: expected FunctionDef<fn(...)> type");

    let const_name = &const_item.ident;
    let static_name = Ident::new(
        &format!("{}_VALIDATION", const_name),
        const_name.span(),
    );
    let enable_fn_name = Ident::new(
        &format!("enable_{}_validation", const_name.to_string().to_lowercase()),
        const_name.span(),
    );

    let abi = &bare_fn.abi;
    let unsafety = &bare_fn.unsafety;

    let param_names: Vec<Ident> = (0..bare_fn.inputs.len())
        .map(|i| Ident::new(&format!("_a{}", i), proc_macro2::Span::call_site()))
        .collect();
    let param_types: Vec<&syn::Type> = bare_fn.inputs.iter()
        .map(|arg| &arg.ty)
        .collect();

    let return_type = match &bare_fn.output {
        syn::ReturnType::Default => quote! { () },
        syn::ReturnType::Type(_, ty) => quote! { #ty },
    };

    let fn_type = if unsafety.is_some() {
        quote! { unsafe #abi fn(#(#param_types),*) -> #return_type }
    } else {
        quote! { #abi fn(#(#param_types),*) -> #return_type }
    };

    let name_str = name.as_str();
    let expect_msg = format!("Failed to create validation detour for {}", name);

    let output = quote! {
        #const_item

        pub(crate) static #static_name:
            ::std::sync::LazyLock<::retour::GenericDetour<#fn_type>>
        = ::std::sync::LazyLock::new(|| {
            #unsafety #abi fn stub(#(#param_names: #param_types),*) -> #return_type {
                ::tracing::info!("DETOUR_CALLED: {}", #name_str);
                unsafe { #static_name.call(#(#param_names),*) }
            }
            unsafe { #const_name.detour(stub) }
                .expect(#expect_msg)
        });

        pub(crate) fn #enable_fn_name() -> ::retour::Result<()> {
            unsafe { #static_name.enable() }
        }

        ::inventory::submit! {
            crate::ValidationEntry {
                name: #name_str,
                enable: #enable_fn_name,
            }
        }
    };

    output.into()
}
