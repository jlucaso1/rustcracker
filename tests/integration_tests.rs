use rustcracker::*;

#[test]
fn test_hex_decode() {
    // Test known MD5 hash decoding
    let hash_str = "5f4dcc3b5aa765d61d8327deb882cf99";
    let decoded = hex::decode(hash_str).unwrap();
    assert_eq!(decoded.len(), 16);
}

#[test]
fn test_target_hash_conversion() {
    // Test converting hex hash to TargetHash structure
    let hash_bytes: [u8; 16] = [
        0x5f, 0x4d, 0xcc, 0x3b, 0x5a, 0xa7, 0x65, 0xd6, 0x1d, 0x83, 0x27, 0xde, 0xb8, 0x82, 0xcf,
        0x99,
    ];

    let target = TargetHash {
        data: [
            u32::from_le_bytes([hash_bytes[0], hash_bytes[1], hash_bytes[2], hash_bytes[3]]),
            u32::from_le_bytes([hash_bytes[4], hash_bytes[5], hash_bytes[6], hash_bytes[7]]),
            u32::from_le_bytes([hash_bytes[8], hash_bytes[9], hash_bytes[10], hash_bytes[11]]),
            u32::from_le_bytes([
                hash_bytes[12],
                hash_bytes[13],
                hash_bytes[14],
                hash_bytes[15],
            ]),
        ],
    };

    // Verify the conversion is correct
    assert_eq!(target.data[0], 0x3bcc4d5f);
    assert_eq!(target.data[1], 0xd665a75a);
    assert_eq!(target.data[2], 0xde27831d);
    assert_eq!(target.data[3], 0x99cf82b8);
}

#[test]
fn test_verify_known_hashes() {
    // Verify known password/hash pairs using CPU MD5
    let test_cases = vec![
        ("password", "5f4dcc3b5aa765d61d8327deb882cf99"),
        ("hello", "5d41402abc4b2a76b9719d911017c592"),
        ("123456", "e10adc3949ba59abbe56e057f20f883e"),
        ("test", "098f6bcd4621d373cade4e832627b4f6"),
        ("admin", "21232f297a57a5a743894a0e4a801fc3"),
    ];

    for (password, expected_hash) in test_cases {
        let computed = format!("{:x}", md5::compute(password.as_bytes()));
        assert_eq!(computed, expected_hash, "MD5 mismatch for '{password}'");
    }
}

#[tokio::test]
async fn test_gpu_cracker_init() {
    // Test that GPU cracker initializes without errors
    let result = GpuCracker::new().await;
    assert!(
        result.is_ok(),
        "Failed to initialize GPU cracker: {:?}",
        result.err()
    );
}

#[tokio::test]
async fn test_simple_crack() {
    // Test cracking a simple hash
    let mut cracker = GpuCracker::new().await.expect("Failed to initialize GPU");

    // md5("password") = 5f4dcc3b5aa765d61d8327deb882cf99
    let target_hash: [u8; 16] = [
        0x5f, 0x4d, 0xcc, 0x3b, 0x5a, 0xa7, 0x65, 0xd6, 0x1d, 0x83, 0x27, 0xde, 0xb8, 0x82, 0xcf,
        0x99,
    ];

    let wordlist = vec!["wrong1", "wrong2", "password", "wrong3"];
    let result = cracker.crack(&target_hash, &wordlist);

    assert_eq!(result, Some("password".to_string()));
}

#[tokio::test]
async fn test_crack_not_found() {
    // Test when password is not in wordlist
    let mut cracker = GpuCracker::new().await.expect("Failed to initialize GPU");

    // md5("password") = 5f4dcc3b5aa765d61d8327deb882cf99
    let target_hash: [u8; 16] = [
        0x5f, 0x4d, 0xcc, 0x3b, 0x5a, 0xa7, 0x65, 0xd6, 0x1d, 0x83, 0x27, 0xde, 0xb8, 0x82, 0xcf,
        0x99,
    ];

    let wordlist = vec!["wrong1", "wrong2", "wrong3", "wrong4"];
    let result = cracker.crack(&target_hash, &wordlist);

    assert_eq!(result, None);
}

#[tokio::test]
async fn test_multiple_known_hashes() {
    // Test cracking multiple known hashes
    let mut cracker = GpuCracker::new().await.expect("Failed to initialize GPU");

    let test_cases = vec![
        ("password", "5f4dcc3b5aa765d61d8327deb882cf99"),
        ("hello", "5d41402abc4b2a76b9719d911017c592"),
        ("123456", "e10adc3949ba59abbe56e057f20f883e"),
        ("test", "098f6bcd4621d373cade4e832627b4f6"),
    ];

    for (expected_password, hash_str) in test_cases {
        let hash_bytes = hex::decode(hash_str).unwrap();
        let mut target_hash = [0u8; 16];
        target_hash.copy_from_slice(&hash_bytes);

        let wordlist = vec!["wrong1", "wrong2", expected_password, "wrong3"];
        let result = cracker.crack(&target_hash, &wordlist);

        assert_eq!(
            result,
            Some(expected_password.to_string()),
            "Failed to crack hash for '{expected_password}'"
        );
    }
}

#[tokio::test]
async fn test_large_batch() {
    // Test with a batch larger than BATCH_SIZE
    let mut cracker = GpuCracker::new().await.expect("Failed to initialize GPU");

    // md5("target") = c90c4...  (we'll compute it)
    let target_password = "target";
    let target_hash_str = format!("{:x}", md5::compute(target_password.as_bytes()));
    let hash_bytes = hex::decode(&target_hash_str).unwrap();
    let mut target_hash = [0u8; 16];
    target_hash.copy_from_slice(&hash_bytes);

    // Create a large wordlist with the target at position 5000
    let mut wordlist: Vec<String> = (0..5000).map(|i| format!("wrong{i}")).collect();
    wordlist.push(target_password.to_string());
    let wordlist_refs: Vec<&str> = wordlist.iter().map(|s| s.as_str()).collect();

    let result = cracker.crack(&target_hash, &wordlist_refs);

    assert_eq!(result, Some(target_password.to_string()));
}

#[tokio::test]
async fn test_empty_password() {
    // Test cracking an empty password
    let mut cracker = GpuCracker::new().await.expect("Failed to initialize GPU");

    // md5("") = d41d8cd98f00b204e9800998ecf8427e
    let target_hash: [u8; 16] = hex::decode("d41d8cd98f00b204e9800998ecf8427e")
        .unwrap()
        .try_into()
        .unwrap();

    let wordlist = vec!["", "test", "password"];
    let result = cracker.crack(&target_hash, &wordlist);

    assert_eq!(result, Some("".to_string()));
}

#[tokio::test]
async fn test_long_password() {
    // Test with a longer password (but still under MAX_MSG_SIZE)
    let mut cracker = GpuCracker::new().await.expect("Failed to initialize GPU");

    let target_password = "this_is_a_much_longer_password_for_testing_purposes_12345";
    let target_hash_str = format!("{:x}", md5::compute(target_password.as_bytes()));
    let hash_bytes = hex::decode(&target_hash_str).unwrap();
    let mut target_hash = [0u8; 16];
    target_hash.copy_from_slice(&hash_bytes);

    let wordlist = vec!["short", "medium_length", target_password, "another"];
    let result = cracker.crack(&target_hash, &wordlist);

    assert_eq!(result, Some(target_password.to_string()));
}
