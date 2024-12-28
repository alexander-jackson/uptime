use std::ops::DerefMut;

use chrono::Duration;
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

pub async fn failure_rate_exceeded(
    pool: &PgPool,
    origin_uid: Uuid,
    limit: u16,
    period: Duration,
) -> Result<bool> {
    let end = Utc::now();
    let start = end - period;

    let exceeded = sqlx::query_scalar!(
        r#"
            SELECT COUNT(*) >= $2
            FROM query_failure qf
            JOIN origin o ON o.id = qf.origin_id
            WHERE o.origin_uid = $1
            AND qf.queried_at BETWEEN $3 AND $4
        "#,
        origin_uid,
        limit as i32,
        start,
        end,
    )
    .fetch_one(pool)
    .await?
    .expect("Count returned a null value");

    Ok(exceeded)
}

pub async fn insert_notification(
    pool: &PgPool,
    origin_uid: Uuid,
    topic: &str,
    subject: &str,
    message: &str,
    created_at: DateTime<Utc>,
) -> Result<Uuid> {
    let notification_uid = Uuid::new_v4();

    sqlx::query!(
        r#"
            INSERT INTO notification (notification_uid, origin_id, topic, subject, message, created_at)
            VALUES (
                $1,
                (SELECT id FROM origin WHERE origin_uid = $2),
                $3,
                $4,
                $5,
                $6
            )
        "#,
        notification_uid,
        origin_uid,
        topic,
        subject,
        message,
        created_at
    )
    .execute(pool)
    .await?;

    Ok(notification_uid)
}

pub async fn latest_notification_older_than(
    pool: &PgPool,
    origin_uid: Uuid,
    cooldown: Duration,
) -> Result<bool> {
    let boundary = Utc::now() - cooldown;

    let notification = sqlx::query_scalar!(
        r#"
            SELECT NOT EXISTS (
                SELECT
                FROM notification n
                JOIN origin o ON o.id = n.origin_id
                WHERE o.origin_uid = $1
                AND n.created_at > $2
                LIMIT 1
            )
        "#,
        origin_uid,
        boundary,
    )
    .fetch_one(pool)
    .await?
    .expect("Exists returned a null value");

    Ok(notification)
}
