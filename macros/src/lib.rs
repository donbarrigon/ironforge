mod controller;
mod create_error;
mod router_builder;

use proc_macro::TokenStream;

#[proc_macro_attribute]
pub fn controller(attr: TokenStream, item: TokenStream) -> TokenStream {
    return controller::controller_macro(attr, item);
}

#[proc_macro]
pub fn create_error(input: TokenStream) -> TokenStream {
    return create_error::create_error_macro(input);
}

#[proc_macro]
pub fn router_build(input: TokenStream) -> TokenStream {
    return router_builder::router_builder_macro(input);
}
