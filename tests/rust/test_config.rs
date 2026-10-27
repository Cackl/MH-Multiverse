use super::*;

// These hit the real OS keychain (keyring crate, service "mh-multiverse") to
// get or create the encryption key — the same entry the running app uses.
// Not mockable without a bigger abstraction than 3 tests justify; run locally,
// not in a keychain-less CI environment.

#[test]
fn encrypt_decrypt_round_trip() {
    let (enc, nonce) = encrypt_password("s3cret-password").unwrap();
    assert!(!enc.is_empty());
    assert!(!nonce.is_empty());
    let plain = decrypt_password(&enc, &nonce).unwrap();
    assert_eq!(plain, "s3cret-password");
}

#[test]
fn encrypt_empty_returns_empty() {
    let (enc, nonce) = encrypt_password("").unwrap();
    assert_eq!(enc, "");
    assert_eq!(nonce, "");
}

#[test]
fn decrypt_empty_returns_empty() {
    assert_eq!(decrypt_password("", "").unwrap(), "");
}
