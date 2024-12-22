use std::time::Duration;

use axum::extract::State;
use axum::response::Redirect;
use axum::routing::get;
use axum::{Form, Router};
use chrono::Utc;
use color_eyre::eyre::Result;
use humantime::format_duration;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use tower_http::services::ServeDir;
use uuid::Uuid;

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
        .route("/add-origin", get(add_origin_template).post(add_origin))
        .nest_service("/assets", ServeDir::new("assets"))
        .with_state(state);

    Ok(router)
}

#[derive(Serialize)]
struct IndexOrigin {
    uri: String,
    status: u16,
    latency_millis: u64,
    queried: String,
}

#[derive(Serialize)]
struct OriginFailure {
    uri: String,
    failure_reason: String,
    queried: String,
}

#[derive(Serialize)]
struct IndexContext {
    origins: Vec<IndexOrigin>,
    failing_origins: Vec<OriginFailure>,
}

async fn index(
    State(ApplicationState {
        pool,
        template_engine,
    }): State<ApplicationState>,
) -> RenderedTemplate {
    let origins = crate::persistence::fetch_origins_with_most_recent_success_metrics(&pool)
        .await
        .expect("failed to fetch origins")
        .into_iter()
        .map(|origin| {
            let delta = (Utc::now() - origin.queried_at).abs();
            let duration = Duration::from_millis(delta.num_milliseconds() as u64);

            IndexOrigin {
                uri: origin.uri,
                status: origin.status as u16,
                latency_millis: origin.latency_millis as u64,
                queried: format_duration(duration).to_string(),
            }
        })
        .collect();

    let failing_origins = crate::persistence::fetch_origins_with_most_recent_failure_metrics(&pool)
        .await
        .expect("failed to fetch failing origins")
        .into_iter()
        .map(|origin| {
            let delta = (Utc::now() - origin.queried_at).abs();
            let duration = Duration::from_millis(delta.num_milliseconds() as u64);

            OriginFailure {
                uri: origin.uri,
                failure_reason: origin.failure_reason,
                queried: format_duration(duration).to_string(),
            }
        })
        .collect();

    let context = IndexContext {
        origins,
        failing_origins,
    };

    template_engine
        .render_serialized("index.tera.html", &context)
        .expect("failed to render template")
}

async fn add_origin_template(
    State(ApplicationState {
        template_engine, ..
    }): State<ApplicationState>,
) -> RenderedTemplate {
    template_engine
        .render_contextless("add-origin.tera.html")
        .expect("failed to render template")
}

#[derive(Deserialize)]
struct OriginCreationRequest {
    uri: String,
}

async fn add_origin(
    State(ApplicationState { pool, .. }): State<ApplicationState>,
    Form(OriginCreationRequest { uri }): Form<OriginCreationRequest>,
) -> Redirect {
    let origin_uid = Uuid::new_v4();

    crate::persistence::insert_origin(&pool, origin_uid, &uri)
        .await
        .expect("failed to insert origin");

    Redirect::to("/")
}
