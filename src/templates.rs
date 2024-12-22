use axum::body::Body;
use axum::response::{IntoResponse, Response};
use color_eyre::eyre::Result;
use reqwest::header::{CACHE_CONTROL, CONTENT_TYPE};
use reqwest::StatusCode;
use serde::Serialize;
use tera::{Context, Tera};

#[derive(Clone)]
pub struct TemplateEngine {
    inner: Tera,
}

impl TemplateEngine {
    pub fn new() -> Result<Self> {
        let inner = Tera::new("templates/**.tera.html")?;

        Ok(Self { inner })
    }

    pub fn render_serialized<C: Serialize>(
        &self,
        template: &str,
        context: &C,
    ) -> Result<RenderedTemplate> {
        let context = Context::from_serialize(context)?;
        let rendered = self.inner.render(template, &context)?;

        Ok(RenderedTemplate { inner: rendered })
    }
}

pub struct RenderedTemplate {
    inner: String,
}

impl IntoResponse for RenderedTemplate {
    fn into_response(self) -> Response {
        Response::builder()
            .status(StatusCode::OK)
            .header(CONTENT_TYPE, "text/html")
            .header(CACHE_CONTROL, "no-store")
            .body(Body::from(self.inner))
            .unwrap()
    }
}
