// src/auth.rs

use crate::models::Claims;
use argon2::{self, Argon2, PasswordHasher, PasswordVerifier, password_hash::{SaltString, rand_core::OsRng}};
use jsonwebtoken::{encode, decode, Header, Validation, EncodingKey, DecodingKey};
use sha2::{Sha256, Digest};

/// Hash a password with argon2id.
pub fn hash_password(password: &str) -> Result<String, String> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| format!("hash error: {}", e))?;
    Ok(hash.to_string())
}

/// Verify a password against an argon2id hash.
pub fn verify_password(password: &str, hash: &str) -> bool {
    let parsed = match argon2::PasswordHash::new(hash) {
        Ok(h) => h,
        Err(_) => return false,
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}

/// Hash an email address with SHA-256 (lowercase, trimmed).
pub fn hash_email(email: &str) -> String {
    let normalized = email.trim().to_lowercase();
    let mut hasher = Sha256::new();
    hasher.update(normalized.as_bytes());
    hex::encode(hasher.finalize())
}

/// Hash a demo person's secret PIN with SHA-256 over "name:pin".
/// Demo-only — a 4-digit PIN is trivially brute-forceable.
pub fn hash_secret(name: &str, pin: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(name.trim().as_bytes());
    hasher.update(b":");
    hasher.update(pin.as_bytes());
    hex::encode(hasher.finalize())
}

/// Create a JWT token for an authenticated user.
pub fn create_jwt(account_id: i64, username: &str, tier: i32, secret: &str) -> Result<String, String> {
    let exp = chrono::Utc::now().timestamp() + 86400; // 24 hours
    let claims = Claims {
        sub: account_id,
        username: username.to_string(),
        tier,
        exp,
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| format!("JWT encode error: {}", e))
}

/// Verify and decode a JWT token.
pub fn verify_jwt(token: &str, secret: &str) -> Result<Claims, String> {
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )
    .map(|data| data.claims)
    .map_err(|e| format!("JWT error: {}", e))
}

/// Generate a 6-digit verification code.
pub fn generate_verification_code() -> String {
    use rand::Rng;
    let code: u32 = rand::thread_rng().gen_range(100_000..999_999);
    format!("{}", code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_password_hash_verify() {
        let hash = hash_password("testpass123").unwrap();
        assert!(verify_password("testpass123", &hash));
        assert!(!verify_password("wrongpass", &hash));
    }

    #[test]
    fn test_email_hash_consistent() {
        let h1 = hash_email("Test@Example.COM");
        let h2 = hash_email("test@example.com");
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 64);
    }

    #[test]
    fn test_jwt_roundtrip() {
        let secret = "test-secret-key";
        let token = create_jwt(42, "shaun", 0, secret).unwrap();
        let claims = verify_jwt(&token, secret).unwrap();
        assert_eq!(claims.sub, 42);
        assert_eq!(claims.username, "shaun");
    }

    #[test]
    fn test_jwt_bad_secret() {
        let token = create_jwt(1, "x", 0, "secret1").unwrap();
        assert!(verify_jwt(&token, "secret2").is_err());
    }

    #[test]
    fn test_verification_code() {
        let code = generate_verification_code();
        assert_eq!(code.len(), 6);
        assert!(code.parse::<u32>().is_ok());
    }
}
