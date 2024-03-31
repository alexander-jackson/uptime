use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use chrono::{DateTime, Utc};
use lambda_runtime::{Config, LambdaEvent, Service};
use reqwest::header::USER_AGENT;
use serde::Deserialize;

type LambdaResult<T> = std::result::Result<T, lambda_runtime::Error>;
type BoxFuture<O> = Pin<Box<dyn Future<Output = O> + Send>>;

#[derive(Debug, Deserialize)]
struct Payload {
    time: DateTime<Utc>,
}

#[derive(Clone, Debug)]
struct Handler {
    target: Arc<String>,
    request_client: reqwest::Client,
}

impl Handler {
    async fn handle(&self, event: LambdaEvent<Payload>) -> LambdaResult<()> {
        tracing::info!(time = %event.payload.time, "Received an event for the lambda invocation!");

        let Config {
            function_name,
            version,
            ..
        } = event.context.env_config.as_ref();

        let response = self
            .request_client
            .get(self.target.as_ref())
            .header(USER_AGENT, format!("{function_name}:{version}"))
            .send()
            .await?;

        let status = response.status().as_u16();

        tracing::info!(%status, "Got a response from the upstream server");

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

    let request_client = reqwest::Client::new();
    let handler = Handler {
        target,
        request_client,
    };

    lambda_runtime::run(handler).await?;

    Ok(())
}
