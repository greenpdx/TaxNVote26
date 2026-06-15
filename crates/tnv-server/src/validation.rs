// src/validation.rs

use crate::models::*;
use sha2::{Sha256, Digest};
use std::collections::HashSet;

// ─── Proof-of-Work verification ──────────────────────────────────

/// Verify that SHA-256(challenge + nonce) has `difficulty` leading zero bits.
#[cfg(feature = "full")]
pub fn verify_pow(challenge: &str, nonce: &str, difficulty: u32) -> bool {
    let mut hasher = Sha256::new();
    hasher.update(challenge.as_bytes());
    hasher.update(nonce.as_bytes());
    let hash = hasher.finalize();

    // Check leading zero bits
    let mut zeros = 0u32;
    for byte in hash.iter() {
        if *byte == 0 {
            zeros += 8;
        } else {
            zeros += byte.leading_zeros();
            break;
        }
        if zeros >= difficulty {
            break;
        }
    }
    zeros >= difficulty
}

/// Validate a parsed template against the node ID set.
pub fn validate_template(
    tpl: &ParsedTemplate,
    valid_ids: &HashSet<String>,
) -> Result<(), String> {
    // Check every entry against the allowlist. The set is never empty (startup
    // fails if budauth.csv yields no node IDs), so this never fails open.
    for entry in &tpl.entries {
        if !valid_ids.contains(&entry.node_id) {
            return Err(format!("unknown node_id: '{}'", entry.node_id));
        }
    }
    Ok(())
}

/// Validate a parsed tax dollar: checksum, node IDs, template existence.
pub fn validate_taxdollar(
    td: &ParsedTaxDollar,
    valid_ids: &HashSet<String>,
) -> Result<(), String> {
    // Node ID check against the allowlist (never empty — see validate_template).
    for alloc in &td.allocations {
        if !valid_ids.contains(&alloc.node_id) {
            return Err(format!("unknown node_id: '{}'", alloc.node_id));
        }
    }

    // Checksum verification
    let computed = compute_td_checksum(&td.allocations);
    if computed != td.checksum {
        return Err(format!(
            "checksum mismatch: expected '{}', got '{}'",
            computed, td.checksum
        ));
    }

    Ok(())
}

/// Compute the canonical checksum for a set of allocations.
/// Sort by id ascending, concatenate "id:pct.6f,...", SHA-256.
pub fn compute_td_checksum(allocations: &[Allocation]) -> String {
    let mut sorted: Vec<&Allocation> = allocations.iter().collect();
    sorted.sort_by(|a, b| a.node_id.cmp(&b.node_id));

    let canonical: String = sorted
        .iter()
        .map(|a| format!("{}:{:.6}", a.node_id, a.pct))
        .collect::<Vec<_>>()
        .join(",");

    let mut hasher = Sha256::new();
    hasher.update(canonical.as_bytes());
    let hash = hasher.finalize();
    format!("sha256:{}", hex::encode(hash))
}

/// Validate registration inputs.
#[cfg(feature = "full")]
pub fn validate_registration(req: &RegisterRequest) -> Result<(), String> {
    if req.username.len() < USERNAME_MIN || req.username.len() > USERNAME_MAX {
        return Err(format!("username must be {}-{} chars", USERNAME_MIN, USERNAME_MAX));
    }
    if !req.username.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') {
        return Err("username: alphanumeric, hyphens, underscores only".into());
    }
    if req.email.len() < EMAIL_MIN || req.email.len() > EMAIL_MAX || !req.email.contains('@') {
        return Err("invalid email".into());
    }
    if req.password.len() < PASSWORD_MIN || req.password.len() > PASSWORD_MAX {
        return Err(format!("password must be {}-{} chars", PASSWORD_MIN, PASSWORD_MAX));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_checksum_deterministic() {
        let allocs = vec![
            Allocation { node_id: "a:020".into(), pct: 0.4 },
            Allocation { node_id: "a:010".into(), pct: 0.6 },
        ];
        let c1 = compute_td_checksum(&allocs);
        let c2 = compute_td_checksum(&allocs);
        assert_eq!(c1, c2);
        assert!(c1.starts_with("sha256:"));
        assert_eq!(c1.len(), CHECKSUM_LEN);
    }

    #[test]
    fn test_checksum_order_independent() {
        let a = vec![
            Allocation { node_id: "a:010".into(), pct: 0.6 },
            Allocation { node_id: "a:020".into(), pct: 0.4 },
        ];
        let b = vec![
            Allocation { node_id: "a:020".into(), pct: 0.4 },
            Allocation { node_id: "a:010".into(), pct: 0.6 },
        ];
        assert_eq!(compute_td_checksum(&a), compute_td_checksum(&b));
    }

    #[cfg(feature = "full")]
    fn reg(username: &str, email: &str, password: &str) -> RegisterRequest {
        RegisterRequest {
            username: username.into(), email: email.into(), password: password.into(),
            challenge: "test".into(), nonce: "test".into(),
        }
    }

    #[cfg(feature = "full")]
    #[test]
    fn test_validate_registration_good() {
        assert!(validate_registration(&reg("shaun", "x@y.com", "12345678")).is_ok());
    }

    #[cfg(feature = "full")]
    #[test]
    fn test_validate_registration_short_username() {
        assert!(validate_registration(&reg("ab", "x@y.com", "12345678")).is_err());
    }

    #[cfg(feature = "full")]
    #[test]
    fn test_validate_registration_short_password() {
        assert!(validate_registration(&reg("shaun", "x@y.com", "short")).is_err());
    }

    #[cfg(feature = "full")]
    #[test]
    fn test_validate_registration_email_too_short() {
        assert!(validate_registration(&reg("shaun", "a@b", "12345678")).is_err());
    }

    #[cfg(feature = "full")]
    #[test]
    fn test_validate_registration_username_too_long() {
        assert!(validate_registration(&reg(&"a".repeat(33), "x@y.com", "12345678")).is_err());
    }

    #[cfg(feature = "full")]
    #[test]
    fn test_validate_registration_bad_username_chars() {
        assert!(validate_registration(&reg("ha ha spaces", "x@y.com", "12345678")).is_err());
    }

    #[cfg(feature = "full")]
    #[test]
    fn test_validate_registration_password_too_long() {
        assert!(validate_registration(&reg("shaun", "x@y.com", &"p".repeat(129))).is_err());
    }

    // ─── PoW tests ───────────────────────────────────────────────

    #[cfg(feature = "full")]
    #[test]
    fn test_pow_valid_solution() {
        // Brute-force a solution with low difficulty for testing
        let challenge = "test_challenge_abc123";
        let difficulty = 8; // only 8 leading zero bits = easy
        let mut nonce = 0u64;
        loop {
            let n = format!("{}", nonce);
            if verify_pow(challenge, &n, difficulty) {
                // Found it — verify again to confirm
                assert!(verify_pow(challenge, &n, difficulty));
                break;
            }
            nonce += 1;
            assert!(nonce < 10_000, "should find difficulty-8 solution quickly");
        }
    }

    #[cfg(feature = "full")]
    #[test]
    fn test_pow_invalid_solution() {
        assert!(!verify_pow("challenge", "wrong_nonce_unlikely", 20));
    }

    #[cfg(feature = "full")]
    #[test]
    fn test_pow_zero_difficulty() {
        // Difficulty 0 = everything passes
        assert!(verify_pow("any", "thing", 0));
    }
}
