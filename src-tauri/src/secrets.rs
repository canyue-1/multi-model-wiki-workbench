use std::collections::HashMap;
use std::sync::Mutex;

use thiserror::Error;

use crate::domain::ProviderKind;

const SERVICE_NAME: &str = "multimodel-wiki-workbench";

#[derive(Debug, Error)]
pub enum SecretError {
    #[error("credential store failed: {0}")]
    Keyring(#[from] keyring::Error),
    #[error("in-memory credential store lock was poisoned")]
    Poisoned,
}

pub trait SecretStore: Send + Sync {
    fn save(&self, provider: ProviderKind, value: &str) -> Result<(), SecretError>;
    fn load(&self, provider: ProviderKind) -> Result<Option<String>, SecretError>;
    fn delete(&self, provider: ProviderKind) -> Result<(), SecretError>;
}

pub struct SystemSecretStore;

impl SecretStore for SystemSecretStore {
    fn save(&self, provider: ProviderKind, value: &str) -> Result<(), SecretError> {
        entry(provider)?.set_password(value)?;
        Ok(())
    }

    fn load(&self, provider: ProviderKind) -> Result<Option<String>, SecretError> {
        match entry(provider)?.get_password() {
            Ok(value) => Ok(Some(value)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    fn delete(&self, provider: ProviderKind) -> Result<(), SecretError> {
        match entry(provider)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(error.into()),
        }
    }
}

fn entry(provider: ProviderKind) -> Result<keyring::Entry, keyring::Error> {
    keyring::Entry::new(SERVICE_NAME, provider.as_str())
}

#[derive(Default)]
pub struct MemorySecretStore {
    values: Mutex<HashMap<ProviderKind, String>>,
}

impl SecretStore for MemorySecretStore {
    fn save(&self, provider: ProviderKind, value: &str) -> Result<(), SecretError> {
        self.values
            .lock()
            .map_err(|_| SecretError::Poisoned)?
            .insert(provider, value.to_owned());
        Ok(())
    }

    fn load(&self, provider: ProviderKind) -> Result<Option<String>, SecretError> {
        Ok(self
            .values
            .lock()
            .map_err(|_| SecretError::Poisoned)?
            .get(&provider)
            .cloned())
    }

    fn delete(&self, provider: ProviderKind) -> Result<(), SecretError> {
        self.values
            .lock()
            .map_err(|_| SecretError::Poisoned)?
            .remove(&provider);
        Ok(())
    }
}
