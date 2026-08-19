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
pub struct VultrAdapter {
    client: Client,
    base_url: String,
}

#[derive(Deserialize)]
struct AccountEnvelope {
    account: Value,
}

#[derive(Deserialize)]
struct InstanceEnvelope {
    instance: Value,
}

#[derive(Deserialize)]
struct InstancesEnvelope {
    instances: Vec<Value>,
    #[serde(default)]
    meta: Option<VultrMeta>,
}

#[derive(Deserialize)]
struct VultrMeta {
    links: Option<VultrLinks>,
}

#[derive(Deserialize)]
struct VultrLinks {
    next: Option<String>,
}

impl Default for VultrAdapter {
    fn default() -> Self {
        Self::new("https://api.vultr.com/v2")
    }
}

impl VultrAdapter {
    #[must_use]
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url.into().trim_end_matches('/').to_owned(),
        }
    }

    async fn request(
        &self,
        method: Method,
        credential: &CredentialMaterial,
        path: &str,
        body: Option<&Value>,
    ) -> Result<Option<Value>, ProviderError> {
        if !matches!(
            credential.credential_type,
            CredentialType::ApiToken | CredentialType::Opaque
        ) {
            return Err(ProviderError::configuration(
                "unsupported_vultr_credential_type",
                false,
            ));
        }
        let request = self
            .client
            .request(method, format!("{}{}", self.base_url, path))
            .bearer_auth(&credential.secret);
        let request = body.map_or(request.try_clone().expect("request is cloneable"), |body| {
            request.json(body)
        });
        let response = request.send().await.map_err(|_| unavailable_error())?;
        if !response.status().is_success() {
            return Err(normalize_http_error(&response));
        }
        if response.status() == StatusCode::NO_CONTENT {
            return Ok(None);
        }
        response.json().await.map(Some).map_err(|_| ProviderError {
            category: ProviderErrorCategory::Unknown,
            code: "invalid_provider_response".to_owned(),
            retryable: false,
            retry_after_seconds: None,
            safe_message: "provider returned an invalid response".to_owned(),
        })
    }

    async fn get_instance(
        &self,
        credential: &CredentialMaterial,
        instance_id: &str,
    ) -> Result<Value, ProviderError> {
        validate_external_id(instance_id)?;
        let value = self
            .request(
                Method::GET,
                credential,
                &format!("/instances/{instance_id}"),
                None,
            )
            .await?
            .ok_or_else(invalid_response)?;
        serde_json::from_value::<InstanceEnvelope>(value)
            .map(|envelope| envelope.instance)
            .map_err(|_| invalid_response())
    }
}

#[async_trait]
impl ProviderAdapter for VultrAdapter {
    fn kind(&self) -> ProviderKind {
        ProviderKind::parse("vultr").expect("static provider kind")
    }

    async fn validate_credential(
        &self,
        credential: &CredentialMaterial,
    ) -> Result<ValidationResult, ProviderError> {
        let value = self
            .request(Method::GET, credential, "/account", None)
            .await?
            .ok_or_else(invalid_response)?;
        let account: AccountEnvelope =
            serde_json::from_value(value).map_err(|_| invalid_response())?;
        Ok(ValidationResult {
            valid: true,
            identity: account
                .account
                .get("email")
                .and_then(Value::as_str)
                .map(str::to_owned),
            scopes: vec!["compute".to_owned()],
        })
    }

    async fn discover_capabilities(
        &self,
        credential: &CredentialMaterial,
    ) -> Result<Vec<Capability>, ProviderError> {
        self.validate_credential(credential).await?;
        Ok(vec![Capability::Compute])
    }

    async fn inventory(
        &self,
        credential: &CredentialMaterial,
        request: &InventoryRequest,
    ) -> Result<InventoryPage, ProviderError> {
        if request.resource_type != "compute_instance" || request.parent_external_id.is_some() {
            return Err(ProviderError::configuration(
                "unsupported_inventory_resource_type",
                false,
            ));
        }
        let path = match request.cursor.as_deref() {
            Some(cursor) => {
                validate_cursor(cursor)?;
                format!("/instances?per_page=100&cursor={cursor}")
            }
            None => "/instances?per_page=100".to_owned(),
        };
        let value = self
            .request(Method::GET, credential, &path, None)
            .await?
            .ok_or_else(invalid_response)?;
        let envelope: InstancesEnvelope =
            serde_json::from_value(value).map_err(|_| invalid_response())?;
        let items = envelope
            .instances
            .into_iter()
            .map(normalize_instance)
            .collect::<Result<_, _>>()?;
        let next_cursor = envelope
            .meta
            .and_then(|meta| meta.links)
            .and_then(|links| links.next)
            .and_then(|next| extract_cursor(&next));
        Ok(InventoryPage { items, next_cursor })
    }

