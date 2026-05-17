use crate::error::HttpError;
use crate::error::http_error::Empty;
use crate::log;
use crate::server::router::{Controller, Middleware, RouteMap, Router};
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
}

impl RouterBuilder {
    pub fn new(name: impl Into<String>) -> Self {
        return Self {
            name: name.into(),
            paths: Vec::new(),
            prefixes: Vec::new(),
            middlewares: Vec::new(),
        };
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
            panic!("invalid wildcard in '{}' — wildcard must be at the end as /*", p);
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

    pub fn make_map(&self) -> Result<AHashMap<String, RouteMap>, HttpError> {
        let mut map = AHashMap::new();

        for path in &self.paths {
            if map.contains_key(&path.name) {
                let msg = format!("duplicate route name '{}'", path.name); // TODO: msg
                log::warning(&msg, None);
                return Err(HttpError::conflict(msg, Empty));
            }

            map.insert(
                path.name.clone(),
                RouteMap {
                    path: path.path.clone(),
                    params: path.params.clone(),
                },
            );
        }

        return Ok(map);
    }

    pub fn make_router(&self) -> Result<Router, HttpError> {
        // TODO: proceso de construir el router
        let mut router = Router::new(self.name.clone());
        router.map = self.make_map()?;
        return Ok(router);
    }

    pub fn build(&self) -> Result<Router, HttpError> {
        return self.make_router();
    }
}
