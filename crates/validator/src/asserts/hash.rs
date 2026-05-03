use sha2::{Digest, Sha256};

pub fn check(path: &str, expected_sha256: &str) -> Result<(), String> {
    let data = std::fs::read(path).map_err(|e| format!("hash: failed to read '{path}': {e}"))?;
    let hash = Sha256::digest(&data);
    let hex_hash = hex::encode(hash);
    if hex_hash.eq_ignore_ascii_case(expected_sha256) {
        Ok(())
    } else {
        Err(format!(
            "hash: SHA256 mismatch for '{path}'\n  expected: {expected_sha256}\n  actual:   {hex_hash}"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_known_sha256() {
        let path = "/tmp/validator_test_hash.txt";
        std::fs::write(path, b"hello").unwrap();
        let expected = "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824";
        assert!(check(path, expected).is_ok());
    }

    #[test]
    fn rejects_wrong_hash() {
        let path = "/tmp/validator_test_hash_bad.txt";
        std::fs::write(path, b"world").unwrap();
        let expected = "0000000000000000000000000000000000000000000000000000000000000000";
        assert!(check(path, expected).is_err());
    }
}
