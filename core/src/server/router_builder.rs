use std::sync::Arc;

use crate::error::ForgeError;
use crate::server::router::{BoxFuture, Controller, Middleware, Router, Segment, default_not_found};
use crate::{Context, log};
use ahash::AHashMap;

pub trait AsyncControllerFn<'a> {
    type Fut: Future<Output = Result<(), ForgeError>> + Send + 'a;
    fn call(&self, c: &'a mut Context) -> Self::Fut;
}

impl<'a, F, Fut> AsyncControllerFn<'a> for F
where
    F: Fn(&'a mut Context) -> Fut,
    Fut: Future<Output = Result<(), ForgeError>> + Send + 'a,
{
    type Fut = Fut;
    fn call(&self, c: &'a mut Context) -> Self::Fut {
        self(c)
    }
}

pub trait IntoController {
    fn into_controller(self) -> Controller;
}

impl<F> IntoController for F
where
    F: for<'a> AsyncControllerFn<'a> + Clone + Send + Sync + 'static,
{
    fn into_controller(self) -> Controller {
        Arc::new(move |c: &mut Context| -> BoxFuture<'_> {
            let f = self.clone();
            Box::pin(async move { f.call(c).await })
        })
    }
}

pub struct Path {
    method: String,
    path: String,
    name: String,
    params: Vec<String>,
    is_dinamic: bool,
    is_wildcard: bool,
    controller: Controller,
    middlewares: Vec<Middleware>,
}

impl Path {
    pub fn new(
        method: impl Into<String>,
        path: impl Into<String>,
        name: impl Into<String>,
        params: Vec<String>,
        is_dinamic: bool,
        is_wildcard: bool,
        controller: Controller,
        middlewares: Vec<Middleware>,
    ) -> Self {
        return Self {
            method: method.into(),
            path: path.into(),
            name: name.into(),
            params: params,
            is_dinamic,
            is_wildcard,
            controller,
            middlewares,
        };
    }

    pub fn name(&mut self, name: impl Into<String>) -> &mut Self {
        self.name = name.into().to_lowercase();
        return self;
    }

    pub fn middleware(&mut self, middleware: Middleware) -> &mut Self {
        self.middlewares.push(middleware);
        return self;
    }

    pub fn middlewares(&mut self, middlewares: Vec<Middleware>) -> &mut Self {
        self.middlewares.extend(middlewares);
        return self;
    }
}

pub struct RouterBuilder {
    pub name: String,
    pub paths: Vec<Path>,
    pub prefixes: Vec<String>,
    pub middlewares: Vec<Middleware>,
    pub not_found: Controller,
}

impl RouterBuilder {
    /// Crea un nuevo RouterBuilder con el nombre dado. El nombre se usa para identificar el router y para generar nombres de rutas.
    pub fn new(name: impl Into<String>) -> Self {
        return Self {
            name: name.into(),
            paths: Vec::new(),
            prefixes: Vec::new(),
            middlewares: Vec::new(),
            not_found: Arc::new(|c| Box::pin(default_not_found(c))),
        };
    }

    pub fn get(&mut self, path: impl Into<String>, controller: impl IntoController) -> &mut Self {
        self.add_path("GET".to_string(), path.into(), controller);
        return self;
    }

    pub fn post(&mut self, path: impl Into<String>, controller: impl IntoController) -> &mut Self {
        self.add_path("POST".to_string(), path.into(), controller);
        return self;
    }

    pub fn put(&mut self, path: impl Into<String>, controller: impl IntoController) -> &mut Self {
        self.add_path("PUT".to_string(), path.into(), controller);
        return self;
    }

    pub fn delete(&mut self, path: impl Into<String>, controller: impl IntoController) -> &mut Self {
        self.add_path("DELETE".to_string(), path.into(), controller);
        return self;
    }

    pub fn patch(&mut self, path: impl Into<String>, controller: impl IntoController) -> &mut Self {
        self.add_path("PATCH".to_string(), path.into(), controller);
        return self;
    }

    pub fn options(&mut self, path: impl Into<String>, controller: impl IntoController) -> &mut Self {
        self.add_path("OPTIONS".to_string(), path.into(), controller);
        return self;
    }

    pub fn head(&mut self, path: impl Into<String>, controller: impl IntoController) -> &mut Self {
        self.add_path("HEAD".to_string(), path.into(), controller);
        return self;
    }

    /// no es comun pero posi acaso aca esta
    pub fn trace(&mut self, path: impl Into<String>, controller: impl IntoController) -> &mut Self {
        self.add_path("TRACE".to_string(), path.into(), controller);
        return self;
    }

