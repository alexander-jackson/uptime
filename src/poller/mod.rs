use std::fmt::{self, Display};
use std::time::Duration;

use color_eyre::eyre::Result;
use reqwest::Client;
use sqlx::types::chrono::Utc;
use sqlx::PgPool;

use crate::persistence::Origin;

#[derive(Copy, Clone, Debug, sqlx::Type)]
pub enum FailureReason {
    RequestTimeout,
    Redirection,
    BadRequest,
    ConnectionFailure,
    InvalidBody,
    Unknown,
}

impl FailureReason {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::RequestTimeout => "RequestTimeout",
            Self::Redirection => "Redirection",
            Self::BadRequest => "BadRequest",
            Self::ConnectionFailure => "ConnectionFailure",
            Self::InvalidBody => "InvalidBody",
            Self::Unknown => "Unknown",
        }
    }
}

impl Display for FailureReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let repr = self.as_str();

        write!(f, "{repr}")
    }
}

impl From<reqwest::Error> for FailureReason {
    fn from(error: reqwest::Error) -> Self {
        if error.is_timeout() {
            return Self::RequestTimeout;
        }

        if error.is_redirect() {
            return Self::Redirection;
        }

        if error.is_request() {
            return Self::BadRequest;
        }

        if error.is_connect() {
            return Self::ConnectionFailure;
        }

        if error.is_body() {
            return Self::InvalidBody;
        }

        Self::Unknown
    }
}

pub struct Poller {
    pool: PgPool,
    client: Client,
}

impl Poller {
    pub fn new(pool: PgPool, client: Client) -> Self {
        Self { pool, client }
    }

    pub async fn run(&self) {
        loop {
            if let Err(e) = self.query_all_origins().await {
                tracing::warn!(%e, "failed to query all the origins");
            }

            tokio::time::sleep(Duration::from_secs(60)).await;
        }
    }

    async fn query_all_origins(&self) -> Result<()> {
        let Self { pool, client } = self;

        // Find all the available origins
        let origins = crate::persistence::fetch_origins(pool).await?;
        let timeout = Duration::from_secs(3);

        let mut tx = pool.begin().await?;

        for Origin { origin_uid, uri } in origins {
            let start = Utc::now();

            match client.get(uri).timeout(timeout).send().await {
                Ok(res) => {
                    let status = res.status();
                    let latency_millis = (Utc::now() - start).num_milliseconds();

                    let query_uid = crate::persistence::insert_query(
                        &mut tx,
                        origin_uid,
                        status.as_u16(),
                        latency_millis,
                        start,
                    )
                    .await?;

                    tracing::info!(
                        %origin_uid,
                        %query_uid,
                        %status,
                        %latency_millis,
                        "made a request to the origin"
                    );
                }
                Err(e) => {
                    let failure_reason = FailureReason::from(e);

                    let query_failure_uid = crate::persistence::insert_query_failure(
                        &mut tx,
                        origin_uid,
                        failure_reason,
                        start,
                    )
                    .await?;

                    tracing::warn!(
                        %origin_uid,
                        %query_failure_uid,
                        %failure_reason,
                        "failed to make a request to the origin"
                    );
                }
            }
        }

        tx.commit().await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests;
