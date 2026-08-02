// ironforge-macros/src/rules.rs
//
// Implementa la proc-macro `rules!`, usada así:
//
//   let mut err = rules! {
//       self, c,
//       name("nombreCompleto") => [ required(), min(3), max(255) ],
//       email => [ required(), email() ],
//       age => [
//           !empty().msg("la edad no puede estar vacía"),
//           !between(18, 62).msg("la edad debe estar entre 18 y 62"),
//       ],
//   };
//
// Nota de sintaxis: un macro tipo función solo acepta UN grupo
// delimitado (`{...}`, `(...)` o `[...]`). Por eso todo -- `self`, `c`
// y los campos -- va dentro de un solo `{ }`, separados por comas, y no
// como `rules!(self, c) { ... }` (dos grupos seguidos no es sintaxis
// válida de invocación de macro en Rust).
//
// Expansión generada (equivalente a lo que harías a mano):
//
//   {
//       let mut err = self.prepare_for_validation(c);
//       {
//           let fe = err.push_field("nombreCompleto".to_string());
//           if !(self.name.required()) {
//               let mut placeholders: Placeholders = Vec::new();
//               placeholders.push(("field", "nombreCompleto".to_string()));
//               fe.push((MSG_REQUIRED.to_string(), placeholders));
//           }
//           if !(self.name.min(3)) {
//               let mut placeholders: Placeholders = Vec::new();
//               placeholders.push(("field", "nombreCompleto".to_string()));
//               placeholders.push(("min", (3).to_string()));
//               fe.push((MSG_MIN.to_string(), placeholders));
//           }
//           // ... max(255) similar ...
//       }
//       { /* bloque de "email" */ }
//       { /* bloque de "age", con negación */ }
//       err
//   }
//
// El macro NO llama `validate()` -- eso lo sigue haciendo la firma del
// trait `Validator`. `rules!` solo produce el `ValidationError` que
// `validate()` retorna.

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::{
    Expr, Ident, LitStr, Token, braced, bracketed, parenthesized,
    parse::{Parse, ParseStream},
    parse_macro_input,
    punctuated::Punctuated,
};

// ===============================================================================
// AST del macro
// ===============================================================================

struct RulesInput {
    self_expr: Expr,
    ctx_expr: Expr,
    fields: Vec<FieldBlock>,
}

impl Parse for RulesInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let self_expr: Expr = input.parse()?;
        input.parse::<Token![,]>()?;
        let ctx_expr: Expr = input.parse()?;
        // coma final después de ctx antes del primer campo (o del final
        // si no hay campos -- caso borde, poco útil pero no debe explotar)
        if input.peek(Token![,]) {
            input.parse::<Token![,]>()?;
        }

        let fields: Punctuated<FieldBlock, Token![,]> = Punctuated::parse_terminated(input)?;

        Ok(RulesInput {
            self_expr,
            ctx_expr,
            fields: fields.into_iter().collect(),
        })
    }
}

struct FieldBlock {
    field_ident: Ident,
    json_name: Option<LitStr>,
    rules: Vec<RuleCall>,
}

impl Parse for FieldBlock {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let field_ident: Ident = input.parse()?;

        // Override opcional: name("nombreCompleto")
        let json_name = if input.peek(syn::token::Paren) {
            let content;
            parenthesized!(content in input);
            let lit: LitStr = content.parse()?;
            Some(lit)
        } else {
            None
        };

        input.parse::<Token![=>]>()?;

        let content;
        bracketed!(content in input);
        let rules: Punctuated<RuleCall, Token![,]> = Punctuated::parse_terminated(&content)?;

        Ok(FieldBlock {
            field_ident,
            json_name,
            rules: rules.into_iter().collect(),
        })
    }
}

struct RuleCall {
    negated: bool,
    method: Ident,
    args: Vec<Expr>,
    msg: Option<LitStr>,
}

impl Parse for RuleCall {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let negated = input.parse::<Option<Token![!]>>()?.is_some();
        let method: Ident = input.parse()?;

        let content;
        parenthesized!(content in input);
        let args: Punctuated<Expr, Token![,]> = Punctuated::parse_terminated(&content)?;
        let args: Vec<Expr> = args.into_iter().collect();

        let msg = if input.peek(Token![.]) {
            input.parse::<Token![.]>()?;
            let msg_ident: Ident = input.parse()?;
            if msg_ident != "msg" {
                return Err(syn::Error::new(
                    msg_ident.span(),
                    "solo se permite `.msg(\"...\")` después de una regla",
                ));
            }
            let content2;
            parenthesized!(content2 in input);
            let lit: LitStr = content2.parse()?;
            Some(lit)
        } else {
            None
        };

        Ok(RuleCall {
            negated,
            method,
            args,
            msg,
        })
    }
}

