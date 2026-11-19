use hyper::body::Incoming;
use hyper::{Request, Response};
use std::convert::Infallible;

use crate::handler::ResBody;
use crate::handler::context::Context;
use crate::log;
use crate::server::router::{Param, Params, Router, Segment};

/// Nombre del parámetro con el resto de la URL en las rutas `/*`.
/// Ej. `r.get("/resources/*", ...)` con `/resources/js/auth.js` → `c.get_param("*") == Some("js/auth.js")`.
pub const WILDCARD_PARAM: &str = "*";

pub async fn dispatch(req: Request<Incoming>, router: &'static Router) -> Result<Response<ResBody>, Infallible> {
    let mut c = Context::new(req);
    let method = c.r.method().to_string();
    let path = c.r.uri().path().to_string();

    let handlers = match resolve(router, &method, &path) {
        Some((segment, params)) => {
            c.params = params;
            &segment.handlers
        }
        None => std::slice::from_ref(&router.not_found_controller),
    };

    for handler in handlers {
        if let Err(e) = handler(&mut c).await {
            if let Err(ex) = c.response_error(e) {
                let exv = serde_json::to_value(&ex).ok();
                log::error("Failed to send error", exv);
            }
            break;
        }
    }

    Ok(c.into_response())
}

/// Busca la ruta de `method` + `path`. Los tramos fijos se comparan sin mayúsculas;
/// los valores de `{param}` y del `*` conservan el texto original de la URL.
///
/// Orden: ruta estática exacta → árbol (tramo fijo antes que `{param}`) →
/// la ruta `/*` más profunda que haya cubierto el camino.
pub(crate) fn resolve<'a>(router: &'a Router, method: &str, path: &str) -> Option<(&'a Segment, Params)> {
    let path = path.trim_start_matches('/');

    let key = format!("{}/{}", path.to_lowercase(), method);
    if let Some(segment) = router.static_routes.get(&key) {
        return Some((segment, Vec::new()));
    }

    let parts: Vec<&str> = if path.is_empty() { Vec::new() } else { path.split('/').collect() };
    let mut node = &router.dinamic_routes;
    let mut values: Vec<&str> = Vec::new();
    // (segmento `/*`, cuántos valores de `{param}` había, desde qué tramo empieza el resto)
    let mut wildcard: Option<(&Segment, usize, usize)> = None;
    let mut matched = true;

    for (i, part) in parts.iter().enumerate() {
        if let Some(w) = node.static_routes.get(method).filter(|s| s.is_wildcard) {
            wildcard = Some((w, values.len(), i));
        }
        if let Some(next) = node.static_routes.get(&part.to_lowercase()) {
            node = next;
        } else if let Some(next) = node.dinamic_routes.as_deref() {
            values.push(part);
            node = next;
        } else {
            matched = false;
            break;
        }
    }

    if matched {
        if let Some(end) = node.static_routes.get(method).filter(|s| s.is_dinamic || s.is_wildcard) {
            // Un `/*` también responde a su prefijo sin nada después (resto vacío).
            let rest = end.is_wildcard.then_some("");
            return Some((end, params(end, &values, rest)));
        }
    }

    let (w, n, from) = wildcard?;
    Some((w, params(w, &values[..n], Some(&parts[from..].join("/")))))
}

fn params(segment: &Segment, values: &[&str], rest: Option<&str>) -> Params {
    let mut out: Params = segment
        .params
        .iter()
        .zip(values)
        .map(|(name, value)| Param { name: name.clone(), value: value.to_string() })
        .collect();
    if let Some(rest) = rest {
        out.push(Param { name: WILDCARD_PARAM.into(), value: rest.to_string() });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Context, ForgeError};

    async fn ok(_c: &mut Context) -> Result<(), ForgeError> {
        Ok(())
    }
    async fn other(_c: &mut Context) -> Result<(), ForgeError> {
        Ok(())
    }
    async fn mw(_c: &mut Context) -> Result<(), ForgeError> {
        Ok(())
    }

    fn router() -> Router {
        Router::build("t", |r| {
            r.get("/", ok);
            r.get("/api/users/me", ok);
            r.delete("/api/users/sessions/{id}", ok);
            r.get("/api/posts/{post}/comments/{comment}", ok);
            r.get("/api/posts/latest", other);
            r.middleware(mw, |r| {
                r.get("/resources/*", ok);
            });
            r.get("/resources/fixed/*", other);
        })
    }

    fn find(r: &Router, method: &str, path: &str) -> Option<(usize, Vec<(String, String)>)> {
        resolve(r, method, path).map(|(s, p)| (s.handlers.len(), p.into_iter().map(|p| (p.name, p.value)).collect()))
    }

    fn kv(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    #[test]
    fn estaticas() {
        let r = router();
        assert_eq!(find(&r, "GET", "/"), Some((1, vec![])));
        assert_eq!(find(&r, "GET", "/api/users/me"), Some((1, vec![])));
        assert_eq!(find(&r, "GET", "/API/Users/ME"), Some((1, vec![])));
        assert_eq!(find(&r, "POST", "/api/users/me"), None);
        assert_eq!(find(&r, "GET", "/api/users"), None);
    }

    #[test]
    fn llena_los_params_con_el_texto_original() {
        let r = router();
        assert_eq!(
            find(&r, "DELETE", "/api/users/sessions/65AbC0123"),
            Some((1, kv(&[("id", "65AbC0123")])))
        );
        assert_eq!(
            find(&r, "GET", "/api/posts/7/comments/9"),
            Some((1, kv(&[("post", "7"), ("comment", "9")])))
        );
        assert_eq!(find(&r, "GET", "/api/users/sessions/1"), None);
    }

    #[test]
    fn tramo_fijo_antes_que_param() {
        let r = router();
        assert_eq!(find(&r, "GET", "/api/posts/latest"), Some((1, vec![])));
    }

    #[test]
    fn comodin_toma_el_resto() {
        let r = router();
        // 2 handlers: el middleware del grupo + el controller.
        assert_eq!(find(&r, "GET", "/resources/js/Auth.js"), Some((2, kv(&[("*", "js/Auth.js")]))));
        assert_eq!(find(&r, "GET", "/resources/css/a/b/c.css"), Some((2, kv(&[("*", "css/a/b/c.css")]))));
        assert_eq!(find(&r, "GET", "/resources"), Some((2, kv(&[("*", "")]))));
        assert_eq!(find(&r, "POST", "/resources/js/a.js"), None);
    }

    #[test]
    fn gana_el_comodin_mas_profundo() {
        let r = router();
        assert_eq!(find(&r, "GET", "/resources/fixed/x.css"), Some((1, kv(&[("*", "x.css")]))));
        assert_eq!(find(&r, "GET", "/resources/fixedx.css"), Some((2, kv(&[("*", "fixedx.css")]))));
    }
}
