use std::ops::DerefMut;

use color_eyre::eyre::Result;
use serde::Serialize;
use sqlx::types::chrono::{DateTime, Utc};
use sqlx::{PgPool, Postgres};
use sqlx_bootstrap::{ApplicationConfig, BootstrapConfig, ConnectionConfig, RootConfig};
use uuid::Uuid;

use crate::poller::FailureReason;
use crate::utils::get_env_var;

type Transaction = sqlx::Transaction<'static, Postgres>;

pub async fn bootstrap() -> Result<PgPool> {
    let root_username = get_env_var("ROOT_USERNAME")?;
    let root_password = get_env_var("ROOT_PASSWORD")?;
    let root_database = get_env_var("ROOT_DATABASE")?;

    let app_username = get_env_var("APP_USERNAME")?;
    let app_password = get_env_var("APP_PASSWORD")?;
    let app_database = get_env_var("APP_DATABASE")?;

    let host = get_env_var("DATABASE_HOST")?;
    let port = get_env_var("DATABASE_PORT")?.parse()?;

    let root_config = RootConfig::new(&root_username, &root_password, &root_database);
    let app_config = ApplicationConfig::new(&app_username, &app_password, &app_database);
    let conn_config = ConnectionConfig::new(&host, port);

    let config = BootstrapConfig::new(root_config, app_config, conn_config);
    let pool = config.bootstrap().await?;

    sqlx::migrate!().run(&pool).await?;

    Ok(pool)
}

#[derive(Serialize)]
pub struct Origin {
    pub origin_uid: Uuid,
    pub uri: String,
}

pub async fn insert_origin(pool: &PgPool, origin_uid: Uuid, uri: &str) -> Result<()> {
    sqlx::query!(
        r#"
            INSERT INTO origin (origin_uid, uri)
            VALUES ($1, $2)
        "#,
        origin_uid,
        uri,
    )
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn fetch_origins(pool: &PgPool) -> Result<Vec<Origin>> {
    let origins = sqlx::query_as!(
        Origin,
        r#"
            SELECT origin_uid, uri
            FROM origin
        "#
    )
    .fetch_all(pool)
    .await?;

    Ok(origins)
}

pub struct IndexOrigin {
    pub uri: String,
    pub status: i16,
    pub latency_millis: i64,
    pub queried_at: DateTime<Utc>,
}

pub async fn fetch_origins_with_most_recent_success_metrics(
    pool: &PgPool,
) -> Result<Vec<IndexOrigin>> {
    let origins = sqlx::query_as!(
        IndexOrigin,
        r#"
            SELECT DISTINCT ON (o.uri)
                o.uri,
                q.status,
                q.latency_millis,
                q.queried_at
            FROM origin o
            JOIN query q ON o.id = q.origin_id
            ORDER BY o.uri, q.queried_at DESC
        "#
    )
    .fetch_all(pool)
    .await?;

    Ok(origins)
}

pub struct OriginFailure {
    pub uri: String,
    pub failure_reason: String,
    pub queried_at: DateTime<Utc>,
}

pub async fn fetch_origins_with_most_recent_failure_metrics(
    pool: &PgPool,
) -> Result<Vec<OriginFailure>> {
    let origins = sqlx::query_as!(
        OriginFailure,
        r#"
            SELECT DISTINCT ON (o.uri)
                o.uri,
                qfr.name AS failure_reason,
                qf.queried_at
            FROM origin o
            JOIN query_failure qf ON o.id = qf.origin_id
            JOIN query_failure_reason qfr ON qfr.id = qf.failure_reason_id
            ORDER BY o.uri, qf.queried_at DESC
        "#
    )
    .fetch_all(pool)
    .await?;

    Ok(origins)
}

pub async fn insert_query(
    tx: &mut Transaction,
    origin_uid: Uuid,
    status: u16,
    latency_millis: i64,
    queried_at: DateTime<Utc>,
) -> Result<Uuid> {
    let query_uid = Uuid::new_v4();

    sqlx::query!(
        r#"
            INSERT INTO query (query_uid, origin_id, status, latency_millis, queried_at)
            VALUES (
                $1,
                (SELECT id FROM origin WHERE origin_uid = $2),
                $3,
                $4,
                $5
            )
        "#,
        query_uid,
        origin_uid,
        status as i16,
        latency_millis,
        queried_at
    )
    .execute(tx.deref_mut())
    .await?;

    Ok(query_uid)
}

pub async fn insert_query_failure(
    tx: &mut Transaction,
    origin_uid: Uuid,
    failure_reason: FailureReason,
    queried_at: DateTime<Utc>,
) -> Result<Uuid> {
    let query_failure_uid = Uuid::new_v4();

    sqlx::query!(
        r#"
            INSERT INTO query_failure (query_failure_uid, origin_id, failure_reason_id, queried_at)
            VALUES (
                $1,
                (SELECT id FROM origin WHERE origin_uid = $2),
                (SELECT id FROM query_failure_reason WHERE name = $3),
                $4
            )
        "#,
        query_failure_uid,
        origin_uid,
        failure_reason.as_str(),
        queried_at
    )
    .execute(tx.deref_mut())
    .await?;

    Ok(query_failure_uid)
}
