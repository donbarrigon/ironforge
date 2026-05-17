use proc_macro::TokenStream;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{Expr, ExprLit, ExprPath, Ident, Lit, Result, Token, braced, bracketed, parenthesized};

const HTTP_METHODS: &[&str] = &[
    "connect", "delete", "get", "head", "options", "patch", "post", "put", "trace",
];

pub fn router_builder_macro(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as RouterInput);
    let builder = &input.builder;
    let statements = expand_items(&input.items);

    let builder_ref = if input.is_mut {
        quote! { &mut #builder }
    } else {
        quote! { #builder }
    };

    quote! {{
        let __ironforge_router_builder = #builder_ref;
        #(#statements)*
    }}
    .into()
}

// ─── Input ────────────────────────────────────────────────────────────────────

struct RouterInput {
    is_mut: bool,
    builder: Expr,
    items: Vec<RouterItem>,
}

impl Parse for RouterInput {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let content;
        parenthesized!(content in input);
        let is_mut = content.parse::<Token![mut]>().is_ok();
        let builder = content.parse()?;

        let body;
        braced!(body in input);
        let items = parse_items(&body)?;

        Ok(Self { is_mut, builder, items })
    }
}

// ─── Items ────────────────────────────────────────────────────────────────────

enum RouterItem {
    Route(RouteItem),
    Group(GroupItem),
    Middleware(MiddlewareItem),
}

struct RouteItem {
    method: Ident,
    path: Expr,
    controller: ExprPath,
    name: Option<Expr>,
    middlewares: Vec<ExprPath>,
}

struct GroupItem {
    prefixes: Vec<Expr>,
    middlewares: Vec<ExprPath>,
    items: Vec<RouterItem>,
}

struct MiddlewareItem {
    middlewares: Vec<ExprPath>,
    items: Vec<RouterItem>,
}

// ─── Parsers ──────────────────────────────────────────────────────────────────

fn parse_items(input: ParseStream<'_>) -> Result<Vec<RouterItem>> {
    let mut items = Vec::new();

    while !input.is_empty() {
        let item_name: Ident = input.parse()?;
        let item_name_string = item_name.to_string();

        let item = match item_name_string.as_str() {
            "group" => RouterItem::Group(parse_group(input)?),
            "middleware" => RouterItem::Middleware(parse_middleware(input)?),
            method if HTTP_METHODS.contains(&method) => RouterItem::Route(parse_route(item_name, input)?),
            _ => {
                return Err(syn::Error::new_spanned(
                    item_name,
                    "expected an HTTP method, group(...) { ... }, or middleware(...) { ... }",
                ));
            }
        };

        items.push(item);
        let _ = input.parse::<Token![;]>();
    }

    Ok(items)
}

fn parse_route(method: Ident, input: ParseStream<'_>) -> Result<RouteItem> {
    let content;
    parenthesized!(content in input);

    let args = Punctuated::<Expr, Token![,]>::parse_terminated(&content)?;
    if args.len() < 2 || args.len() > 3 {
        return Err(syn::Error::new_spanned(
            method,
            "route definitions must be method(path, controller[, name])",
        ));
    }

    let mut args = args.into_iter();
    let path = args.next().expect("checked route path");
    let controller = match args.next().expect("checked route controller") {
        Expr::Path(controller) => controller,
        expr => {
            return Err(syn::Error::new_spanned(
                expr,
                "the route controller must be a function path",
            ));
        }
    };
    let name = args.next();

    let middlewares = if input.peek(syn::token::Bracket) {
        let content;
        bracketed!(content in input);
        Punctuated::<ExprPath, Token![,]>::parse_terminated(&content)?
            .into_iter()
            .collect()
    } else {
        Vec::new()
    };

    Ok(RouteItem {
        method,
        path,
        controller,
        name,
        middlewares,
    })
}

fn parse_group(input: ParseStream<'_>) -> Result<GroupItem> {
    // Parsear uno o varios prefixes separados por coma: group("api", "v1")
    let content;
    parenthesized!(content in input);
    let prefixes = Punctuated::<Expr, Token![,]>::parse_terminated(&content)?
        .into_iter()
        .collect::<Vec<_>>();

    if prefixes.is_empty() {
        return Err(syn::Error::new(
            content.span(),
            "group(...) requires at least one path segment",
        ));
    }

    // Validar que todos sean string literals
    for prefix in &prefixes {
        match prefix {
            Expr::Lit(ExprLit { lit: Lit::Str(_), .. }) => {}
            expr => {
                return Err(syn::Error::new_spanned(
                    expr,
                    "group path segments must be string literals",
                ));
            }
        }
    }

    // Middlewares opcionales: [auth, rate_limit]
    let middlewares = if input.peek(syn::token::Bracket) {
        let content;
        bracketed!(content in input);
        Punctuated::<ExprPath, Token![,]>::parse_terminated(&content)?
            .into_iter()
            .collect()
    } else {
        Vec::new()
    };

    // Body
    let body;
    braced!(body in input);
    let items = parse_items(&body)?;

    Ok(GroupItem {
        prefixes,
        middlewares,
        items,
    })
}