    /// no es comun pero posi acaso aca esta
    pub fn connect(&mut self, path: impl Into<String>, controller: impl IntoController) -> &mut Self {
        self.add_path("CONNECT".to_string(), path.into(), controller);
        return self;
    }

    // pub fn any(&mut self, path: impl Into<String>, controller: impl IntoController) -> &mut Self {
    //     self.add_path("ANY".to_string(), path.into(), controller);
    //     return self;
    // }

    /// Crea un grupo de rutas envueltas en uno o varios prefijos
    pub fn prefix(&mut self, prefix: impl Into<String>, f: impl FnOnce(&mut Self)) -> &mut Self {
        let p: Vec<String> = prefix
            .into()
            .trim()
            .trim_matches('/')
            .split('/')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        let lp = p.len();
        self.prefixes.extend(p);
        f(self); // ejecuta la closure
        for _ in 0..lp {
            self.prefixes.pop();
        }

        return self;
    }

    /// Crea un grupo de rutas envueltas en un middleware
    pub fn middleware(&mut self, middleware: impl IntoController, f: impl FnOnce(&mut Self)) -> &mut Self {
        self.middlewares.push(middleware.into_controller());
        f(self); // ejecuta la closure
        self.middlewares.pop();
        return self;
    }

    /// Crea un grupo de rutas envueltas en varios middlewares
    pub fn middlewares(&mut self, middlewares: Vec<impl IntoController>, f: impl FnOnce(&mut Self)) -> &mut Self {
        let lm = middlewares.len();
        for m in middlewares {
            self.middlewares.push(m.into_controller());
        }
        f(self);
        for _ in 0..lm {
            self.middlewares.pop();
        }
        return self;
    }

    /// crea un grupo de rutas envueltas en un prefijo y un middleware
    pub fn group(
        &mut self,
        prefix: impl Into<String>,
        middleware: impl IntoController,
        f: impl FnOnce(&mut Self),
    ) -> &mut Self {
        let p: Vec<String> = prefix
            .into()
            .trim()
            .trim_matches('/')
            .split('/')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        let lp = p.len();
        self.prefixes.extend(p);
        self.middlewares.push(middleware.into_controller());
        f(self);
        for _ in 0..lp {
            self.prefixes.pop();
        }
        self.middlewares.pop();
        return self;
    }

    /// crea un grupo de rutas envueltas en un prefijo y varios middlewares
    pub fn groups(
        &mut self,
        prefix: impl Into<String>,
        middlewares: Vec<impl IntoController>,
        f: impl FnOnce(&mut Self),
    ) -> &mut Self {
        let p: Vec<String> = prefix
            .into()
            .trim()
            .trim_matches('/')
            .split('/')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        let lp = p.len();
        let lm = middlewares.len();

        self.prefixes.extend(p);
        for m in middlewares {
            self.middlewares.push(m.into_controller());
        }

        f(self);

        for _ in 0..lp {
            self.prefixes.pop();
        }
        for _ in 0..lm {
            self.middlewares.pop();
        }
        return self;
    }

    /// Configura el controller que se usará cuando no matchee ninguna
    /// ruta. Si nunca se llama, queda el default_not_found.
    pub fn set_not_found(&mut self, controller: impl IntoController) -> &mut Self {
        self.not_found = controller.into_controller();
        return self;
    }

    /// Construye el router con las rutas y middlewares configurados.
    pub fn build(&self) -> Result<Router, ForgeError> {
        let mut router = Router::new(self.name.clone(), self.not_found.clone());
        router.map = self.make_map()?;
        router.static_routes = self.make_static_routes();
        router.dinamic_routes = self.make_dinamic_routes(self.not_found.clone())?;
        return Ok(router);
    }

    fn add_path<T: IntoController>(&mut self, method: String, path: String, controller: T) {
        let controller_name = std::any::type_name::<T>()
            .rsplit("::")
            .next()
            .unwrap_or("unknown")
            .to_string();

        let mut p = format!("{}/{}", self.prefixes.join("/"), path)
            .trim()
            .trim_matches('/')
            .to_lowercase();

        while p.contains("//") {
            p = p.replace("//", "/");
        }

        if p.contains('*') && !p.ends_with("/*") {
            let msg = format!("invalid wildcard in '{}' — wildcard must be at the end as /*", p);
            log::critical(&msg, None);
            panic!("{}", msg);
        }

        let parts: Vec<&str> = p.split('/').collect();
        let mut params: Vec<String> = Vec::new();
        let mut name_parts: Vec<String> = Vec::new();
        let mut is_wildcard: bool = false;

        for part in parts {
            if part.starts_with('{') && part.ends_with('}') {
                params.push(part[1..part.len() - 1].to_string());
            } else if part.starts_with(':') {
                params.push(part[1..part.len()].to_string());
            } else if part.ends_with("*") {
                is_wildcard = true;
            } else {
                name_parts.push(part.to_string());
            }
        }
        let is_dinamic: bool = params.len() > 0;

        let name = format!("{}.{}", name_parts.join("."), controller_name);
        self.paths.push(Path::new(
            method,
            p,
            name,
            params,
            is_dinamic,
            is_wildcard,
            controller.into_controller(),
            self.middlewares.clone(),
        ));
    }

