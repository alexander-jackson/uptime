use std::net::SocketAddr;
use std::str::FromStr;

use aws_config::BehaviorVersion;
use color_eyre::eyre::Result;
use poller::{AlertThreshold, PollerConfiguration};
use reqwest::Client;
use sqlx::PgPool;
use tokio::net::TcpListener;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

mod persistence;
mod poller;
mod router;
mod templates;
mod utils;

use crate::poller::Poller;
use crate::utils::get_env_var;

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

    let sdk_config = aws_config::load_defaults(BehaviorVersion::latest()).await;
    let sns_client = aws_sdk_sns::Client::new(&sdk_config);

    let topic = get_env_var("SNS_TOPIC")?;
    let configuration = PollerConfiguration::new(AlertThreshold::default(), topic);

    let http_client = Client::new();
    let poller = Poller::new(pool.clone(), http_client, sns_client, configuration);

    let router = crate::router::build(pool.clone())?;
    let addr = SocketAddr::from_str(&get_env_var("SERVER_ADDR")?)?;
    let listener = TcpListener::bind(addr).await?;

    tracing::info!(%addr, "listening for incoming requests");

    let _ = tokio::join!(poller.run(), axum::serve(listener, router));

    Ok(())
}
