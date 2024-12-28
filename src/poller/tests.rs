use std::{collections::HashMap, sync::Arc, time::Duration};

use color_eyre::eyre::Result;
use sqlx::PgPool;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::poller::{AlertThreshold, FailureReason, Notifier, Poller, PollerConfiguration};

const SNS_TOPIC: &str = "some-sns-topic";

#[derive(Debug, Eq, PartialEq)]
struct Message {
    subject: String,
    message: String,
}

impl Message {
    fn new(subject: &str, message: &str) -> Self {
        Self {
            subject: subject.to_owned(),
            message: message.to_owned(),
        }
    }
}

#[derive(Clone, Debug, Default)]
struct MockSnsClient {
    sent_messages: Arc<RwLock<HashMap<String, Vec<Message>>>>,
}

impl Notifier for MockSnsClient {
    async fn notify(&self, topic: &str, subject: &str, message: &str) -> Result<()> {
        self.sent_messages
            .write()
            .await
            .entry(topic.to_owned())
            .or_default()
            .push(Message::new(subject, message));

        Ok(())
    }
}

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

fn create_poller(pool: &PgPool) -> Poller<MockSnsClient> {
    let http_client = reqwest::Client::new();
    let sns_client = MockSnsClient::default();
    let configuration = PollerConfiguration::new(AlertThreshold::default(), SNS_TOPIC);

    Poller::new(pool.clone(), http_client, sns_client.clone(), configuration)
}

#[sqlx::test]
async fn can_query_all_origins(pool: PgPool) -> Result<()> {
    let mut server = mockito::Server::new_async().await;
    let uri = server.url();

    let poller = create_poller(&pool);

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

    let poller = create_poller(&pool);

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

    let poller = create_poller(&pool);

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

#[sqlx::test]
async fn can_route_alerts_to_clients(pool: PgPool) -> Result<()> {
    // intentionally invalid TLD
    let uri = "https://mozilla.rust";

    let poller = create_poller(&pool);

    let origin_uid = Uuid::new_v4();
    crate::persistence::insert_origin(&pool, origin_uid, uri).await?;

    // Make 3 queries, all of which fail
    for _ in 0..3 {
        poller.query_all_origins().await?;
    }

    let map = poller.notifier.sent_messages.read().await;
    let messages = &map[SNS_TOPIC];

    let expected_message = format!("The failure rate of {uri} exceeds the SLA");

    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].subject, "Outage detected");
    assert_eq!(messages[0].message, expected_message);

    Ok(())
}

#[sqlx::test]
async fn alerts_are_not_constantly_routed(pool: PgPool) -> Result<()> {
    // intentionally invalid TLD
    let uri = "https://mozilla.rust";

    let poller = create_poller(&pool);

    let origin_uid = Uuid::new_v4();
    crate::persistence::insert_origin(&pool, origin_uid, uri).await?;

    // Make 3 queries, all of which fail to trigger an alert
    for _ in 0..3 {
        poller.query_all_origins().await?;
    }

    // Trigger another query which fails
    poller.query_all_origins().await?;

    // Check we only sent a single message
    let map = poller.notifier.sent_messages.read().await;

    assert_eq!(map[SNS_TOPIC].len(), 1);

    Ok(())
}

#[sqlx::test]
async fn alerts_can_cooldown_after_firing(pool: PgPool) -> Result<()> {
    // intentionally invalid TLD
    let uri = "https://mozilla.rust";

    let mut poller = create_poller(&pool);
    poller.configuration.alert_threshold.cooldown = chrono::Duration::milliseconds(100);

    let origin_uid = Uuid::new_v4();
    crate::persistence::insert_origin(&pool, origin_uid, uri).await?;

    // Make 3 queries, all of which fail to trigger an alert
    for _ in 0..3 {
        poller.query_all_origins().await?;
    }

    // Wait a bit for the cooldown
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Trigger another query which fails
    poller.query_all_origins().await?;

    // Check we sent 2 messages since the cooldown had passed
    let map = poller.notifier.sent_messages.read().await;

    assert_eq!(map[SNS_TOPIC].len(), 2);

    Ok(())
}
