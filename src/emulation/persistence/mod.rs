use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

use crate::emulation::errors::{EmulationError, Result};

/// Session data containing authentication and TLS state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionData {
    /// HTTP cookies
    pub cookies: Vec<Cookie>,
    /// Authentication token
    pub auth_token: String,
    /// Optional refresh token
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
    /// TLS session ticket for session resumption
    pub tls_session_ticket: Vec<u8>,
    /// User-Agent string used for this session
    pub user_agent: String,
    /// Session creation timestamp
    pub created_at: DateTime<Utc>,
    /// Session expiration timestamp
    pub expires_at: DateTime<Utc>,
}

/// HTTP Cookie
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cookie {
    pub name: String,
    pub value: String,
    pub domain: String,
    pub path: String,
    pub secure: bool,
    pub http_only: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires: Option<DateTime<Utc>>,
}

impl SessionData {
    /// Create a new session data with the given parameters
    pub fn new(
        cookies: Vec<Cookie>,
        auth_token: String,
        tls_session_ticket: Vec<u8>,
        user_agent: String,
        duration_hours: u64,
    ) -> Self {
        let created_at = Utc::now();
        let expires_at = created_at + Duration::hours(duration_hours as i64);

        Self {
            cookies,
            auth_token,
            refresh_token: None,
            tls_session_ticket,
            user_agent,
            created_at,
            expires_at,
        }
    }

    /// Check if the session is still valid
    pub fn is_valid(&self) -> bool {
        Utc::now() < self.expires_at
    }

    /// Check if the session is close to expiring (within threshold)
    pub fn is_expiring_soon(&self, minutes_threshold: u64) -> bool {
        let threshold = Utc::now() + Duration::minutes(minutes_threshold as i64);
        self.expires_at < threshold
    }

    /// Get remaining session lifetime
    pub fn remaining_lifetime(&self) -> Duration {
        self.expires_at - Utc::now()
    }
}

/// Session persistence manager for encrypted storage
pub struct SessionPersistence {
    storage_path: PathBuf,
    encryption_key: [u8; 32],
}

impl SessionPersistence {
    /// Create a new session persistence manager
    ///
    /// # Arguments
    /// * `storage_path` - Path to the encrypted session file
    /// * `encryption_key` - 32-byte AES-256 key for encryption
    pub fn new(storage_path: PathBuf, encryption_key: [u8; 32]) -> Self {
        Self {
            storage_path,
            encryption_key,
        }
    }

    /// Create from configuration, loading encryption key from environment
    pub fn from_config(config: &crate::emulation::config::EmulationConfig) -> Result<Self> {
        let key_env = &config.encryption_key_env;
        let key_hex = std::env::var(key_env).map_err(|_| {
            EmulationError::ConfigError(format!(
                "Encryption key not found in environment variable: {}",
                key_env
            ))
        })?;

        let key_bytes = hex::decode(&key_hex).map_err(|e| {
            EmulationError::ConfigError(format!("Invalid encryption key hex: {}", e))
        })?;

        if key_bytes.len() != 32 {
            return Err(EmulationError::ConfigError(format!(
                "Encryption key must be 32 bytes, got {}",
                key_bytes.len()
            )));
        }

        let mut key = [0u8; 32];
        key.copy_from_slice(&key_bytes);

        Ok(Self::new(config.session_storage_path.clone(), key))
    }

    /// Generate a random encryption key (for initial setup)
    pub fn generate_key() -> [u8; 32] {
        use rand::RngCore;
        let mut key = [0u8; 32];
        OsRng.fill_bytes(&mut key);
        key
    }