    async fn execute(
        &self,
        credential: &CredentialMaterial,
        request: &ProviderOperationRequest,
    ) -> Result<ProviderOperationResult, ProviderError> {
        if request.resource_type != "compute_instance" {
            return Err(ProviderError::configuration(
                "unsupported_operation_resource_type",
                false,
            ));
        }
        if request.action == "create" {
            validate_create_parameters(&request.parameters)?;
            let value = self
                .request(
                    Method::POST,
                    credential,
                    "/instances",
                    Some(&request.parameters),
                )
                .await?
                .ok_or_else(invalid_response)?;
            let instance: InstanceEnvelope =
                serde_json::from_value(value).map_err(|_| invalid_response())?;
            let item = normalize_instance(instance.instance)?;
            return Ok(ProviderOperationResult {
                external_id: Some(item.external_id),
                state: item.state,
            });
        }

        let instance_id = request
            .external_id
            .as_deref()
            .ok_or_else(|| ProviderError::configuration("instance_id_required", false))?;
        validate_external_id(instance_id)?;
        if request.action == "get" {
            let item = normalize_instance(self.get_instance(credential, instance_id).await?)?;
            return Ok(ProviderOperationResult {
                external_id: Some(item.external_id),
                state: item.state,
            });
        }
        let suffix = match request.action.as_str() {
            "start" => "start",
            "stop" => "halt",
            "reboot" => "reboot",
            "delete" => {
                self.request(
                    Method::DELETE,
                    credential,
                    &format!("/instances/{instance_id}"),
                    None,
                )
                .await?;
                return Ok(ProviderOperationResult {
                    external_id: Some(instance_id.to_owned()),
                    state: json!({ "status": "deleted" }),
                });
            }
            _ => {
                return Err(ProviderError::configuration(
                    "unsupported_compute_action",
                    false,
                ));
            }
        };
        self.request(
            Method::POST,
            credential,
            &format!("/instances/{instance_id}/{suffix}"),
            None,
        )
        .await?;
        let item = normalize_instance(self.get_instance(credential, instance_id).await?)?;
        Ok(ProviderOperationResult {
            external_id: Some(item.external_id),
            state: item.state,
        })
    }
}

fn normalize_instance(instance: Value) -> Result<InventoryItem, ProviderError> {
    let id = instance
        .get("id")
        .and_then(Value::as_str)
        .ok_or_else(invalid_response)?;
    let name = instance
        .get("label")
        .or_else(|| instance.get("hostname"))
        .and_then(Value::as_str)
        .unwrap_or(id);
    let status = match (
        instance.get("power_status").and_then(Value::as_str),
        instance.get("status").and_then(Value::as_str),
    ) {
        (Some("running"), _) | (_, Some("active")) => "running",
        (Some("stopped"), _) | (_, Some("suspended")) => "stopped",
        (_, Some("pending")) => "provisioning",
        _ => "unknown",
    };
    Ok(InventoryItem {
        external_type: "compute_instance".to_owned(),
        external_id: id.to_owned(),
        name: name.to_owned(),
        state: json!({
            "name": name,
            "status": status,
            "provider_status": instance.get("status").cloned().unwrap_or(Value::Null),
            "power_status": instance.get("power_status").cloned().unwrap_or(Value::Null),
            "server_status": instance.get("server_status").cloned().unwrap_or(Value::Null),
            "region": instance.get("region").cloned().unwrap_or(Value::Null),
            "plan": instance.get("plan").cloned().unwrap_or(Value::Null),
            "os": instance.get("os").cloned().unwrap_or(Value::Null),
            "vcpu_count": instance.get("vcpu_count").cloned().unwrap_or(Value::Null),
            "ram": instance.get("ram").cloned().unwrap_or(Value::Null),
            "disk": instance.get("disk").cloned().unwrap_or(Value::Null),
            "main_ip": instance.get("main_ip").cloned().unwrap_or(Value::Null),
            "v6_main_ip": instance.get("v6_main_ip").cloned().unwrap_or(Value::Null),
        }),
        metadata: instance,
    })
}

fn validate_create_parameters(value: &Value) -> Result<(), ProviderError> {
    let has_source = ["os_id", "snapshot_id", "iso_id", "app_id", "image_id"]
        .iter()
        .any(|key| value.get(key).is_some_and(|value| !value.is_null()));
    if value.get("region").and_then(Value::as_str).is_some()
        && value.get("plan").and_then(Value::as_str).is_some()
        && has_source
    {
        Ok(())
    } else {
        Err(ProviderError::configuration(
            "invalid_compute_instance_parameters",
            false,
        ))
    }
}

