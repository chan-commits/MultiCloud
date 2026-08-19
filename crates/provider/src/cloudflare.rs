use crate::{
    Capability, CredentialMaterial, InventoryItem, ProviderAdapter, ProviderError,
    ProviderErrorCategory, ProviderKind, ProviderOperationRequest, ProviderOperationResult,
    ValidationResult,
};
use async_trait::async_trait;
use reqwest::{Client, StatusCode, header::RETRY_AFTER};
use serde::Deserialize;

#[derive(Clone)]
pub struct CloudflareAdapter {
    client: Client,
    base_url: String,
}

#[derive(Deserialize)]
struct CloudflareEnvelope<T> {
    success: bool,
    result: T,
}

#[derive(Deserialize)]
struct TokenVerification {
    id: Option<String>,
    status: String,
}

impl Default for CloudflareAdapter {
    fn default() -> Self {
        Self::new("https://api.cloudflare.com/client/v4")
    }
}

impl CloudflareAdapter {
    #[must_use]
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url.into().trim_end_matches('/').to_owned(),
        }
    }

    async fn get<T: for<'de> Deserialize<'de>>(
        &self,
        credential: &CredentialMaterial,
        path: &str,
    ) -> Result<CloudflareEnvelope<T>, ProviderError> {
        let response = self
            .client
            .get(format!("{}{}", self.base_url, path))
            .bearer_auth(&credential.secret)
            .send()
            .await
            .map_err(|_| unavailable_error())?;
        if !response.status().is_success() {
            return Err(normalize_http_error(&response));
        }
        response.json().await.map_err(|_| ProviderError {
            category: ProviderErrorCategory::Unknown,
            code: "invalid_provider_response".to_owned(),
            retryable: false,
            retry_after_seconds: None,
            safe_message: "provider returned an invalid response".to_owned(),
        })
    }
}

#[async_trait]
impl ProviderAdapter for CloudflareAdapter {
    fn kind(&self) -> ProviderKind {
        ProviderKind::parse("cloudflare").expect("static provider kind")
    }

    async fn validate_credential(
        &self,
        credential: &CredentialMaterial,
    ) -> Result<ValidationResult, ProviderError> {
        let response: CloudflareEnvelope<TokenVerification> =
            self.get(credential, "/user/tokens/verify").await?;
        let valid = response.success && response.result.status == "active";
        if !valid {
            return Err(ProviderError {
                category: ProviderErrorCategory::Authentication,
                code: "inactive_api_token".to_owned(),
                retryable: false,
                retry_after_seconds: None,
                safe_message: "Cloudflare API token is not active".to_owned(),
            });
        }
        Ok(ValidationResult {
            valid,
            identity: response.result.id,
        })
    }

    async fn discover_capabilities(
        &self,
        credential: &CredentialMaterial,
    ) -> Result<Vec<Capability>, ProviderError> {
        self.validate_credential(credential).await?;
        let zones: CloudflareEnvelope<Vec<serde_json::Value>> =
            self.get(credential, "/zones?per_page=1").await?;
        if zones.success {
            Ok(vec![Capability::Dns])
        } else {
            Ok(Vec::new())
        }
    }

    async fn inventory(
        &self,
        _credential: &CredentialMaterial,
        _resource_type: &str,
    ) -> Result<Vec<InventoryItem>, ProviderError> {
        Err(ProviderError::configuration(
            "inventory_available_in_phase_5",
            false,
        ))
    }

    async fn execute(
        &self,
        _credential: &CredentialMaterial,
        _request: &ProviderOperationRequest,
    ) -> Result<ProviderOperationResult, ProviderError> {
        Err(ProviderError::configuration(
            "operations_available_in_phase_5",
            false,
        ))
    }
}

fn unavailable_error() -> ProviderError {
    ProviderError {
        category: ProviderErrorCategory::Unavailable,
        code: "provider_unavailable".to_owned(),
        retryable: true,
        retry_after_seconds: None,
        safe_message: "provider is temporarily unavailable".to_owned(),
    }
}

fn normalize_http_error(response: &reqwest::Response) -> ProviderError {
    let status = response.status();
    let (category, code, retryable) = match status {
        StatusCode::UNAUTHORIZED => (
            ProviderErrorCategory::Authentication,
            "authentication_failed",
            false,
        ),
        StatusCode::FORBIDDEN => (
            ProviderErrorCategory::Authorization,
            "permission_denied",
            false,
        ),
        StatusCode::NOT_FOUND => (ProviderErrorCategory::NotFound, "not_found", false),
        StatusCode::CONFLICT => (ProviderErrorCategory::Conflict, "conflict", false),
        StatusCode::TOO_MANY_REQUESTS => (ProviderErrorCategory::RateLimited, "rate_limited", true),
        status if status.is_server_error() => (
            ProviderErrorCategory::Unavailable,
            "provider_unavailable",
            true,
        ),
        _ => (
            ProviderErrorCategory::InvalidRequest,
            "provider_request_rejected",
            false,
        ),
    };
    ProviderError {
        category,
        code: code.to_owned(),
        retryable,
        retry_after_seconds: response
            .headers()
            .get(RETRY_AFTER)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse().ok()),
        safe_message: "provider request failed".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{Json, Router, http::StatusCode, routing::get};
    use serde_json::json;
    use tokio::net::TcpListener;

    async fn mock_server() -> String {
        let app = Router::new()
            .route(
                "/user/tokens/verify",
                get(|| async {
                    Json(json!({
                        "success": true,
                        "result": { "id": "token-id", "status": "active" }
                    }))
                }),
            )
            .route(
                "/zones",
                get(|| async { Json(json!({ "success": true, "result": [] })) }),
            )
            .route("/limited", get(|| async { StatusCode::TOO_MANY_REQUESTS }));
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        format!("http://{address}")
    }

    #[tokio::test]
    async fn validates_token_and_discovers_dns() {
        let base_url = mock_server().await;
        let adapter = CloudflareAdapter::new(&base_url);
        let credential = CredentialMaterial {
            secret: "test-token".to_owned(),
        };
        let validation = adapter.validate_credential(&credential).await.unwrap();
        assert!(validation.valid);
        assert_eq!(validation.identity.as_deref(), Some("token-id"));
        assert_eq!(
            adapter.discover_capabilities(&credential).await.unwrap(),
            vec![Capability::Dns]
        );
        let Err(error) = adapter
            .get::<serde_json::Value>(&credential, "/limited")
            .await
        else {
            panic!("rate-limited request unexpectedly succeeded");
        };
        assert_eq!(error.category, ProviderErrorCategory::RateLimited);
        assert!(error.retryable);
    }
}
