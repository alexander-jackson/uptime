use lambda_runtime::{service_fn, LambdaEvent};

type LambdaResult<T> = std::result::Result<T, lambda_runtime::Error>;

async fn handler(event: LambdaEvent<()>) -> LambdaResult<()> {
    tracing::info!(payload = ?event.payload, "Received an event for the lambda invocation!");

    Ok(())
}

#[tokio::main]
async fn main() -> LambdaResult<()> {
    tracing_subscriber::fmt().init();

    lambda_runtime::run(service_fn(handler)).await?;

    Ok(())
}