fn validate_external_id(value: &str) -> Result<(), ProviderError> {
    if !value.is_empty()
        && value.len() <= 128
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '-')
    {
        Ok(())
    } else {
        Err(ProviderError::configuration("invalid_external_id", false))
    }
}

fn validate_cursor(value: &str) -> Result<(), ProviderError> {
    if !value.is_empty()
        && value.len() <= 512
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '=')
        })
    {
        Ok(())
    } else {
        Err(ProviderError::configuration(
            "invalid_inventory_cursor",
            false,
        ))
    }
}

fn extract_cursor(next: &str) -> Option<String> {
    let query = next.split_once('?').map_or(next, |(_, query)| query);
    query.split('&').find_map(|pair| {
        pair.split_once('=')
            .filter(|(key, value)| *key == "cursor" && validate_cursor(value).is_ok())
            .map(|(_, value)| value.to_owned())
    })
}

fn invalid_response() -> ProviderError {
    ProviderError {
        category: ProviderErrorCategory::Unknown,
        code: "invalid_provider_response".to_owned(),
        retryable: false,
        retry_after_seconds: None,
        safe_message: "provider returned an invalid response".to_owned(),
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
    let (category, code, retryable) = match response.status() {
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
        routing::{get, post},
    };
    use tokio::net::TcpListener;

    fn instance() -> Value {
        json!({
            "id": "instance-1", "label": "api-1", "status": "active",
            "power_status": "running", "server_status": "ok", "region": "ewr",
            "plan": "vc2-1c-2gb", "os": "Debian", "vcpu_count": 1,
            "ram": 2048, "disk": 55, "main_ip": "192.0.2.1"
        })
    }

    async fn mock_server() -> String {
        let app = Router::new()
            .route(
                "/account",
                get(|| async { Json(json!({ "account": { "email": "owner@example.com" } })) }),
            )
            .route(
                "/instances",
                get(|| async {
                    Json(
                        json!({ "instances": [instance()], "meta": { "links": { "next": null } } }),
                    )
                })
                .post(|| async { (StatusCode::CREATED, Json(json!({ "instance": instance() }))) }),
            )
            .route(
                "/instances/{id}",
                get(|| async { Json(json!({ "instance": instance() })) })
                    .delete(|| async { StatusCode::NO_CONTENT }),
            )
            .route(
                "/instances/{id}/start",
                post(|| async { StatusCode::NO_CONTENT }),
            )
            .route(
                "/instances/{id}/halt",
                post(|| async { StatusCode::NO_CONTENT }),
            )
            .route(
                "/instances/{id}/reboot",
                post(|| async { StatusCode::NO_CONTENT }),
            );
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        format!("http://{address}")
    }

    fn credential() -> CredentialMaterial {
        CredentialMaterial {
            credential_type: CredentialType::ApiToken,
            identity: None,
            secret: "token".to_owned(),
            consumer_key: None,
        }
    }

    #[tokio::test]
    async fn validates_syncs_and_runs_complete_instance_lifecycle() {
        let adapter = VultrAdapter::new(mock_server().await);
        let credential = credential();
        assert!(
            adapter
                .validate_credential(&credential)
                .await
                .unwrap()
                .valid
        );
        assert_eq!(
            adapter.discover_capabilities(&credential).await.unwrap(),
            vec![Capability::Compute]
        );
        let page = adapter
            .inventory(
                &credential,
                &InventoryRequest {
                    resource_type: "compute_instance".to_owned(),
                    parent_external_id: None,
                    cursor: None,
                },
            )
            .await
            .unwrap();
        assert_eq!(page.items[0].external_id, "instance-1");

        let requests = [
            (
                "create",
                None,
                json!({ "region": "ewr", "plan": "vc2-1c-2gb", "os_id": 1743 }),
            ),
            ("get", Some("instance-1"), json!({})),
            ("start", Some("instance-1"), json!({})),
            ("stop", Some("instance-1"), json!({})),
            ("reboot", Some("instance-1"), json!({})),
            ("delete", Some("instance-1"), json!({})),
        ];
        for (action, external_id, parameters) in requests {
            let result = adapter
                .execute(
                    &credential,
                    &ProviderOperationRequest {
                        action: action.to_owned(),
                        resource_type: "compute_instance".to_owned(),
                        external_id: external_id.map(str::to_owned),
                        parameters,
                        idempotency_key: format!("{action}-1"),
                    },
                )
                .await
                .unwrap();
            assert_eq!(result.external_id.as_deref(), Some("instance-1"));
        }
    }
}