// ===============================================================================
// Codegen
// ===============================================================================

pub fn rules_impl(input: TokenStream) -> TokenStream {
    let RulesInput {
        self_expr,
        ctx_expr,
        fields,
    } = parse_macro_input!(input as RulesInput);

    let field_blocks: Vec<TokenStream2> = fields.iter().map(|field| gen_field_block(&self_expr, field)).collect();

    let expanded = quote! {
        {
            let mut err = #self_expr.prepare_for_validation(#ctx_expr);
            #(#field_blocks)*
            err
        }
    };

    expanded.into()
}

fn gen_field_block(self_expr: &Expr, field: &FieldBlock) -> TokenStream2 {
    let field_ident = &field.field_ident;
    let json_name: String = field
        .json_name
        .as_ref()
        .map(|lit| lit.value())
        .unwrap_or_else(|| field_ident.to_string());

    let rule_checks: Vec<TokenStream2> = field
        .rules
        .iter()
        .map(|rule| gen_rule_check(self_expr, field_ident, &json_name, rule))
        .collect();

    quote! {
        {
            let fe = err.push_field(#json_name.to_string());
            #(#rule_checks)*
        }
    }
}

fn gen_rule_check(self_expr: &Expr, field_ident: &Ident, json_name: &str, rule: &RuleCall) -> TokenStream2 {
    let method = &rule.method;
    let args = &rule.args;
    let negated = rule.negated;

    // `re_ex` es la única regla que puede fallar en sí misma (patrón
    // dinámico que no compiló) -- se trata igual que un `static_regex!`
    // con patrón hardcodeado mal escrito: es un bug del desarrollador,
    // no un error de validación del dato, así que se panickea temprano
    // en vez de intentar reportarlo como un ValidationError.
    let call = if method == "re_ex" {
        quote! {
            #self_expr.#field_ident.#method(#(#args),*)
                .expect("re_ex: el patrón regex no compiló")
        }
    } else {
        quote! { #self_expr.#field_ident.#method(#(#args),*) }
    };

    // Sin negar: la regla es "debe cumplirse", entonces el error ocurre
    // cuando el método da `false`. Negada: la regla es "NO debe
    // cumplirse", el error ocurre cuando el método da `true`.
    let condition = if negated {
        quote! { #call }
    } else {
        quote! { !(#call) }
    };

    // Placeholders automáticos según cantidad de argumentos:
    // - 0 args  -> solo :field
    // - 1 arg   -> :field y uno nombrado igual que la regla (ej. min(3) -> :min)
    // - 2 args  -> :field, :min y :max (convención de las reglas de rango: between(18, 62))
    // - 3+ args -> :field y posicionales :arg0, :arg1, ... (caso raro; si
    //   se necesita un nombre más claro ahí, usar `.msg("...")` propio
    //   referenciando esos mismos nombres :argN)
    let mut placeholder_pushes: Vec<TokenStream2> = vec![quote! {
        placeholders.push(("field", #json_name.to_string()));
    }];

    match args.len() {
        0 => {}
        1 => {
            let name = method.to_string();
            let a0 = &args[0];
            placeholder_pushes.push(quote! {
                placeholders.push((#name, (#a0).to_string()));
            });
        }
        2 => {
            let a0 = &args[0];
            let a1 = &args[1];
            placeholder_pushes.push(quote! { placeholders.push(("min", (#a0).to_string())); });
            placeholder_pushes.push(quote! { placeholders.push(("max", (#a1).to_string())); });
        }
        _ => {
            for (i, a) in args.iter().enumerate() {
                let name = format!("arg{i}");
                placeholder_pushes.push(quote! { placeholders.push((#name, (#a).to_string())); });
            }
        }
    }

    // Mensaje: si hay `.msg("...")`, se usa tal cual. Si no, se busca la
    // constante por defecto -- `MSG_<REGLA>` para la versión positiva,
    // `MSG_NOT_<REGLA>` para la negada (`!regla()`). Debe existir y
    // estar en scope (importada) donde se invoca `rules!`. Ej: `min` ->
    // `MSG_MIN` / `MSG_NOT_MIN`; `is_in` -> `MSG_IS_IN` / `MSG_NOT_IS_IN`.
    let msg_expr = if let Some(lit) = &rule.msg {
        quote! { #lit.to_string() }
    } else {
        let prefix = if negated { "MSG_NOT_" } else { "MSG_" };
        let msg_ident = format_ident!("{}{}", prefix, method.to_string().to_uppercase());
        quote! { #msg_ident.to_string() }
    };

    quote! {
        if #condition {
            let mut placeholders: crate::error::validation_error::Placeholders = Vec::new();
            #(#placeholder_pushes)*
            fe.push((#msg_expr, placeholders));
        }
    }
}