    fn make_static_routes(&self) -> AHashMap<String, Segment> {
        let mut map = AHashMap::new();
        for p in &self.paths {
            if p.is_dinamic || p.is_wildcard {
                continue;
            }
            let key = format!("{}/{}", p.path, p.method);
            map.insert(
                key,
                Segment {
                    // controller: p.controller.clone(),
                    // middlewares: p.middlewares.clone(),
                    handlers: {
                        let mut h = p.middlewares.clone();
                        h.push(p.controller.clone());
                        h
                    },
                    params: Vec::new(),
                    static_routes: AHashMap::new(),
                    dinamic_routes: None,
                    is_dinamic: false,
                    is_wildcard: false,
                },
            );
        }
        return map;
    }

    fn make_dinamic_routes(&self, not_found: Controller) -> Result<Segment, ForgeError> {
        let mut route = Segment::new(not_found.clone());
        for p in &self.paths {
            if !p.is_dinamic && !p.is_wildcard {
                continue;
            }

            let mut node: &mut Segment = &mut route;
            let mut parts: Vec<&str> = p.path.split('/').filter(|s| *s != "*").collect();
            parts.push(p.method.as_str());
            let len = parts.len();

            for (i, part) in parts.iter().enumerate() {
                let is_last = i == len - 1;
                if (part.starts_with('{') && part.ends_with('}')) || part.starts_with(':') {
                    if node.dinamic_routes.is_none() {
                        node.dinamic_routes = Some(Box::new(Segment::new(not_found.clone())));
                    }
                    node = match node.dinamic_routes.as_mut() {
                        Some(n) => n,
                        None => {
                            let msg = format!("dynamic route [{}] node is None", p.path.clone());
                            log::critical(&msg, None);
                            return Err(ForgeError::internal().message(msg));
                        }
                    };
                } else {
                    if !node.static_routes.contains_key(&part.to_string()) {
                        node.static_routes
                            .insert(part.to_string(), Segment::new(not_found.clone()));
                    }
                    node = match node.static_routes.get_mut(&part.to_string()) {
                        Some(n) => n,
                        None => {
                            let msg = format!("static route [{}] node is None", p.path.clone());
                            log::critical(&msg, None);
                            return Err(ForgeError::internal().message(msg));
                        }
                    };
                }

                if is_last {
                    // node.controller = p.controller.clone();
                    // node.middlewares = p.middlewares.clone();
                    node.handlers = {
                        let mut h = p.middlewares.clone();
                        h.push(p.controller.clone());
                        h
                    };
                    node.params = p.params.clone();
                    node.is_dinamic = p.is_dinamic;
                    node.is_wildcard = p.is_wildcard;
                }
            }
        }
        return Ok(route);
    }

    /// Genera el mapa de nombres de ruta -> "METHOD:/path/con/:params"
    /// en un solo string, ej: "GET:/api/users/:id/show"
    fn make_map(&self) -> Result<Arc<AHashMap<String, String>>, ForgeError> {
        let mut map = AHashMap::new();

        for path in &self.paths {
            if map.contains_key(&path.name) {
                let msg = format!("duplicate route name '{}'", path.name); // TODO: msg
                log::warning(&msg, None);
                return Err(ForgeError::conflict().message(msg));
            }

            let normalized_path = path
                .path
                .split('/')
                .map(|part| {
                    if (part.starts_with('{') && part.ends_with('}')) || part.starts_with(':') {
                        let name = part
                            .trim_start_matches(':')
                            .trim_start_matches('{')
                            .trim_end_matches('}');
                        format!(":{}", name)
                    } else {
                        part.to_string()
                    }
                })
                .collect::<Vec<String>>()
                .join("/");

            map.insert(path.name.clone(), format!("{}:/{}", path.method, normalized_path));
        }

        return Ok(Arc::new(map));
    }
}
