use crate::{
    Capability, CredentialMaterial, CredentialType, InventoryItem, InventoryPage, InventoryRequest,
    ProviderAdapter, ProviderError, ProviderErrorCategory, ProviderKind, ProviderOperationRequest,
    ProviderOperationResult, ValidationResult,
};
use async_trait::async_trait;
use reqwest::{Client, Method, StatusCode, header::RETRY_AFTER};
use serde_json::{Value, json};
use sha1::{Digest, Sha1};

#[derive(Clone)]
pub struct OvhAdapter {
    client: Client,
    base_url: String,
}

impl Default for OvhAdapter {
    fn default() -> Self {
        Self::new("https://eu.api.ovh.com/1.0")
    }
}

impl OvhAdapter {
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
    ) -> Result<Value, ProviderError> {
        let application_key = credential
            .identity
            .as_deref()
            .ok_or_else(|| ProviderError::configuration("ovh_application_key_required", false))?;
        let consumer_key = credential
            .consumer_key
            .as_deref()
            .ok_or_else(|| ProviderError::configuration("ovh_consumer_key_required", false))?;
        if credential.credential_type != CredentialType::OvhApplication {
            return Err(ProviderError::configuration(
                "unsupported_ovh_credential_type",
                false,
            ));
        }
        let timestamp = self.server_time().await?;
        let target = format!("{}{}", self.base_url, path);
        let serialized_body = body.map_or_else(String::new, Value::to_string);
        let signature_source = format!(
            "{}+{}+{}+{}+{}+{}",
            credential.secret,
            consumer_key,
            method.as_str(),
            target,
            serialized_body,
            timestamp
        );
        let signature = format!("$1${:x}", Sha1::digest(signature_source.as_bytes()));
        let mut request = self
            .client
            .request(method, target)
            .header("X-Ovh-Application", application_key)
            .header("X-Ovh-Consumer", consumer_key)
            .header("X-Ovh-Timestamp", timestamp.to_string())
            .header("X-Ovh-Signature", signature);
        if body.is_some() {
            request = request
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .body(serialized_body);
        }
        let response = request.send().await.map_err(|_| unavailable_error())?;
        if !response.status().is_success() {
            return Err(normalize_http_error(&response));
        }
        if response.status() == StatusCode::NO_CONTENT {
            return Ok(Value::Null);
        }
        response.json().await.map_err(|_| invalid_response())
    }

    async fn server_time(&self) -> Result<i64, ProviderError> {
        let response = self
            .client
            .get(format!("{}/auth/time", self.base_url))
            .send()
            .await
            .map_err(|_| unavailable_error())?;
        if !response.status().is_success() {
            return Err(normalize_http_error(&response));
        }
        response.json::<i64>().await.map_err(|_| invalid_response())
    }

    async fn get_vps(
        &self,
        credential: &CredentialMaterial,
        service_name: &str,
    ) -> Result<Value, ProviderError> {
        validate_service_name(service_name)?;
        self.request(
            Method::GET,
            credential,
            &format!("/vps/{service_name}"),
            None,
        )
        .await
    }
}

#[async_trait]
impl ProviderAdapter for OvhAdapter {
    fn kind(&self) -> ProviderKind {
        ProviderKind::parse("ovh").expect("static provider kind")
    }

    async fn validate_credential(
        &self,
        credential: &CredentialMaterial,
    ) -> Result<ValidationResult, ProviderError> {
        let account = self.request(Method::GET, credential, "/me", None).await?;
        Ok(ValidationResult {
            valid: true,
            identity: account
                .get("nichandle")
                .or_else(|| account.get("email"))
                .and_then(Value::as_str)
                .map(str::to_owned),
            scopes: vec!["vps:read".to_owned(), "vps:operate".to_owned()],
        })
    }

    async fn discover_capabilities(
        &self,
        credential: &CredentialMaterial,
    ) -> Result<Vec<Capability>, ProviderError> {
        self.request(Method::GET, credential, "/vps", None).await?;
        Ok(vec![Capability::Compute])
    }

    async fn inventory(
        &self,
        credential: &CredentialMaterial,
        request: &InventoryRequest,
    ) -> Result<InventoryPage, ProviderError> {
        if request.resource_type != "compute_instance"
            || request.parent_external_id.is_some()
            || request.cursor.is_some()
        {
            return Err(ProviderError::configuration(
                "unsupported_inventory_resource_type",
                false,
            ));
        }
        let service_names = self.request(Method::GET, credential, "/vps", None).await?;
        let service_names = service_names.as_array().ok_or_else(invalid_response)?;
        let mut items = Vec::with_capacity(service_names.len());
        for service_name in service_names {
            let service_name = service_name.as_str().ok_or_else(invalid_response)?;
            items.push(normalize_vps(
                service_name,
                self.get_vps(credential, service_name).await?,
            ));
        }
        Ok(InventoryPage {
            items,
            next_cursor: None,
        })
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
        let service_name = request
            .external_id
            .as_deref()
            .ok_or_else(|| ProviderError::configuration("service_name_required", false))?;
        validate_service_name(service_name)?;
        if request.action == "get" {
            let item = normalize_vps(service_name, self.get_vps(credential, service_name).await?);
            return Ok(ProviderOperationResult {
                external_id: Some(item.external_id),
                state: item.state,
            });
        }
        let action = match request.action.as_str() {
            "start" => "start",
            "stop" => "stop",
            "reboot" => "reboot",
            _ => {
                return Err(ProviderError::configuration(
                    "unsupported_ovh_vps_action",
                    false,
                ));
            }
        };
        let task = self
            .request(
                Method::POST,
                credential,
                &format!("/vps/{service_name}/{action}"),
                None,
            )
            .await?;
        let mut item = normalize_vps(service_name, self.get_vps(credential, service_name).await?);
        if let Some(state) = item.state.as_object_mut() {
            state.insert("provider_task".to_owned(), task);
        }
        Ok(ProviderOperationResult {
            external_id: Some(item.external_id),
            state: item.state,
        })
    }
}

