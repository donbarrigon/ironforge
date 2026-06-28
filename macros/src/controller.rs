use proc_macro::TokenStream;
use quote::quote;
use syn::{ItemFn, ReturnType, Visibility, parse_macro_input};

pub fn controller_macro(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);

    if !matches!(input.vis, Visibility::Public(_)) {
        return syn::Error::new_spanned(
            &input.sig.ident,
            format!(
                "the controller `{}` must be public — use `pub fn {}`",
                input.sig.ident, input.sig.ident,
            ),
        )
        .to_compile_error()
        .into();
    }

    if input.sig.asyncness.is_some() {
        return syn::Error::new_spanned(
            &input.sig.asyncness,
            "do not use `async` — #[controller] adds it automatically",
        )
        .to_compile_error()
        .into();
    }

    if !matches!(input.sig.output, ReturnType::Default) {
        return syn::Error::new_spanned(
            &input.sig.output,
            "do not declare a return type — #[controller] adds it automatically",
        )
        .to_compile_error()
        .into();
    }

    let vis = &input.vis;
    let name = &input.sig.ident;
    let inputs = &input.sig.inputs;
    let body = &input.block;

    let expanded = quote! {
        #vis async fn #name(#inputs) -> ::std::result::Result<(), HttpError> {
            #body
        }
    };

    expanded.into()
}

pub fn middleware_macro(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);

    if !matches!(input.vis, Visibility::Public(_)) {
        return syn::Error::new_spanned(
            &input.sig.ident,
            format!(
                "the middleware `{}` must be public — use `pub fn {}`",
                input.sig.ident, input.sig.ident,
            ),
        )
        .to_compile_error()
        .into();
    }

    if input.sig.asyncness.is_some() {
        return syn::Error::new_spanned(
            &input.sig.asyncness,
            "do not use `async` — #[middleware] adds it automatically",
        )
        .to_compile_error()
        .into();
    }

    if !matches!(input.sig.output, ReturnType::Default) {
        return syn::Error::new_spanned(
            &input.sig.output,
            "do not declare a return type — #[middleware] adds it automatically",
        )
        .to_compile_error()
        .into();
    }

    let vis = &input.vis;
    let name = &input.sig.ident;
    let inputs = &input.sig.inputs;
    let body = &input.block;

    let expanded = quote! {
        #vis async fn #name(#inputs) -> ::std::result::Result<(), HttpError> {
            #body
        }
    };

    expanded.into()
}
