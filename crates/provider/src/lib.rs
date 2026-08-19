mod cloudflare;
mod crypto;
mod fake;

pub use cloudflare::CloudflareAdapter;
pub use crypto::{EncryptedCredential, EnvelopeCipher};
pub use fake::FakeProviderAdapter;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::HashMap, fmt, sync::Arc};
use thiserror::Error;
use zeroize::{Zeroize, ZeroizeOnDrop};

#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProviderKind(String);

impl ProviderKind {
    /// Parses the stable lowercase adapter identifier.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError`] when the identifier is empty, too long, or malformed.
    pub fn parse(value: impl Into<String>) -> Result<Self, ProviderError> {
        let value = value.into();
        let valid = !value.is_empty()
            && value.len() <= 64
            && value
                .chars()
                .all(|character| character.is_ascii_lowercase() || character == '-');
        valid
            .then_some(Self(value))
            .ok_or_else(|| ProviderError::configuration("invalid_provider_kind", false))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ProviderKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    Compute,
    Dns,
    Firewall,
    Certificate,
}

#[derive(Clone, Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
pub struct CredentialMaterial {
    pub secret: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ValidationResult {
    pub valid: bool,
    pub identity: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InventoryItem {
    pub external_type: String,
    pub external_id: String,
    pub name: String,
    pub state: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProviderOperationRequest {
    pub action: String,
    pub resource_type: String,
    pub external_id: Option<String>,
    pub parameters: Value,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProviderOperationResult {
    pub external_id: Option<String>,
    pub state: Value,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderErrorCategory {
    Authentication,
    Authorization,
    RateLimited,
    NotFound,
    Conflict,
    InvalidRequest,
    Unavailable,
    Configuration,
    Unknown,
}

#[derive(Clone, Debug, Error, Serialize, Deserialize)]
#[error("provider error {code} ({category:?})")]
pub struct ProviderError {
    pub category: ProviderErrorCategory,
    pub code: String,
    pub retryable: bool,
    pub retry_after_seconds: Option<u64>,
    #[serde(skip_serializing)]
    pub safe_message: String,
}

impl ProviderError {
    #[must_use]
    pub fn configuration(code: impl Into<String>, retryable: bool) -> Self {
        Self {
            category: ProviderErrorCategory::Configuration,
            code: code.into(),
            retryable,
            retry_after_seconds: None,
            safe_message: "provider configuration is invalid".to_owned(),
        }
    }
}

#[async_trait]
pub trait ProviderAdapter: Send + Sync {
    fn kind(&self) -> ProviderKind;
    async fn validate_credential(
        &self,
        credential: &CredentialMaterial,
    ) -> Result<ValidationResult, ProviderError>;
    async fn discover_capabilities(
        &self,
        credential: &CredentialMaterial,
    ) -> Result<Vec<Capability>, ProviderError>;
    async fn inventory(
        &self,
        credential: &CredentialMaterial,
        resource_type: &str,
    ) -> Result<Vec<InventoryItem>, ProviderError>;
    async fn execute(
        &self,
        credential: &CredentialMaterial,
        request: &ProviderOperationRequest,
    ) -> Result<ProviderOperationResult, ProviderError>;
}

#[derive(Clone, Default)]
pub struct ProviderRegistry {
    adapters: HashMap<ProviderKind, Arc<dyn ProviderAdapter>>,
}

#[derive(Clone)]
pub struct ProviderRuntime {
    registry: ProviderRegistry,
}

impl ProviderRuntime {
    #[must_use]
    pub const fn new(registry: ProviderRegistry) -> Self {
        Self { registry }
    }

    /// Validates credentials and discovers capabilities through the registered adapter.
    ///
    /// # Errors
    ///
    /// Returns the adapter's normalized [`ProviderError`].
    pub async fn validate_and_discover(
        &self,
        kind: &ProviderKind,
        credential: &CredentialMaterial,
    ) -> Result<(ValidationResult, Vec<Capability>), ProviderError> {
        let adapter = self.registry.get(kind)?;
        let validation = adapter.validate_credential(credential).await?;
        let capabilities = adapter.discover_capabilities(credential).await?;
        Ok((validation, capabilities))
    }

    /// Runs an inventory request through the registered adapter.
    ///
    /// # Errors
    ///
    /// Returns the adapter's normalized [`ProviderError`].
    pub async fn sync_inventory(
        &self,
        kind: &ProviderKind,
        credential: &CredentialMaterial,
        resource_type: &str,
    ) -> Result<Vec<InventoryItem>, ProviderError> {
        self.registry
            .get(kind)?
            .inventory(credential, resource_type)
            .await
    }

    /// Runs a lifecycle operation through the registered adapter.
    ///
    /// # Errors
    ///
    /// Returns the adapter's normalized [`ProviderError`].
    pub async fn execute(
        &self,
        kind: &ProviderKind,
        credential: &CredentialMaterial,
        request: &ProviderOperationRequest,
    ) -> Result<ProviderOperationResult, ProviderError> {
        self.registry.get(kind)?.execute(credential, request).await
    }
}

impl ProviderRegistry {
    #[must_use]
    pub fn new(adapters: impl IntoIterator<Item = Arc<dyn ProviderAdapter>>) -> Self {
        let adapters = adapters
            .into_iter()
            .map(|adapter| (adapter.kind(), adapter))
            .collect();
        Self { adapters }
    }

    /// Resolves an adapter without coupling callers to concrete providers.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError`] when no adapter is registered for the kind.
    pub fn get(&self, kind: &ProviderKind) -> Result<Arc<dyn ProviderAdapter>, ProviderError> {
        self.adapters.get(kind).cloned().ok_or_else(|| {
            ProviderError::configuration(format!("unsupported_provider_{kind}"), false)
        })
    }

    #[must_use]
    pub fn kinds(&self) -> Vec<ProviderKind> {
        self.adapters.keys().cloned().collect()
    }
}

#[async_trait]
pub trait SyncFramework: Send + Sync {
    async fn sync_inventory(
        &self,
        adapter: &dyn ProviderAdapter,
        credential: &CredentialMaterial,
        resource_type: &str,
    ) -> Result<Vec<InventoryItem>, ProviderError> {
        adapter.inventory(credential, resource_type).await
    }
}

#[async_trait]
pub trait OperationAdapterFramework: Send + Sync {
    async fn execute_operation(
        &self,
        adapter: &dyn ProviderAdapter,
        credential: &CredentialMaterial,
        request: &ProviderOperationRequest,
    ) -> Result<ProviderOperationResult, ProviderError> {
        adapter.execute(credential, request).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_kind_is_stable_and_extensible() {
        assert!(ProviderKind::parse("cloudflare").is_ok());
        assert!(ProviderKind::parse("future-provider").is_ok());
        assert!(ProviderKind::parse("Cloudflare").is_err());
    }
}
