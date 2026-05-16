use proc_macro::TokenStream;
use proc_macro2::TokenTree;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::quote;
use syn::{
    Expr, Ident, LitStr, Token,
    parse::{Parse, ParseStream},
    parse_macro_input,
};

// ─── Funciones validas de HttpError ──────────────────────────────────────────

const VALID_KINDS: &[&str] = &[
    // 4xx
    "bad_request",
    "unauthorized",
    "payment_required",
    "forbidden",
    "not_found",
    "method_not_allowed",
    "not_acceptable",
    "proxy_authentication_required",
    "request_timeout",
    "conflict",
    "gone",
    "length_required",
    "precondition_failed",
    "payload_too_large",
    "uri_too_long",
    "unsupported_media_type",
    "range_not_satisfiable",
    "expectation_failed",
    "im_a_teapot",
    "misdirected_request",
    "unprocessable_entity",
    "locked",
    "failed_dependency",
    "upgrade_required",
    "precondition_required",
    "too_many_requests",
    "request_header_fields_too_large",
    "unavailable_for_legal_reasons",
    // 5xx
    "internal_server_error",
    "not_implemented",
    "bad_gateway",
    "service_unavailable",
    "gateway_timeout",
    "http_version_not_supported",
    "variant_also_negotiates",
    "insufficient_storage",
    "loop_detected",
    "not_extended",
    "network_authentication_required",
];

// ─── Input parser ─────────────────────────────────────────────────────────────

enum Cause {
    Empty,
    Str(LitStr),
    Expr(Expr),
}

struct ErrorInput {
    kind: Option<Ident>,
    message: Expr,
    cause: Option<Cause>,
    data: Option<TokenStream2>,
}

impl Parse for ErrorInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        // Primer token: kind (ident valido) o message (expr)
        let kind: Option<Ident> = if input.peek(Ident) {
            let fork = input.fork();
            let ident: Ident = fork.parse()?;
            let is_valid = VALID_KINDS.contains(&ident.to_string().as_str());
            if is_valid && fork.peek(Token![,]) {
                let ident: Ident = input.parse()?;
                input.parse::<Token![,]>()?;
                Some(ident)
            } else {
                None
            }
        } else {
            None
        };

        // Message
        let message: Expr = input.parse()?;

        // Cause (opcional)
        let cause = if input.peek(Token![,]) {
            input.parse::<Token![,]>()?;
            if input.is_empty() || input.peek(Token![,]) {
                None
            } else if input.peek(LitStr) {
                Some(Cause::Str(input.parse()?))
            } else if input.peek(Ident) {
                let ident: Ident = input.parse()?;
                if ident == "Empty" {
                    Some(Cause::Empty)
                } else {
                    let expr = syn::parse2(quote! { #ident })?;
                    Some(Cause::Expr(expr))
                }
            } else {
                Some(Cause::Expr(input.parse()?))
            }
        } else {
            None
        };

        // Data (opcional) — capturar el grupo { } completo
        let data = if input.peek(Token![,]) {
            input.parse::<Token![,]>()?;
            if input.is_empty() {
                None
            } else {
                // Capturar como TokenTree para preservar el { } con su contenido
                let tt: TokenTree = input.parse()?;
                Some(quote! { #tt })
            }
        } else {
            None
        };

        Ok(ErrorInput {
            kind,
            message,
            cause,
            data,
        })
    }
}

// ─── Macro ───────────────────────────────────────────────────────────────────

pub fn create_error_macro(input: TokenStream) -> TokenStream {
    let ErrorInput {
        kind,
        message,
        cause,
        data,
    } = parse_macro_input!(input as ErrorInput);

    // Determinar el kind — default: internal_server_error
    let kind_ident = match kind {
        Some(k) => k,
        None => Ident::new("internal_server_error", Span::call_site()),
    };

    // Construir el cause
    let cause_tokens = match cause {
        None | Some(Cause::Empty) => quote! { ::ironforge::error::Empty },
        Some(Cause::Str(s)) => quote! {
            ::std::io::Error::new(::std::io::ErrorKind::Other, #s)
        },
        Some(Cause::Expr(e)) => quote! { #e },
    };

    // Construir el error base
    let base = quote! {
        ::ironforge::error::HttpError::#kind_ident(#message, #cause_tokens)
    };

    // Agregar data si existe
    let expanded = match data {
        Some(d) => quote! {
            #base.with_data(::serde_json::json!(#d))
        },
        None => base,
    };

    expanded.into()
}
