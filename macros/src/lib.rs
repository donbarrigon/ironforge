mod controller;
mod router_builder;

use proc_macro::TokenStream;

#[proc_macro_attribute]
pub fn controller(attr: TokenStream, item: TokenStream) -> TokenStream {
    controller::controller_macro(attr, item)
}

#[proc_macro_attribute]
pub fn middleware(attr: TokenStream, item: TokenStream) -> TokenStream {
    controller::middleware_macro(attr, item)
}

#[proc_macro]
pub fn router_build(input: TokenStream) -> TokenStream {
    return router_builder::router_builder_macro(input);
}