    /// Save session data to encrypted file
    pub fn save_session(&self, data: &SessionData) -> Result<()> {
        info!("Saving session to {:?}", self.storage_path);

        // Serialize to JSON
        let json = serde_json::to_vec(data).map_err(|e| {
            EmulationError::SerializationError(e)
        })?;

        // Encrypt with AES-256-GCM
        let cipher = Aes256Gcm::new(&self.encryption_key.into());
        
        // Generate random nonce (12 bytes for GCM)
        let mut nonce_bytes = [0u8; 12];
        use rand::RngCore;
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = cipher.encrypt(nonce, json.as_ref()).map_err(|e| {
            EmulationError::EncryptionError(format!("Encryption failed: {}", e))
        })?;

        // Prepend nonce to ciphertext
        let mut output = nonce_bytes.to_vec();
        output.extend_from_slice(&ciphertext);

        // Ensure parent directory exists
        if let Some(parent) = self.storage_path.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                EmulationError::IoError(e)
            })?;
        }

        // Write to file
        fs::write(&self.storage_path, output).map_err(|e| {
            EmulationError::IoError(e)
        })?;

        debug!(
            "Session saved successfully, expires at: {}",
            data.expires_at
        );

        Ok(())
    }

    /// Load session data from encrypted file
    pub fn load_session(&self) -> Result<Option<SessionData>> {
        if !self.storage_path.exists() {
            debug!("Session file does not exist: {:?}", self.storage_path);
            return Ok(None);
        }

        info!("Loading session from {:?}", self.storage_path);

        // Read encrypted file
        let encrypted = fs::read(&self.storage_path).map_err(|e| {
            EmulationError::IoError(e)
        })?;

        if encrypted.len() < 12 {
            warn!("Session file too short, corrupted");
            return Ok(None);
        }

        // Extract nonce and ciphertext
        let (nonce_bytes, ciphertext) = encrypted.split_at(12);
        let nonce = Nonce::from_slice(nonce_bytes);

        // Decrypt
        let cipher = Aes256Gcm::new(&self.encryption_key.into());
        let plaintext = cipher.decrypt(nonce, ciphertext).map_err(|e| {
            EmulationError::EncryptionError(format!("Decryption failed: {}", e))
        })?;

        // Deserialize
        let data: SessionData = serde_json::from_slice(&plaintext).map_err(|e| {
            EmulationError::SerializationError(e)
        })?;

        debug!(
            "Session loaded, created: {}, expires: {}",
            data.created_at, data.expires_at
        );

        Ok(Some(data))
    }

    /// Check if a saved session exists and is valid
    pub fn is_session_valid(&self) -> bool {
        match self.load_session() {
            Ok(Some(data)) => data.is_valid(),
            _ => false,
        }
    }

    /// Delete the session file
    pub fn delete_session(&self) -> Result<()> {
        if self.storage_path.exists() {
            fs::remove_file(&self.storage_path).map_err(|e| {
                EmulationError::IoError(e)
            })?;
            info!("Session file deleted: {:?}", self.storage_path);
        }
        Ok(())
    }

    /// Get the storage path
    pub fn storage_path(&self) -> &Path {
        &self.storage_path
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_session() -> SessionData {
        SessionData::new(
            vec![Cookie {
                name: "session_id".to_string(),
                value: "test123".to_string(),
                domain: "example.com".to_string(),
                path: "/".to_string(),
                secure: true,
                http_only: true,
                expires: None,
            }],
            "auth_token_123".to_string(),
            vec![1, 2, 3, 4, 5],
            "Mozilla/5.0".to_string(),
            24,
        )
    }

    #[test]
    fn test_session_validity() {
        let session = create_test_session();
        assert!(session.is_valid());
        assert!(!session.is_expiring_soon(30));
    }

    #[test]
    fn test_expired_session() {
        let mut session = create_test_session();
        session.expires_at = Utc::now() - Duration::hours(1);
        assert!(!session.is_valid());
    }

    #[test]
    fn test_expiring_soon() {
        let mut session = create_test_session();
        session.expires_at = Utc::now() + Duration::minutes(15);
        assert!(session.is_expiring_soon(30));
        assert!(!session.is_expiring_soon(10));
    }

    #[test]
    fn test_save_and_load_session() {
        let temp_dir = TempDir::new().unwrap();
        let storage_path = temp_dir.path().join("session.enc");
        let key = SessionPersistence::generate_key();

        let persistence = SessionPersistence::new(storage_path, key);
        let session = create_test_session();

        // Save
        persistence.save_session(&session).unwrap();

        // Load
        let loaded = persistence.load_session().unwrap().unwrap();

        // Verify
        assert_eq!(loaded.auth_token, session.auth_token);
        assert_eq!(loaded.cookies.len(), session.cookies.len());
        assert_eq!(loaded.cookies[0].name, session.cookies[0].name);
        assert_eq!(loaded.tls_session_ticket, session.tls_session_ticket);
    }

    #[test]
    fn test_load_nonexistent_session() {
        let temp_dir = TempDir::new().unwrap();
        let storage_path = temp_dir.path().join("nonexistent.enc");
        let key = SessionPersistence::generate_key();

        let persistence = SessionPersistence::new(storage_path, key);
        let loaded = persistence.load_session().unwrap();

        assert!(loaded.is_none());
    }

    #[test]
    fn test_is_session_valid() {
        let temp_dir = TempDir::new().unwrap();
        let storage_path = temp_dir.path().join("session.enc");
        let key = SessionPersistence::generate_key();

        let persistence = SessionPersistence::new(storage_path, key);

        // No session yet
        assert!(!persistence.is_session_valid());

        // Save valid session
        let session = create_test_session();
        persistence.save_session(&session).unwrap();
        assert!(persistence.is_session_valid());

        // Save expired session
        let mut expired_session = create_test_session();
        expired_session.expires_at = Utc::now() - Duration::hours(1);
        persistence.save_session(&expired_session).unwrap();
        assert!(!persistence.is_session_valid());
    }

    #[test]
    fn test_delete_session() {
        let temp_dir = TempDir::new().unwrap();
        let storage_path = temp_dir.path().join("session.enc");
        let key = SessionPersistence::generate_key();

        let persistence = SessionPersistence::new(storage_path.clone(), key);
        let session = create_test_session();

        // Save and verify exists
        persistence.save_session(&session).unwrap();
        assert!(storage_path.exists());

        // Delete and verify removed
        persistence.delete_session().unwrap();
        assert!(!storage_path.exists());
    }

    #[test]
    fn test_encryption_with_different_keys() {
        let temp_dir = TempDir::new().unwrap();
        let storage_path = temp_dir.path().join("session.enc");
        let key1 = SessionPersistence::generate_key();
        let key2 = SessionPersistence::generate_key();

        let persistence1 = SessionPersistence::new(storage_path.clone(), key1);
        let persistence2 = SessionPersistence::new(storage_path, key2);

        let session = create_test_session();
        persistence1.save_session(&session).unwrap();

        // Loading with different key should fail
        let result = persistence2.load_session();
        assert!(result.is_err());
    }
}
