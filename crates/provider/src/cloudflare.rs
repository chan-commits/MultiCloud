use crate::{
    Capability, CredentialMaterial, CredentialType, InventoryItem, InventoryPage, InventoryRequest,
    ProviderAdapter, ProviderError, ProviderErrorCategory, ProviderKind, ProviderOperationRequest,
    ProviderOperationResult, ValidationResult,
};
use async_trait::async_trait;
use reqwest::{Client, Method, StatusCode, header::RETRY_AFTER};
use serde::Deserialize;
use serde_json::{Value, json};

#[derive(Clone)]
pub struct CloudflareAdapter {
    client: Client,
    base_url: String,
}

#[derive(Deserialize)]
struct CloudflareEnvelope<T> {
    success: bool,
    result: T,
    #[serde(default)]
    result_info: Option<ResultInfo>,
}

#[derive(Deserialize)]
struct ResultInfo {
    page: Option<u32>,
    total_pages: Option<u32>,
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
        self.request(Method::GET, credential, path, None).await
    }

    async fn request<T: for<'de> Deserialize<'de>>(
        &self,
        method: Method,
        credential: &CredentialMaterial,
        path: &str,
        body: Option<&Value>,
    ) -> Result<CloudflareEnvelope<T>, ProviderError> {
        let request = self
            .client
            .request(method, format!("{}{}", self.base_url, path));
        let request = match credential.credential_type {
            CredentialType::ApiToken | CredentialType::Opaque => {
                request.bearer_auth(&credential.secret)
            }
            CredentialType::OvhApplication => {
                return Err(ProviderError::configuration(
                    "unsupported_cloudflare_credential_type",
                    false,
                ));
            }
            CredentialType::GlobalApiKey => request
                .header(
                    "X-Auth-Email",
                    credential.identity.as_deref().ok_or_else(|| {
                        ProviderError::configuration("global_api_key_email_required", false)
                    })?,
                )
                .header("X-Auth-Key", &credential.secret),
        };
        let request = if let Some(body) = body {
            request.json(body)
        } else {
            request
        };
        let response = request.send().await.map_err(|_| unavailable_error())?;
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
        let (valid, identity) = match credential.credential_type {
            CredentialType::GlobalApiKey => {
                let response: CloudflareEnvelope<Value> = self.get(credential, "/user").await?;
                (
                    response.success,
                    response
                        .result
                        .get("id")
                        .and_then(Value::as_str)
                        .map(str::to_owned),
                )
            }
            CredentialType::ApiToken | CredentialType::Opaque | CredentialType::OvhApplication => {
                let response: CloudflareEnvelope<TokenVerification> =
                    self.get(credential, "/user/tokens/verify").await?;
                (
                    response.success && response.result.status == "active",
                    response.result.id,
                )
            }
        };
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
            identity,
            scopes: Vec::new(),
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
        credential: &CredentialMaterial,
        request: &InventoryRequest,
    ) -> Result<InventoryPage, ProviderError> {
        let page = request.cursor.as_deref().unwrap_or("1");
        if !page.chars().all(|character| character.is_ascii_digit()) {
            return Err(ProviderError::configuration(
                "invalid_inventory_cursor",
                false,
            ));
        }
        let (path, external_type) = match request.resource_type.as_str() {
            "dns_zone" => (format!("/zones?per_page=50&page={page}"), "dns_zone"),
            "dns_record" => {
                let zone_id = request
                    .parent_external_id
                    .as_deref()
                    .ok_or_else(|| ProviderError::configuration("dns_zone_id_required", false))?;
                validate_external_id(zone_id)?;
                (
                    format!("/zones/{zone_id}/dns_records?per_page=100&page={page}"),
                    "dns_record",
                )
            }
            _ => {
                return Err(ProviderError::configuration(
                    "unsupported_inventory_resource_type",
                    false,
                ));
            }
        };
        let response: CloudflareEnvelope<Vec<Value>> = self.get(credential, &path).await?;
        let items = response
            .result
            .into_iter()
            .filter(|resource| {
                external_type != "dns_record"
                    || resource
                        .get("type")
                        .and_then(Value::as_str)
                        .is_some_and(is_supported_record_type)
            })
            .map(|resource| normalize_inventory_item(external_type, resource))
            .collect::<Result<_, _>>()?;
        let next_cursor = response.result_info.and_then(|info| {
            let page = info.page?;
            (page < info.total_pages?).then(|| page.saturating_add(1).to_string())
        });
        Ok(InventoryPage { items, next_cursor })
    }

    async fn execute(
        &self,
        credential: &CredentialMaterial,
        request: &ProviderOperationRequest,
    ) -> Result<ProviderOperationResult, ProviderError> {
        if request.resource_type != "dns_record" {
            return Err(ProviderError::configuration(
                "unsupported_operation_resource_type",
                false,
            ));
        }
        let zone_id = request
            .parameters
            .get("zone_id")
            .and_then(Value::as_str)
            .ok_or_else(|| ProviderError::configuration("dns_zone_id_required", false))?;
        validate_external_id(zone_id)?;
        let (method, path, body) =
            match request.action.as_str() {
                "create" => {
                    validate_dns_record_parameters(&request.parameters)?;
                    (
                        Method::POST,
                        format!("/zones/{zone_id}/dns_records"),
                        Some(&request.parameters),
                    )
                }
                "update" => {
                    validate_dns_record_parameters(&request.parameters)?;
                    let record_id = request.external_id.as_deref().ok_or_else(|| {
                        ProviderError::configuration("dns_record_id_required", false)
                    })?;
                    validate_external_id(record_id)?;
                    (
                        Method::PUT,
                        format!("/zones/{zone_id}/dns_records/{record_id}"),
                        Some(&request.parameters),
                    )
                }
                "delete" => {
                    let record_id = request.external_id.as_deref().ok_or_else(|| {
                        ProviderError::configuration("dns_record_id_required", false)
                    })?;
                    validate_external_id(record_id)?;
                    (
                        Method::DELETE,
                        format!("/zones/{zone_id}/dns_records/{record_id}"),
                        None,
                    )
                }
                _ => {
                    return Err(ProviderError::configuration(
                        "unsupported_dns_action",
                        false,
                    ));
                }
            };
        let response: CloudflareEnvelope<Value> =
            self.request(method, credential, &path, body).await?;
        let external_id = response
            .result
            .get("id")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .or_else(|| request.external_id.clone());
        Ok(ProviderOperationResult {
            external_id,
            state: response.result,
        })
    }
}

