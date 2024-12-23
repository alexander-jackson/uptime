use color_eyre::eyre::Result;
use reqwest::Client;
use sqlx::PgPool;
use uuid::Uuid;

use crate::poller::{FailureReason, Poller};

async fn fetch_latest_query_status(pool: &PgPool, uri: &str) -> Result<Option<u16>> {
    let successes =
        crate::persistence::fetch_origins_with_most_recent_success_metrics(&pool).await?;

    let status = successes
        .into_iter()
        .filter_map(|r| (r.uri == uri).then_some(r.status as u16))
        .next();

    Ok(status)
}

async fn fetch_latest_query_failure(pool: &PgPool, uri: &str) -> Result<Option<String>> {
    let failures =
        crate::persistence::fetch_origins_with_most_recent_failure_metrics(&pool).await?;

    let failure_reason = failures
        .into_iter()
        .filter_map(|r| (r.uri == uri).then_some(r.failure_reason))
        .next();

    Ok(failure_reason)
}

#[sqlx::test]
async fn can_query_all_origins(pool: PgPool) -> Result<()> {
    let mut server = mockito::Server::new_async().await;
    let uri = server.url();

    let client = Client::new();
    let poller = Poller::new(pool.clone(), client);

    let origin_uid = Uuid::new_v4();
    crate::persistence::insert_origin(&pool, origin_uid, &uri).await?;

    let mock = server
        .mock("GET", "/")
        .with_status(200)
        .create_async()
        .await;

    poller.query_all_origins().await?;

    mock.assert_async().await;

    let status = fetch_latest_query_status(&pool, &uri).await?;

    assert_eq!(status, Some(200));

    Ok(())
}

#[sqlx::test]
async fn can_record_client_failures(pool: PgPool) -> Result<()> {
    let mut server = mockito::Server::new_async().await;
    let uri = server.url();

    let client = Client::new();
    let poller = Poller::new(pool.clone(), client);

    let origin_uid = Uuid::new_v4();
    crate::persistence::insert_origin(&pool, origin_uid, &uri).await?;

    let mock = server
        .mock("GET", "/")
        .with_status(404)
        .create_async()
        .await;

    poller.query_all_origins().await?;

    mock.assert_async().await;

    let status = fetch_latest_query_status(&pool, &uri).await?;

    assert_eq!(status, Some(404));

    Ok(())
}

#[sqlx::test]
async fn can_record_query_failures(pool: PgPool) -> Result<()> {
    // intentionally invalid TLD
    let uri = "https://mozilla.rust";

    let client = Client::new();
    let poller = Poller::new(pool.clone(), client);

    let origin_uid = Uuid::new_v4();
    crate::persistence::insert_origin(&pool, origin_uid, uri).await?;

    poller.query_all_origins().await?;

    let failure_reason = fetch_latest_query_failure(&pool, uri).await?;

    assert_eq!(
        failure_reason.as_deref(),
        Some(FailureReason::BadRequest.as_str())
    );

    Ok(())
}