fn parse_middleware(input: ParseStream<'_>) -> Result<MiddlewareItem> {
    // Parsear uno o varios middlewares separados por coma: middleware(auth, admin)
    let content;
    parenthesized!(content in input);
    let middlewares = Punctuated::<ExprPath, Token![,]>::parse_terminated(&content)?
        .into_iter()
        .collect::<Vec<_>>();

    if middlewares.is_empty() {
        return Err(syn::Error::new(
            content.span(),
            "middleware(...) requires at least one middleware function",
        ));
    }

    // Body
    let body;
    braced!(body in input);
    let items = parse_items(&body)?;

    Ok(MiddlewareItem { middlewares, items })
}

// ─── Expanders ────────────────────────────────────────────────────────────────

fn expand_items(items: &[RouterItem]) -> Vec<proc_macro2::TokenStream> {
    items.iter().map(expand_item).collect()
}

fn expand_item(item: &RouterItem) -> proc_macro2::TokenStream {
    match item {
        RouterItem::Route(route) => expand_route(route),
        RouterItem::Group(group) => expand_group(group),
        RouterItem::Middleware(middleware) => expand_middleware(middleware),
    }
}

fn expand_route(route: &RouteItem) -> proc_macro2::TokenStream {
    let method = route.method.to_string().to_uppercase();
    let path = &route.path;
    let controller = &route.controller;
    let middlewares = &route.middlewares;
    let controller_name = route
        .controller
        .path
        .segments
        .last()
        .map(|segment| segment.ident.to_string())
        .unwrap_or_else(|| "controller".to_string());
    let name = route.name.as_ref();

    let apply_name = name.map(|name| {
        quote! {
            if let Some(__ironforge_path) = __ironforge_router_builder.paths.last_mut() {
                __ironforge_path.name(#name);
            }
        }
    });

    let apply_middlewares = middlewares.iter().map(|middleware| {
        quote! {
            if let Some(__ironforge_path) = __ironforge_router_builder.paths.last_mut() {
                __ironforge_path.use_middleware(::std::sync::Arc::new(|__ironforge_context| {
                    ::std::boxed::Box::pin(#middleware(__ironforge_context))
                }));
            }
        }
    });

    quote! {
        __ironforge_router_builder.add_path(
            #method.to_string(),
            (#path).to_string(),
            #controller_name.to_string(),
            ::std::sync::Arc::new(|__ironforge_context| {
                ::std::boxed::Box::pin(#controller(__ironforge_context))
            }),
        );
        #apply_name
        #(#apply_middlewares)*
    }
}

fn expand_group(group: &GroupItem) -> proc_macro2::TokenStream {
    let prefixes = &group.prefixes;
    let middlewares = &group.middlewares;
    let statements = expand_items(&group.items);

    // Pushear cada prefix al stack
    let push_prefixes = prefixes.iter().map(|prefix| {
        quote! {
            __ironforge_router_builder.prefixes.push((#prefix).to_string());
        }
    });

    // Popear en orden inverso
    let pop_prefixes = prefixes.iter().map(|_| {
        quote! {
            __ironforge_router_builder.prefixes.pop();
        }
    });

    // Pushear middlewares del group
    let push_middlewares = middlewares.iter().map(|middleware| {
        quote! {
            __ironforge_router_builder.middlewares.push(::std::sync::Arc::new(|__ironforge_context| {
                ::std::boxed::Box::pin(#middleware(__ironforge_context))
            }));
        }
    });

    // Popear middlewares en orden inverso
    let pop_middlewares = middlewares.iter().map(|_| {
        quote! {
            __ironforge_router_builder.middlewares.pop();
        }
    });

    quote! {
        #(#push_prefixes)*
        #(#push_middlewares)*
        #(#statements)*
        #(#pop_middlewares)*
        #(#pop_prefixes)*
    }
}

fn expand_middleware(middleware: &MiddlewareItem) -> proc_macro2::TokenStream {
    let middlewares = &middleware.middlewares;
    let statements = expand_items(&middleware.items);

    let push_middlewares = middlewares.iter().map(|mw| {
        quote! {
            __ironforge_router_builder.middlewares.push(::std::sync::Arc::new(|__ironforge_context| {
                ::std::boxed::Box::pin(#mw(__ironforge_context))
            }));
        }
    });

    let pop_middlewares = middlewares.iter().map(|_| {
        quote! {
            __ironforge_router_builder.middlewares.pop();
        }
    });

    quote! {
        #(#push_middlewares)*
        #(#statements)*
        #(#pop_middlewares)*
    }
}