fn normalize_vps(service_name: &str, resource: Value) -> InventoryItem {
    let name = resource
        .get("displayName")
        .and_then(Value::as_str)
        .filter(|name| !name.is_empty())
        .or_else(|| resource.get("name").and_then(Value::as_str))
        .unwrap_or(service_name);
    let provider_state = resource.get("state").and_then(Value::as_str);
    let status = match provider_state {
        Some("running") => "running",
        Some("stopped") => "stopped",
        Some("backuping" | "installing" | "rebooting" | "stopping" | "upgrading") => "provisioning",
        Some("maintenance" | "rescued") => "active",
        Some("error") => "error",
        _ => "unknown",
    };
    InventoryItem {
        external_type: "compute_instance".to_owned(),
        external_id: service_name.to_owned(),
        name: name.to_owned(),
        state: json!({
            "name": name,
            "status": status,
            "provider_status": resource.get("state").cloned().unwrap_or(Value::Null),
            "region": resource.get("zone").cloned().unwrap_or(Value::Null),
            "cluster": resource.get("cluster").cloned().unwrap_or(Value::Null),
            "offer_type": resource.get("offerType").cloned().unwrap_or(Value::Null),
            "vcpu_count": resource.get("vcore").cloned().unwrap_or(Value::Null),
            "ram": resource.get("memoryLimit").cloned().unwrap_or(Value::Null),
            "disk": resource.pointer("/model/disk").cloned().unwrap_or(Value::Null),
            "model": resource.get("model").cloned().unwrap_or(Value::Null),
            "netboot_mode": resource.get("netbootMode").cloned().unwrap_or(Value::Null),
        }),
        metadata: resource,
    }
}

fn validate_service_name(value: &str) -> Result<(), ProviderError> {
    if !value.is_empty()
        && value.len() <= 255
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '.'))
    {
        Ok(())
    } else {
        Err(ProviderError::configuration("invalid_service_name", false))
    }
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
        http::{HeaderMap, StatusCode},
        routing::{get, post},
    };
    use tokio::net::TcpListener;

    fn vps() -> Value {
        json!({
            "name": "vps-test.vps.ovh.net", "displayName": "production-vps",
            "state": "running", "zone": "eu-west-par", "cluster": "cluster-1",
            "offerType": "vps-2020", "vcore": 2, "memoryLimit": 4096,
            "model": { "name": "VPS-2" }, "netbootMode": "local"
        })
    }

    fn assert_signed(headers: &HeaderMap) -> Result<(), StatusCode> {
        for name in [
            "x-ovh-application",
            "x-ovh-consumer",
            "x-ovh-timestamp",
            "x-ovh-signature",
        ] {
            if !headers.contains_key(name) {
                return Err(StatusCode::UNAUTHORIZED);
            }
        }
        Ok(())
    }

    async fn mock_server() -> String {
        let app = Router::new()
            .route("/auth/time", get(|| async { Json(1_700_000_000_i64) }))
            .route(
                "/me",
                get(|headers: HeaderMap| async move {
                    assert_signed(&headers)?;
                    Ok::<_, StatusCode>(Json(json!({ "nichandle": "ab12345-ovh" })))
                }),
            )
            .route(
                "/vps",
                get(|headers: HeaderMap| async move {
                    assert_signed(&headers)?;
                    Ok::<_, StatusCode>(Json(json!(["vps-test.vps.ovh.net"])))
                }),
            )
            .route(
                "/vps/{service_name}",
                get(|headers: HeaderMap| async move {
                    assert_signed(&headers)?;
                    Ok::<_, StatusCode>(Json(vps()))
                }),
            )
            .route(
                "/vps/{service_name}/start",
                post(|| async { Json(json!({ "id": 1, "state": "todo", "type": "start" })) }),
            )
            .route(
                "/vps/{service_name}/stop",
                post(|| async { Json(json!({ "id": 2, "state": "todo", "type": "stop" })) }),
            )
            .route(
                "/vps/{service_name}/reboot",
                post(|| async { Json(json!({ "id": 3, "state": "todo", "type": "reboot" })) }),
            );
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        format!("http://{address}")
    }

    fn credential() -> CredentialMaterial {
        CredentialMaterial {
            credential_type: CredentialType::OvhApplication,
            identity: Some("application-key".to_owned()),
            secret: "application-secret".to_owned(),
            consumer_key: Some("consumer-key".to_owned()),
        }
    }

    #[tokio::test]
    async fn validates_syncs_and_operates_ovh_vps() {
        let adapter = OvhAdapter::new(mock_server().await);
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
        assert_eq!(page.items[0].external_id, "vps-test.vps.ovh.net");
        for action in ["get", "start", "stop", "reboot"] {
            let result = adapter
                .execute(
                    &credential,
                    &ProviderOperationRequest {
                        action: action.to_owned(),
                        resource_type: "compute_instance".to_owned(),
                        external_id: Some("vps-test.vps.ovh.net".to_owned()),
                        parameters: json!({}),
                        idempotency_key: format!("ovh-{action}-1"),
                    },
                )
                .await
                .unwrap();
            assert_eq!(result.external_id.as_deref(), Some("vps-test.vps.ovh.net"));
        }
    }
}
