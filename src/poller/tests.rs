use std::collections::HashMap;

use color_eyre::eyre::Result;
use reqwest::Client;
use sqlx::PgPool;
use uuid::Uuid;

use crate::poller::Poller;

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

    let origins: HashMap<String, i16> =
        crate::persistence::fetch_origins_with_most_recent_success_metrics(&pool)
            .await?
            .into_iter()
            .map(|o| (o.uri, o.status))
            .collect();

    assert_eq!(origins.get(&uri).copied(), Some(200));

    Ok(())
}
