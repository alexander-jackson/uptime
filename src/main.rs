use chrono::{DateTime, Utc};
use lambda_runtime::{service_fn, LambdaEvent};
use serde::Deserialize;

type LambdaResult<T> = std::result::Result<T, lambda_runtime::Error>;

#[derive(Debug, Deserialize)]
struct Payload {
    time: DateTime<Utc>,
}

async fn handler(event: LambdaEvent<Payload>) -> LambdaResult<()> {
    tracing::info!(time = %event.payload.time, "Received an event for the lambda invocation!");

    Ok(())
}

#[tokio::main]
async fn main() -> LambdaResult<()> {
    tracing_subscriber::fmt().init();

    lambda_runtime::run(service_fn(handler)).await?;

    Ok(())
}
