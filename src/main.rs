use std::future::Future;
use std::ops::Deref;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use aws_config::BehaviorVersion;
use aws_sdk_s3::primitives::ByteStream;
use chrono::{DateTime, Utc};
use lambda_runtime::{Config, LambdaEvent, Service};
use reqwest::header::USER_AGENT;
use serde::{Deserialize, Serialize};

type LambdaResult<T> = std::result::Result<T, lambda_runtime::Error>;
type BoxFuture<O> = Pin<Box<dyn Future<Output = O> + Send>>;

#[derive(Debug, Deserialize)]
struct Payload {
    time: DateTime<Utc>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct State {
    most_recent_status: Option<u16>,
}

#[derive(Clone, Debug)]
struct Handler {
    target: Arc<str>,
    state_bucket: Arc<str>,
    state_key: Arc<str>,
    request_client: reqwest::Client,
    s3_client: aws_sdk_s3::Client,
}

impl Handler {
    async fn persist_state(&self, status: u16) -> LambdaResult<()> {
        let new_state = State {
            most_recent_status: Some(status),
        };

        let bytes = serde_json::to_vec(&new_state)?;

        self.s3_client
            .put_object()
            .bucket(self.state_bucket.deref())
            .key(self.state_key.deref())
            .body(ByteStream::from(bytes))
            .send()
            .await?;

        Ok(())
    }

    async fn handle(&self, event: LambdaEvent<Payload>) -> LambdaResult<()> {
        tracing::info!(time = %event.payload.time, "Received an event for the lambda invocation!");

        let Config {
            function_name,
            version,
            ..
        } = event.context.env_config.as_ref();

        let state_response = self
            .s3_client
            .get_object()
            .bucket(self.state_bucket.deref())
            .key(self.state_key.deref())
            .send()
            .await?;

        let bytes = state_response.body.collect().await?;
        let state: State = serde_json::from_slice(&bytes.into_bytes())?;

        let response = self
            .request_client
            .get(self.target.as_ref())
            .header(USER_AGENT, format!("{function_name}:{version}"))
            .send()
            .await?;

        let status = response.status().as_u16();

        tracing::info!(%status, "Got a response from the upstream server");

        match state.most_recent_status {
            Some(value) if value != status => {
                tracing::warn!("Status response has changed");
                self.persist_state(status).await?;
            }
            None => {
                tracing::info!("Got the first status for the response");
                self.persist_state(status).await?;
            }
            _ => (),
        };

        Ok(())
    }
}

impl Service<LambdaEvent<Payload>> for Handler {
    type Response = ();
    type Error = lambda_runtime::Error;
    type Future = BoxFuture<Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: LambdaEvent<Payload>) -> Self::Future {
        let handler = self.clone();

        Box::pin(async move { handler.handle(req).await })
    }
}

#[tokio::main]
async fn main() -> LambdaResult<()> {
    tracing_subscriber::fmt().with_ansi(false).init();

    let target = std::env::var("TARGET_URI")?;
    let target = Arc::from(target);

    let state_bucket = Arc::from(std::env::var("STATE_BUCKET")?);
    let state_key = Arc::from(std::env::var("STATE_KEY")?);

    let config = aws_config::load_defaults(BehaviorVersion::latest()).await;

    let request_client = reqwest::Client::new();
    let s3_client = aws_sdk_s3::Client::new(&config);

    let handler = Handler {
        target,
        state_bucket,
        state_key,
        request_client,
        s3_client,
    };

    lambda_runtime::run(handler).await?;

    Ok(())
}
