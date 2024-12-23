use color_eyre::eyre::Result;
use reqwest::Client;
use sqlx::PgPool;
use uuid::Uuid;

use crate::poller::Poller;

async fn fetch_latest_query_status(pool: &PgPool, uri: &str) -> Result<Option<u16>> {
    let successes =
        crate::persistence::fetch_origins_with_most_recent_success_metrics(&pool).await?;

    let status = successes
        .iter()
        .filter_map(|r| (r.uri == uri).then_some(r.status as u16))
        .next();

    Ok(status)
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
