use axum::http::StatusCode;
use tokio::sync::Notify;

use std::sync::RwLock;

use crate::config;
use crate::transmission;

#[derive(Debug)]
pub struct TransmissionRpc {
    url: config::RpcUrl,
    auth: TransmissionAuth,
    /// The transmission session ID. Will need to be updated infrequently.
    id: RwLock<String>,
    pub notify: Notify,
}

impl TransmissionRpc {
    pub fn new(url: config::RpcUrl, auth: TransmissionAuth) -> Self {
        Self {
            url,
            auth,
            id: RwLock::new(String::new()),
            notify: Notify::new(),
        }
    }

    pub async fn request<T: serde::de::DeserializeOwned>(
        &self,
        rpc: &reqwest::Client,
        msg: &transmission::types::Request,
    ) -> Result<transmission::types::Response<T>, StatusCode> {
        let resp = self.csrf_request(rpc, msg).await?;

        match resp.status() {
            x @ reqwest::StatusCode::UNAUTHORIZED => {
                // could be wrong username/password
                return Err(x);
            }
            x @ reqwest::StatusCode::FORBIDDEN => {
                // could be connecting from a non-whitelisted IP
                return Err(x);
            }
            x if !x.is_success() => {
                println!(
                    "Transmission returned {}: {}",
                    resp.status(),
                    resp.text().await.unwrap_or(String::new()),
                );
                return Err(StatusCode::BAD_GATEWAY);
            }
            _ => {}
        }

        // transmission unfortunately uses success http statuses for unsucessful rpc requests

        let resp = resp
            .json::<transmission::types::Response<T>>()
            .await
            .inspect_err(|e| println!("Failed to parse JSON response: {e:?}"))
            .or(Err(StatusCode::BAD_GATEWAY))?;

        if !resp.is_success() {
            println!(
                "Transmission returned an unsuccessful response: {}",
                resp.result,
            );
            return Err(StatusCode::BAD_GATEWAY);
        }

        Ok(resp)
    }

    async fn csrf_request<T: serde::Serialize + ?Sized>(
        &self,
        rpc: &reqwest::Client,
        msg: &T,
    ) -> Result<reqwest::Response, StatusCode> {
        let old_id: String = self
            .id
            .read()
            .or(Err(StatusCode::INTERNAL_SERVER_ERROR))?
            .clone();

        let resp = self.http_request(rpc, &old_id, msg).await?;

        if let Some(new_id) = resp.headers().get("X-Transmission-Session-Id") {
            let new_id = new_id
                .to_str()
                .inspect_err(|e| println!("Bad transmission session ID: {e:?}"))
                .or(Err(StatusCode::BAD_GATEWAY))?
                .to_string();

            if new_id != old_id {
                self.id
                    .write()
                    .or(Err(StatusCode::INTERNAL_SERVER_ERROR))?
                    .clone_from(&new_id);
            }

            if resp.status() == reqwest::StatusCode::CONFLICT {
                return self.http_request(rpc, &new_id, msg).await;
            }
        }

        Ok(resp)
    }

    async fn http_request<T: serde::Serialize + ?Sized>(
        &self,
        rpc: &reqwest::Client,
        rpc_id: &str,
        msg: &T,
    ) -> Result<reqwest::Response, StatusCode> {
        rpc.post(&self.url.to_string())
            .basic_auth(&self.auth.username, Some(&self.auth.password))
            .header("X-Transmission-Session-Id", rpc_id)
            .json(msg)
            .send()
            .await
            .inspect_err(|e| println!("Sending json request failed: {e:?}"))
            .or(Err(StatusCode::BAD_GATEWAY))
    }
}

#[derive(Debug, Clone)]
pub struct TransmissionAuth {
    pub username: String,
    pub password: String,
}
