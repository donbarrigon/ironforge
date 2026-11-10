// ironforge-macros/src/rules.rs
//
// Implementa la proc-macro `rules!`, usada así dentro de `validate`:
//
//   impl Validator for UserRegisterValidator {
//       fn validate(&mut self, c: &mut Context) -> Result<(), ValidationError> {
//           rules! {
//               self, c,
//               nickname("nickName") => [ required(), min(3), max(255) ],
//               email => [ required(), re_email() ],
//               phone => [ required(), re_phone() ],
//               age => [
//                   !empty().msg("la edad no puede estar vacía"),
//                   !between(18, 62).msg("la edad debe estar entre 18 y 62"),
//               ],
//           }
//       }
//   }
//
// Nota de sintaxis: un macro tipo función solo acepta UN grupo
// delimitado (`{...}`, `(...)` o `[...]`). Por eso todo -- `self`, `c`
// y los campos -- va dentro de un solo `{ }`, separados por comas, y no
// como `rules!(self, c) { ... }` (dos grupos seguidos no es sintaxis
// válida de invocación de macro en Rust).
//
// La macro se expande a una expresión de tipo `Result<(), ValidationError>`,
// por eso puede ser directamente el cuerpo de `validate`.
//
// Expansión generada (equivalente a lo que harías a mano):
//
//   {
//       use ::ironforge::validator::required::*;   // traits + MSG_*
//       use ::ironforge::validator::range::*;
//       use ::ironforge::validator::range_time::*;
//       use ::ironforge::validator::format::*;
//       use ::ironforge::validator::membership::*;
//       use ::ironforge::validator::Validator as _;
//
//       let mut err: ValidationError = match self.prepare_for_validation(c) {
//           Ok(()) => ValidationError::new(),
//           Err(e) => e,
//       };
//       {
//           let mut msgs: Vec<(String, Placeholders)> = Vec::new();
//           if !(self.nickname.required()) {
//               let mut placeholders: Placeholders = Vec::new();
//               placeholders.push(("field", "nickName".to_string()));
//               msgs.push((MSG_REQUIRED.to_string(), placeholders));
//           }
//           if !(Range::min(&self.nickname, 3)) {
//               let mut placeholders: Placeholders = Vec::new();
//               placeholders.push(("field", "nickName".to_string()));
//               placeholders.push(("min", (3).to_string()));
//               msgs.push((MSG_MIN.to_string(), placeholders));
//           }
//           // ... max(255) similar ...
//           if !msgs.is_empty() {
//               let fe = err.push_field("nickName".to_string());
//               for m in msgs { fe.push(m); }
//           }
//       }
//       { /* bloque de "email" */ }
//       { /* bloque de "age", con negación */ }
//       if err.has_errors() { Err(err) } else { Ok(()) }
//   }
//
// Decisiones importantes:
//
// - Las rutas del código generado son absolutas (`::ironforge::...`)
//   porque `crate::` dentro de una proc-macro se resuelve en el crate
//   que la invoca (ej. spanails), no en ironforge. Los módulos
//   `validator::{required, range, range_time, format, membership}` y
//   `error::validation_error` deben ser `pub`. La macro trae a scope los
//   traits y las constantes `MSG_*` por sí sola: quien la use no importa
//   nada de eso.
//
// - `min`, `max`, `gt` y `lt` chocan con `Ord::min/max` y
//   `PartialOrd::gt/lt` de la librería estándar (que se resuelven primero
//   en la sintaxis `x.min(3)` porque toman `self` por valor). Por eso
//   para esos cuatro la macro llama el trait explícitamente:
//   `Range::min(&x, 3)`.
//
// - Los métodos de `Format` se llaman `re_email`, `re_phone`, etc., pero
//   sus mensajes por defecto son `MSG_EMAIL`, `MSG_PHONE`, etc. (sin el
//   prefijo `re_`). La macro quita el prefijo al buscar el mensaje. La
//   única excepción es `re_ex`, cuyo mensaje es `MSG_RE_EX`.
//
// - Un campo solo se registra en el `ValidationError` si tiene al menos
//   un mensaje. Si se registrara siempre, `has_errors()` sería siempre
//   `true` (por los `FieldError` vacíos) y la validación nunca pasaría.
//
// - Si `prepare_for_validation` falla, su ValidationError se usa como base
//   y se siguen acumulando los errores de los campos. Si prefieres cortar
//   temprano, cambia `Err(e) => e` por `Err(e) => return Err(e)` en
//   `rules_impl` (funciona porque la macro se expande dentro de `validate`).
//
// - Si algún día usas `rules!` dentro del propio crate ironforge, agrega
//   `extern crate self as ironforge;` en su `lib.rs` para que las rutas
//   `::ironforge::...` resuelvan.

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::{
    Expr, Ident, LitStr, Token, bracketed, parenthesized,
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

    // La expresión resultante es un `Result<(), ValidationError>`:
    // - los `use` traen a scope (solo dentro de este bloque) los traits de
    //   reglas y las constantes `MSG_*`, así quien invoca no importa nada
    // - parte de lo que devuelva `prepare_for_validation` (su error, si
    //   falló, se conserva y se sigue acumulando sobre él)
    // - al final, si hay errores retorna `Err(err)`, si no `Ok(())`
    let expanded = quote! {
        {
            #[allow(unused_imports)]
            use ::ironforge::validator::required::*;
            #[allow(unused_imports)]
            use ::ironforge::validator::range::*;
            #[allow(unused_imports)]
            use ::ironforge::validator::range_time::*;
            #[allow(unused_imports)]
            use ::ironforge::validator::format::*;
            #[allow(unused_imports)]
            use ::ironforge::validator::membership::*;
            #[allow(unused_imports)]
            use ::ironforge::validator::Validator as _;

            let mut err: ::ironforge::error::validation_error::ValidationError =
                match #self_expr.prepare_for_validation(#ctx_expr) {
                    Ok(()) => ::ironforge::error::validation_error::ValidationError::new(),
                    Err(e) => e,
                };
            #(#field_blocks)*
            if err.has_errors() {
                Err(err)
            } else {
                Ok(())
            }
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

    // Los mensajes se acumulan primero en `msgs`, y el campo solo se
    // registra en `err` si hay al menos uno. Si se llamara `push_field`
    // siempre, quedarían `FieldError` vacíos y `has_errors()` daría
    // `true` aunque todo fuera válido.
    quote! {
        {
            let mut msgs: Vec<(String, ::ironforge::error::validation_error::Placeholders)> = Vec::new();
            #(#rule_checks)*
            if !msgs.is_empty() {
                let fe = err.push_field(#json_name.to_string());
                for m in msgs {
                    fe.push(m);
                }
            }
        }
    }
}

fn gen_rule_check(self_expr: &Expr, field_ident: &Ident, json_name: &str, rule: &RuleCall) -> TokenStream2 {
    let method = &rule.method;
    let args = &rule.args;
    let negated = rule.negated;
    let rule_name = method.to_string();

    // `re_ex` es la única regla que puede fallar en sí misma (patrón
    // dinámico que no compiló) -- se trata igual que un `static_regex!`
    // con patrón hardcodeado mal escrito: es un bug del desarrollador,
    // no un error de validación del dato, así que se panickea temprano
    // en vez de intentar reportarlo como un ValidationError.
    //
    // `min`, `max`, `gt` y `lt` se llaman con el trait explícito porque
    // `Ord::min/max` y `PartialOrd::gt/lt` de std ganarían la resolución
    // de `x.min(3)` (toman `self` por valor, se prueban antes que `&self`).
    let call = if rule_name == "re_ex" {
        quote! {
            #self_expr.#field_ident.#method(#(#args),*)
                .expect("re_ex: el patrón regex no compiló")
        }
    } else if matches!(rule_name.as_str(), "is_in" | "not_in") {
        // `campo => [ is_in(&LISTA) ]`: el argumento es la lista y el campo
        // es el valor buscado (al revés que `x.is_in(..)`). Se compara con
        // `==` para aceptar, por ejemplo, un `String` contra `&[&str]`.
        let Some(list) = args.first().filter(|_| args.len() == 1) else {
            return syn::Error::new(method.span(), format!("`{rule_name}` recibe exactamente un argumento: la lista"))
                .to_compile_error();
        };
        let found = quote! { (#list).iter().any(|item| *item == #self_expr.#field_ident) };
        if rule_name == "is_in" { found } else { quote! { !(#found) } }
    } else if matches!(rule_name.as_str(), "min" | "max" | "gt" | "lt") {
        quote! {
            ::ironforge::validator::range::Range::#method(&#self_expr.#field_ident, #(#args),*)
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
        // La lista de `is_in`/`not_in` no va al mensaje: puede ser enorme y no implementa `Display`.
        1 if matches!(rule_name.as_str(), "is_in" | "not_in") => {}
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
    // `MSG_NOT_<REGLA>` para la negada (`!regla()`). Las constantes ya
    // están en scope por los `use` que genera `rules_impl`.
    //
    // Las reglas de `Format` se llaman `re_<algo>` pero su mensaje es
    // `MSG_<ALGO>` (sin `re_`): `re_email` -> `MSG_EMAIL`. La excepción
    // es `re_ex`, que sí es `MSG_RE_EX`.
    let msg_expr = if let Some(lit) = &rule.msg {
        quote! { #lit.to_string() }
    } else {
        let prefix = if negated { "MSG_NOT_" } else { "MSG_" };
        let msg_base: &str = if rule_name == "re_ex" {
            &rule_name
        } else {
            rule_name.strip_prefix("re_").unwrap_or(&rule_name)
        };
        let msg_ident = format_ident!("{}{}", prefix, msg_base.to_uppercase());
        quote! { #msg_ident.to_string() }
    };

    quote! {
        if #condition {
            let mut placeholders: ::ironforge::error::validation_error::Placeholders = Vec::new();
            #(#placeholder_pushes)*
            msgs.push((#msg_expr, placeholders));
        }
    }
}
