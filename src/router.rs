use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::persistence::Origin;

#[derive(Clone)]
struct ApplicationState {
    pool: PgPool,
}

pub fn build(pool: PgPool) -> Router {
    let state = ApplicationState { pool };

    Router::new()
        .route("/origins", get(get_origins).put(put_origin))
        .with_state(state)
}

async fn get_origins(
    State(ApplicationState { pool }): State<ApplicationState>,
) -> Json<Vec<Origin>> {
    let origins = crate::persistence::fetch_origins(&pool)
        .await
        .expect("failed to fetch origins");

    Json(origins)
}

#[derive(Debug, Deserialize)]
struct OriginCreationRequest {
    uri: String,
}

async fn put_origin(
    State(ApplicationState { pool }): State<ApplicationState>,
    request: Json<OriginCreationRequest>,
) -> Json<Uuid> {
    let origin_uid = Uuid::new_v4();
    let origin = Origin {
        origin_uid,
        uri: request.uri.clone(),
    };

    crate::persistence::insert_origin(&pool, &origin)
        .await
        .expect("failed to create origin");

    Json(origin_uid)
}
