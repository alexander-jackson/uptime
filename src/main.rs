use color_eyre::eyre::Result;
use poller::Poller;
use reqwest::Client;
use sqlx::PgPool;
use tokio::net::TcpListener;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;
use utils::get_env_var;

mod persistence;
mod poller;
mod router;
mod utils;

async fn setup() -> Result<PgPool> {
    dotenvy::dotenv().ok();

    color_eyre::install()?;

    let fmt_layer = tracing_subscriber::fmt::layer();
    let env_filter_layer = EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .from_env()?;

    tracing_subscriber::registry()
        .with(fmt_layer)
        .with(env_filter_layer)
        .init();

    let pool = crate::persistence::bootstrap().await?;

    Ok(pool)
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let pool = setup().await?;
    let client = Client::new();

    let poller = Poller::new(pool.clone(), client);

    let router = crate::router::build(pool.clone());
    let addr = get_env_var("SERVER_ADDR")?;
    let listener = TcpListener::bind(addr).await?;

    let _ = tokio::join!(poller.run(), axum::serve(listener, router));

    Ok(())
}
