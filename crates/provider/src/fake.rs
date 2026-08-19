use crate::{
    Capability, CredentialMaterial, InventoryItem, InventoryPage, InventoryRequest,
    ProviderAdapter, ProviderError, ProviderErrorCategory, ProviderKind, ProviderOperationRequest,
    ProviderOperationResult, ValidationResult,
};
use async_trait::async_trait;
use serde_json::json;

#[derive(Clone, Default)]
pub struct FakeProviderAdapter;

#[async_trait]
impl ProviderAdapter for FakeProviderAdapter {
    fn kind(&self) -> ProviderKind {
        ProviderKind::parse("fake").expect("static provider kind")
    }

    async fn validate_credential(
        &self,
        credential: &CredentialMaterial,
    ) -> Result<ValidationResult, ProviderError> {
        if credential.secret == "valid-fake-token" {
            Ok(ValidationResult {
                valid: true,
                identity: Some("fake-account".to_owned()),
                scopes: vec!["fake:*".to_owned()],
            })
        } else {
            Err(ProviderError {
                category: ProviderErrorCategory::Authentication,
                code: "invalid_credential".to_owned(),
                retryable: false,
                retry_after_seconds: None,
                safe_message: "provider credential was rejected".to_owned(),
            })
        }
    }

    async fn discover_capabilities(
        &self,
        credential: &CredentialMaterial,
    ) -> Result<Vec<Capability>, ProviderError> {
        self.validate_credential(credential).await?;
        Ok(vec![
            Capability::Compute,
            Capability::Dns,
            Capability::Firewall,
            Capability::Certificate,
        ])
    }

    async fn inventory(
        &self,
        credential: &CredentialMaterial,
        request: &InventoryRequest,
    ) -> Result<InventoryPage, ProviderError> {
        self.validate_credential(credential).await?;
        Ok(InventoryPage {
            items: vec![InventoryItem {
                external_type: request.resource_type.clone(),
                external_id: "fake-resource-1".to_owned(),
                name: "Fake Resource".to_owned(),
                state: json!({ "status": "active" }),
                metadata: json!({ "provider": "fake" }),
            }],
            next_cursor: None,
        })
    }

    async fn execute(
        &self,
        credential: &CredentialMaterial,
        request: &ProviderOperationRequest,
    ) -> Result<ProviderOperationResult, ProviderError> {
        self.validate_credential(credential).await?;
        Ok(ProviderOperationResult {
            external_id: request
                .external_id
                .clone()
                .or_else(|| Some("fake-resource-created".to_owned())),
            state: json!({ "action": request.action, "status": "succeeded" }),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ProviderRegistry, ProviderRuntime};
    use std::sync::Arc;

    #[tokio::test]
    async fn fake_adapter_covers_credential_inventory_and_operation() {
        let runtime = ProviderRuntime::new(ProviderRegistry::new([
            Arc::new(FakeProviderAdapter) as Arc<dyn ProviderAdapter>
        ]));
        let kind = ProviderKind::parse("fake").unwrap();
        let credential = CredentialMaterial {
            credential_type: crate::CredentialType::ApiToken,
            identity: None,
            secret: "valid-fake-token".to_owned(),
            consumer_key: None,
        };
        assert!(
            runtime
                .validate_and_discover(&kind, &credential)
                .await
                .unwrap()
                .0
                .valid
        );
        assert_eq!(
            runtime
                .sync_inventory(
                    &kind,
                    &credential,
                    &InventoryRequest {
                        resource_type: "compute".to_owned(),
                        parent_external_id: None,
                        cursor: None,
                    },
                )
                .await
                .unwrap()
                .items
                .len(),
            1
        );
        let result = runtime
            .execute(
                &kind,
                &credential,
                &ProviderOperationRequest {
                    action: "create".to_owned(),
                    resource_type: "compute".to_owned(),
                    external_id: None,
                    parameters: json!({}),
                    idempotency_key: "fake-create-1".to_owned(),
                },
            )
            .await
            .unwrap();
        assert_eq!(result.external_id.as_deref(), Some("fake-resource-created"));
    }
}
