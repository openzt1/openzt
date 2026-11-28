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
    let mut perf_items: Vec<syn::Item> = Vec::new();

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

                perf_items.extend(perf_stats_items(func, &detour_name));
            }
        }
        items.extend(perf_items);

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
                .flat_map(|info| {
                    let detour_name = &info.detour_name;
                    let detour_static_name = Ident::new(&format!("{}_DETOUR", detour_name), detour_name.span());
                    let stat_name = perf_stat_ident(detour_name);
                    let enable: Stmt = parse_quote! {
                        #detour_static_name.enable()?;
                    };
                    let register: Stmt = parse_quote! {
                        #[cfg(feature = "perf-stats")]
                        crate::perf_stats::register(&#stat_name);
                    };
                    [enable, register]
                })
                .collect();

            let init_fn: syn::Item = parse_quote! {
                pub unsafe fn init_detours() -> ::retour::Result<()> {
                    #(#enables)*
                    Ok(())
                }
            };

            items.push(init_fn);

            let status_entries: Vec<proc_macro2::TokenStream> = detour_infos
                .iter()
                .map(|info| {
                    let detour_name = &info.detour_name;
                    let detour_static_name = Ident::new(&format!("{}_DETOUR", detour_name), detour_name.span());
                    quote! { (::std::stringify!(#detour_name), #detour_static_name.is_enabled()) }
                })
                .collect();

            let status_fn: syn::Item = parse_quote! {
                /// `(name, is_enabled)` per detour, in declaration order - lets the live battery's
                /// `*_DETOURS_ENABLED` wiring checks catch a silently-failed `init_detours()` without any
                /// hand-maintained list to keep in sync.
                #[cfg(feature = "reimplementation-tests")]
                #[allow(dead_code)]
                pub fn status() -> ::std::vec::Vec<(&'static str, bool)> {
                    ::std::vec![#(#status_entries),*]
                }
            };

            items.push(status_fn);
        }
    }

    TokenStream::from(quote! { #module })
}

fn perf_stat_ident(detour_name: &Ident) -> Ident {
    Ident::new(&format!("{}_PERF_STAT", detour_name), detour_name.span())
}

/// Items that time a detour under the `perf-stats` feature. The user's function is emitted unchanged without
/// the feature; with it, the user's body is renamed to `__perf_inner_<name>` and a wrapper with the original
/// name and signature holds a [`Stat`] guard around the call, so the generated `*_DETOUR` static and the
/// `retour` hook point at the wrapper without knowing about it. The wrapper forwards every argument and the
/// return value unchanged, so it does not depend on the calling convention.
fn perf_stats_items(func: &mut syn::ItemFn, detour_name: &Ident) -> Vec<syn::Item> {
    let fn_name = func.sig.ident.clone();
    let inner_name = Ident::new(&format!("__perf_inner_{}", fn_name), fn_name.span());
    let stat_name = perf_stat_ident(detour_name);

    let mut inner = func.clone();
    inner.sig.ident = inner_name.clone();
    inner.attrs.push(parse_quote! { #[cfg(feature = "perf-stats")] });
    inner.attrs.push(parse_quote! { #[allow(dead_code)] });

    let arg_idents: Vec<Ident> = (0..func.sig.inputs.len()).map(|i| Ident::new(&format!("__perf_arg{}", i), fn_name.span())).collect();
    let params: Vec<proc_macro2::TokenStream> = func
        .sig
        .inputs
        .iter()
        .zip(&arg_idents)
        .map(|(arg, ident)| match arg {
            FnArg::Typed(pat_type) => {
                let ty = &pat_type.ty;
                quote! { #ident: #ty }
            }
            FnArg::Receiver(_) => panic!("#[detour] functions cannot take self"),
        })
        .collect();

    let attrs: Vec<&syn::Attribute> = func.attrs.iter().filter(|attr| !attr.path().is_ident("inline")).collect();
    let vis = &func.vis;
    let unsafety = &func.sig.unsafety;
    let abi = &func.sig.abi;
    let output = &func.sig.output;
    let call = if func.sig.unsafety.is_some() {
        quote! { unsafe { #inner_name(#(#arg_idents),*) } }
    } else {
        quote! { #inner_name(#(#arg_idents),*) }
    };

    let stat: syn::Item = parse_quote! {
        #[cfg(feature = "perf-stats")]
        static #stat_name: crate::perf_stats::Stat =
            crate::perf_stats::Stat::new(::std::concat!(::std::module_path!(), "::", ::std::stringify!(#detour_name)));
    };
    let wrapper: syn::Item = parse_quote! {
        #(#attrs)*
        #[cfg(feature = "perf-stats")]
        #vis #unsafety #abi fn #fn_name(#(#params),*) #output {
            let __perf_guard = #stat_name.enter();
            #call
        }
    };

    func.attrs.push(parse_quote! { #[cfg(not(feature = "perf-stats"))] });
    vec![stat, syn::Item::Fn(inner), wrapper]
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
