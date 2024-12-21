use axum::extract::State;
use axum::routing::get;
use axum::Router;
use color_eyre::eyre::Result;
use serde::Serialize;
use sqlx::PgPool;
use tower_http::services::ServeDir;

use crate::persistence::IndexOrigin;
use crate::templates::{RenderedTemplate, TemplateEngine};

#[derive(Clone)]
struct ApplicationState {
    pool: PgPool,
    template_engine: TemplateEngine,
}

pub fn build(pool: PgPool) -> Result<Router> {
    let template_engine = TemplateEngine::new()?;
    let state = ApplicationState {
        pool,
        template_engine,
    };

    let router = Router::new()
        .route("/", get(index))
        .nest_service("/assets", ServeDir::new("assets"))
        .with_state(state);

    Ok(router)
}

#[derive(Serialize)]
struct IndexContext {
    origins: Vec<IndexOrigin>,
}

async fn index(
    State(ApplicationState {
        pool,
        template_engine,
    }): State<ApplicationState>,
) -> RenderedTemplate {
    let origins = crate::persistence::fetch_origins_with_most_recent_success_metrics(&pool)
        .await
        .expect("failed to fetch origins");

    let context = IndexContext { origins };

    template_engine
        .render_serialized("index.tera.html", &context)
        .expect("failed to render template")
}
