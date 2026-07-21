use std::sync::Arc;

use crate::error::HttpError;
use crate::log;
use crate::server::router::{Controller, Middleware, Router, Segment, default_not_found};
use ahash::AHashMap;

pub struct Path {
    pub method: String,
    pub path: String,
    pub name: String,
    pub params: Vec<String>,
    pub is_dinamic: bool,
    pub is_wildcard: bool,
    pub controller_name: String,
    pub controller: Controller,
    pub middlewares: Vec<Middleware>,
}

impl Path {
    pub fn new(
        method: impl Into<String>,
        path: impl Into<String>,
        name: impl Into<String>,
        params: Vec<String>,
        is_dinamic: bool,
        is_wildcard: bool,
        controller_name: impl Into<String>,
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
            controller_name: controller_name.into(),
            controller,
            middlewares,
        };
    }

    pub fn name(&mut self, name: impl Into<String>) -> &mut Self {
        self.name = name.into().to_lowercase();
        return self;
    }

    pub fn use_middleware(&mut self, middleware: Middleware) -> &mut Self {
        self.middlewares.push(middleware);
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
    pub fn new(name: impl Into<String>) -> Self {
        return Self {
            name: name.into(),
            paths: Vec::new(),
            prefixes: Vec::new(),
            middlewares: Vec::new(),
            not_found: Arc::new(|c| Box::pin(default_not_found(c))),
        };
    }

    /// Configura el controller que se usará cuando no matchee ninguna
    /// ruta. Si nunca se llama, queda el default_not_found.
    pub fn not_found(&mut self, controller: Controller) -> &mut Self {
        self.not_found = controller;
        return self;
    }

    pub fn add_path(&mut self, method: String, path: String, controller_name: String, controller: Controller) {
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
            controller_name,
            controller,
            self.middlewares.clone(),
        ));
    }

    pub fn make_router(&self) -> Result<Router, HttpError> {
        let mut router = Router::new(self.name.clone(), self.not_found.clone());
        router.map = self.make_map()?;
        router.static_routes = self.make_static_routes();
        router.dinamic_routes = self.make_dinamic_routes(self.not_found.clone())?;
        return Ok(router);
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
                    controller: p.controller.clone(),
                    middlewares: p.middlewares.clone(),
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

    fn make_dinamic_routes(&self, not_found: Controller) -> Result<Segment, HttpError> {
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
                            return Err(HttpError::internal_server_error(msg));
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
                            return Err(HttpError::internal_server_error(msg));
                        }
                    };
                }

                if is_last {
                    node.controller = p.controller.clone();
                    node.middlewares = p.middlewares.clone();
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
    fn make_map(&self) -> Result<Arc<AHashMap<String, String>>, HttpError> {
        let mut map = AHashMap::new();

        for path in &self.paths {
            if map.contains_key(&path.name) {
                let msg = format!("duplicate route name '{}'", path.name); // TODO: msg
                log::warning(&msg, None);
                return Err(HttpError::conflict(msg));
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

    pub fn build(&self) -> Result<Router, HttpError> {
        return self.make_router();
    }
}