fn validate_external_id(value: &str) -> Result<(), ProviderError> {
    let valid = !value.is_empty()
        && value.len() <= 128
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'));
    if valid {
        Ok(())
    } else {
        Err(ProviderError::configuration("invalid_external_id", false))
    }
}

fn is_supported_record_type(value: &str) -> bool {
    matches!(value, "A" | "AAAA" | "CNAME" | "TXT" | "MX")
}

fn validate_dns_record_parameters(parameters: &Value) -> Result<(), ProviderError> {
    let record_type = parameters.get("type").and_then(Value::as_str);
    if record_type.is_some_and(is_supported_record_type)
        && parameters.get("name").and_then(Value::as_str).is_some()
        && parameters.get("content").and_then(Value::as_str).is_some()
    {
        Ok(())
    } else {
        Err(ProviderError::configuration(
            "invalid_dns_record_parameters",
            false,
        ))
    }
}

fn normalize_inventory_item(
    external_type: &str,
    resource: Value,
) -> Result<InventoryItem, ProviderError> {
    let external_id = resource
        .get("id")
        .and_then(Value::as_str)
        .ok_or_else(|| ProviderError::configuration("provider_resource_id_missing", false))?;
    let name = resource
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| ProviderError::configuration("provider_resource_name_missing", false))?;
    let state = if external_type == "dns_zone" {
        json!({
            "name": name,
            "status": resource.get("status").cloned().unwrap_or(Value::Null),
            "name_servers": resource.get("name_servers").cloned().unwrap_or_else(|| json!([])),
        })
    } else {
        json!({
            "type": resource.get("type").cloned().unwrap_or(Value::Null),
            "name": name,
            "content": resource.get("content").cloned().unwrap_or(Value::Null),
            "ttl": resource.get("ttl").cloned().unwrap_or(Value::Null),
            "priority": resource.get("priority").cloned().unwrap_or(Value::Null),
            "proxied": resource.get("proxied").cloned().unwrap_or(Value::Null),
        })
    };
    Ok(InventoryItem {
        external_type: external_type.to_owned(),
        external_id: external_id.to_owned(),
        name: name.to_owned(),
        state,
        metadata: resource,
    })
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
    use axum::{
        Json, Router,
        http::StatusCode,
        routing::{get, put},
    };
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
                get(|| async {
                    Json(json!({
                        "success": true,
                        "result": [{
                            "id": "zone123", "name": "example.com", "status": "active",
                            "name_servers": ["ns1.example.com"]
                        }],
                        "result_info": { "page": 1, "total_pages": 1 }
                    }))
                }),
            )
            .route(
                "/user",
                get(|| async { Json(json!({ "success": true, "result": { "id": "user-id" } })) }),
            )
            .route(
                "/zones/{zone_id}/dns_records",
                get(|| async {
                    Json(json!({
                        "success": true,
                        "result": [{
                            "id": "record123", "type": "A", "name": "www.example.com",
                            "content": "1.1.1.1", "ttl": 120, "proxied": false
                        }],
                        "result_info": { "page": 1, "total_pages": 1 }
                    }))
                })
                .post(|| async {
                    Json(json!({
                        "success": true,
                        "result": { "id": "record-created", "type": "A", "name": "www.example.com", "content": "1.1.1.1" }
                    }))
                }),
            )
            .route(
                "/zones/{zone_id}/dns_records/{record_id}",
                put(|| async {
                    Json(json!({
                        "success": true,
                        "result": { "id": "record123", "type": "A", "name": "www.example.com", "content": "2.2.2.2" }
                    }))
                })
                .delete(|| async {
                    Json(json!({ "success": true, "result": { "id": "record123" } }))
                }),
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
            credential_type: CredentialType::ApiToken,
            identity: None,
            secret: "test-token".to_owned(),
            consumer_key: None,
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

        let zones = adapter
            .inventory(
                &credential,
                &InventoryRequest {
                    resource_type: "dns_zone".to_owned(),
                    parent_external_id: None,
                    cursor: None,
                },
            )
            .await
            .unwrap();
        assert_eq!(zones.items[0].external_id, "zone123");
        let records = adapter
            .inventory(
                &credential,
                &InventoryRequest {
                    resource_type: "dns_record".to_owned(),
                    parent_external_id: Some("zone123".to_owned()),
                    cursor: None,
                },
            )
            .await
            .unwrap();
        assert_eq!(records.items[0].external_id, "record123");
        let created = adapter
            .execute(
                &credential,
                &ProviderOperationRequest {
                    action: "create".to_owned(),
                    resource_type: "dns_record".to_owned(),
                    external_id: None,
                    parameters: json!({
                        "zone_id": "zone123", "type": "A", "name": "www.example.com",
                        "content": "1.1.1.1", "ttl": 120
                    }),
                    idempotency_key: "dns-create-1".to_owned(),
                },
            )
            .await
            .unwrap();
        assert_eq!(created.external_id.as_deref(), Some("record-created"));

        let global = CredentialMaterial {
            credential_type: CredentialType::GlobalApiKey,
            identity: Some("owner@example.com".to_owned()),
            secret: "global-key".to_owned(),
            consumer_key: None,
        };
        assert!(adapter.validate_credential(&global).await.unwrap().valid);
    }
}
