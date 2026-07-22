#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SecretKind {
    Password,
    KeyPassphrase,
}

pub trait SecretStore: Send + Sync {
    fn get(&self, profile: &str, kind: SecretKind) -> Option<String>;
    fn set(&self, profile: &str, kind: SecretKind, value: &str) -> Result<(), String>;
    fn delete(&self, profile: &str, kind: SecretKind) -> Result<(), String>;
}

pub struct KeyringStore;

impl SecretStore for KeyringStore {
    fn get(&self, profile: &str, kind: SecretKind) -> Option<String> {
        let key = format!("{profile}:{kind:?}");
        match keyring::Entry::new("claudedeck", &key) {
            Ok(entry) => entry.get_password().ok(),
            Err(_) => None,
        }
    }

    fn set(&self, profile: &str, kind: SecretKind, value: &str) -> Result<(), String> {
        let key = format!("{profile}:{kind:?}");
        let entry = keyring::Entry::new("claudedeck", &key).map_err(|e| e.to_string())?;
        entry.set_password(value).map_err(|e| e.to_string())
    }

    fn delete(&self, profile: &str, kind: SecretKind) -> Result<(), String> {
        let key = format!("{profile}:{kind:?}");
        let entry = keyring::Entry::new("claudedeck", &key).map_err(|e| e.to_string())?;
        entry.delete_credential().map_err(|e| e.to_string())
    }
}

pub struct MemoryStore(std::sync::Mutex<std::collections::HashMap<(String, SecretKind), String>>);

impl Default for MemoryStore {
    fn default() -> Self {
        Self::new()
    }
}

impl MemoryStore {
    pub fn new() -> Self {
        MemoryStore(std::sync::Mutex::new(std::collections::HashMap::new()))
    }
}

impl SecretStore for MemoryStore {
    fn get(&self, profile: &str, kind: SecretKind) -> Option<String> {
        let store = self.0.lock().unwrap();
        store.get(&(profile.to_string(), kind)).cloned()
    }

    fn set(&self, profile: &str, kind: SecretKind, value: &str) -> Result<(), String> {
        let mut store = self.0.lock().unwrap();
        store.insert((profile.to_string(), kind), value.to_string());
        Ok(())
    }

    fn delete(&self, profile: &str, kind: SecretKind) -> Result<(), String> {
        let mut store = self.0.lock().unwrap();
        store.remove(&(profile.to_string(), kind));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_set_and_get() {
        let store = MemoryStore::new();
        let result = store.set("myprofile", SecretKind::Password, "secret123");
        assert!(result.is_ok());

        let retrieved = store.get("myprofile", SecretKind::Password);
        assert_eq!(retrieved, Some("secret123".to_string()));
    }

    #[test]
    fn test_get_nonexistent() {
        let store = MemoryStore::new();
        let result = store.get("nonexistent", SecretKind::Password);
        assert_eq!(result, None);
    }

    #[test]
    fn test_delete() {
        let store = MemoryStore::new();
        store
            .set("myprofile", SecretKind::Password, "secret123")
            .unwrap();

        let retrieved = store.get("myprofile", SecretKind::Password);
        assert_eq!(retrieved, Some("secret123".to_string()));

        let delete_result = store.delete("myprofile", SecretKind::Password);
        assert!(delete_result.is_ok());

        let after_delete = store.get("myprofile", SecretKind::Password);
        assert_eq!(after_delete, None);
    }

    #[test]
    fn test_separate_slots() {
        let store = MemoryStore::new();
        store
            .set("myprofile", SecretKind::Password, "pwd_secret")
            .unwrap();
        store
            .set("myprofile", SecretKind::KeyPassphrase, "key_secret")
            .unwrap();

        let pwd = store.get("myprofile", SecretKind::Password);
        assert_eq!(pwd, Some("pwd_secret".to_string()));

        let key = store.get("myprofile", SecretKind::KeyPassphrase);
        assert_eq!(key, Some("key_secret".to_string()));
    }
}
