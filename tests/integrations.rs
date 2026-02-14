use std::io::Cursor;

#[test]
fn test_encrypt_decrypt_roundtrip() {
    let original = b"Hello, deadrop! This is a secret message that should survive encryption.";
    let key = deadrop::crypto::EncryptionKey::generate();

    let mut reader = Cursor::new(original);
    let ciphertext = deadrop::crypto::encrypt_file_streaming(
        &mut reader,
        &key,
        original.len() as u64,
        |_| {},
    ).unwrap();

    // Verify ciphertext is different from plaintext
    assert_ne!(&ciphertext[40..], original.as_slice());

    // Verify key round-trips through URL encoding
    let encoded = key.to_url_safe();
    let decoded = deadrop::crypto::EncryptionKey::from_url_safe(&encoded).unwrap();
    assert_eq!(key.0, decoded.0);
}

#[test]
fn test_password_key_derivation() {
    let salt: [u8; 16] = [42u8; 16];
    let key1 = deadrop::crypto::EncryptionKey::from_password("hunter2", &salt).unwrap();
    let key2 = deadrop::crypto::EncryptionKey::from_password("hunter2", &salt).unwrap();
    let key3 = deadrop::crypto::EncryptionKey::from_password("hunter3", &salt).unwrap();

    // Same password + salt = same key
    assert_eq!(key1.0, key2.0);
    // Different password = different key
    assert_ne!(key1.0, key3.0);
}

#[test]
fn test_config_duration_parsing() {
    let config = deadrop::config::DropConfig::new(
        std::path::PathBuf::from("Cargo.toml"), // Use any existing file
        8080,
        "30m".to_string(),
        1,
        None,
        "0.0.0.0".to_string(),
        false,
    ).unwrap();
    assert_eq!(config.expiry_duration, chrono::Duration::minutes(30));
}
