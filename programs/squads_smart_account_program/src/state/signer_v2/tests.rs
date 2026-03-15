use anchor_lang::prelude::*;
use crate::borsh::{BorshDeserialize, BorshSerialize};
use super::*;

#[test]
fn test_v1_serialization_roundtrip() {
    let signers = vec![
        LegacySmartAccountSigner {
            key: Pubkey::new_unique(),
            permissions: Permissions::all(),
        },
        LegacySmartAccountSigner {
            key: Pubkey::new_unique(),
            permissions: Permissions { mask: 0b011 },
        },
    ];

    let wrapper = SmartAccountSignerWrapper::V1(signers.clone());
    let mut buf = Vec::new();
    wrapper.serialize(&mut buf).unwrap();

    // Verify version byte is 0x00
    assert_eq!(buf[3], SIGNERS_VERSION_V1);

    // Deserialize and verify
    let deserialized = SmartAccountSignerWrapper::deserialize(&mut buf.as_slice()).unwrap();
    assert_eq!(wrapper, deserialized);
}

#[test]
fn test_v2_serialization_roundtrip() {
    let signers = vec![
        SmartAccountSigner::Native {
            key: Pubkey::new_unique(),
            permissions: Permissions::all(),
        },
        SmartAccountSigner::P256Webauthn {
            permissions: Permissions { mask: 0b111 },
            data: P256WebauthnData {
                compressed_pubkey: [0x02; 33],
                rp_id_len: 8,
                rp_id: {
                    let mut arr = [0u8; 32];
                    arr[..8].copy_from_slice(b"test.com");
                    arr
                },
                rp_id_hash: [0xAB; 32],
                counter: 42,
                session_key_data: SessionKeyData::default(),
            },
            nonce: 0,
        },
    ];

    let wrapper = SmartAccountSignerWrapper::V2(signers);
    let mut buf = Vec::new();
    wrapper.serialize(&mut buf).unwrap();

    // Verify version byte is 0x01
    assert_eq!(buf[3], SIGNERS_VERSION_V2);

    // Deserialize and verify
    let deserialized = SmartAccountSignerWrapper::deserialize(&mut buf.as_slice()).unwrap();
    assert_eq!(wrapper, deserialized);
}

#[test]
fn test_derive_key_id() {
    let pubkey_bytes = [0x02; 33];
    let data = P256WebauthnData::new([0x02; 33], b"example.com", [0xAB; 32], 0);
    let signer = SmartAccountSigner::P256Webauthn {
        permissions: Permissions::all(),
        data,
        nonce: 0,
    };

    // Truncated key should match the first 32 bytes of the pubkey
    let expected_key = Pubkey::new_from_array(pubkey_bytes[..32].try_into().unwrap());
    assert_eq!(signer.key(), expected_key);
}

#[test]
fn test_wrapper_force_v2() {
    let signers = vec![LegacySmartAccountSigner {
        key: Pubkey::new_unique(),
        permissions: Permissions::all(),
    }];

    let mut wrapper = SmartAccountSignerWrapper::V1(signers);
    assert_eq!(wrapper.version(), SIGNERS_VERSION_V1);

    wrapper.force_v2();
    assert_eq!(wrapper.version(), SIGNERS_VERSION_V2);
}

#[test]
fn test_push_external_converts_to_v2() {
    let mut wrapper = SmartAccountSignerWrapper::V1(vec![LegacySmartAccountSigner {
        key: Pubkey::new_unique(),
        permissions: Permissions::all(),
    }]);

    assert_eq!(wrapper.version(), SIGNERS_VERSION_V1);

    // Migrate to V2 before adding external signer
    wrapper.force_v2();
    assert_eq!(wrapper.version(), SIGNERS_VERSION_V2);

    // Now add an external signer
    wrapper.add_signer(SmartAccountSigner::P256Webauthn {
        permissions: Permissions::all(),
        data: P256WebauthnData {
            compressed_pubkey: [0x02; 33],
            rp_id_len: 8,
            rp_id: {
                let mut arr = [0u8; 32];
                arr[..8].copy_from_slice(b"test.com");
                arr
            },
            rp_id_hash: [0xAB; 32],
            counter: 0,
            session_key_data: SessionKeyData::default(),
        },
        nonce: 0,
    }).unwrap();

    assert_eq!(wrapper.version(), SIGNERS_VERSION_V2);
    assert_eq!(wrapper.len(), 2);
}

// ========================================================================
// Priority 1: V2 Signer Type Validation Tests
// ========================================================================

#[test]
fn test_ed25519_external_data_size() {
    assert_eq!(Ed25519ExternalData::SIZE, 32 + 40); // pubkey + session key data
    assert_eq!(Ed25519ExternalData::PACKED_PAYLOAD_LEN, 1 + Ed25519ExternalData::SIZE + 8);
}

#[test]
fn test_ed25519_external_serialization_roundtrip() {
    let data = Ed25519ExternalData {
        external_pubkey: [0xAB; 32],
        session_key_data: SessionKeyData {
            key: Pubkey::new_unique(),
            expiration: 1000000,
        },
    };

    let mut buf = Vec::new();
    data.serialize(&mut buf).unwrap();
    let deserialized = Ed25519ExternalData::deserialize(&mut buf.as_slice()).unwrap();

    assert_eq!(data, deserialized);
}

#[test]
fn test_secp256k1_data_size_validation() {
    assert_eq!(Secp256k1Data::SIZE, 64 + 20 + 1 + 40); // pubkey + eth_addr + has_eth + session
    assert_eq!(Secp256k1Data::PACKED_PAYLOAD_LEN, 1 + Secp256k1Data::SIZE + 8);
}

#[test]
fn test_secp256k1_serialization_roundtrip() {
    let data = Secp256k1Data {
        uncompressed_pubkey: [0xCD; 64],
        eth_address: [0xEF; 20],
        has_eth_address: true,
        session_key_data: SessionKeyData::default(),
    };

    let mut buf = Vec::new();
    data.serialize(&mut buf).unwrap();
    let deserialized = Secp256k1Data::deserialize(&mut buf.as_slice()).unwrap();

    assert_eq!(data, deserialized);
}

#[test]
fn test_p256_webauthn_data_size_validation() {
    assert_eq!(P256WebauthnData::SIZE, 33 + 1 + 32 + 32 + 8 + 40); // 146 bytes
    assert_eq!(P256WebauthnData::PACKED_PAYLOAD_LEN, 1 + P256WebauthnData::SIZE + 8);
}

#[test]
fn test_p256_rp_id_length_validation() {
    let data = P256WebauthnData::new(
        [0x02; 33],
        b"example.com", // 11 bytes
        [0xAB; 32],
        0,
    );

    assert_eq!(data.rp_id_len, 11);
    assert_eq!(data.get_rp_id(), b"example.com");
}

#[test]
fn test_p256_rp_id_padding() {
    let long_rp_id = b"this-is-a-very-long-domain-name-that-exceeds-32-bytes-limit";
    let data = P256WebauthnData::new(
        [0x02; 33],
        long_rp_id,
        [0xAB; 32],
        0,
    );

    // Should be capped at 32 bytes
    assert_eq!(data.rp_id_len, 32);
    assert_eq!(data.get_rp_id().len(), 32);
}

#[test]
fn test_p256_counter_increment() {
    let mut data = P256WebauthnData::default();
    assert_eq!(data.counter, 0);

    data.counter = 42;
    assert_eq!(data.counter, 42);

    data.counter = u64::MAX;
    assert_eq!(data.counter, u64::MAX);
}

#[test]
fn test_session_key_expiration_validation() {
    let mut session_data = SessionKeyData::default();
    let current_time = 1000000;
    let key = Pubkey::new_unique();

    // Valid expiration (future)
    let future_expiration = current_time + 3600;
    assert!(session_data.set(key, future_expiration, current_time).is_ok());

    // Invalid: expiration equals current time
    let equal_expiration = current_time;
    assert!(session_data.set(key, equal_expiration, current_time).is_err());

    // Invalid: expiration in the past
    let past_expiration = current_time - 1;
    assert!(session_data.set(key, past_expiration, current_time).is_err());
}

#[test]
fn test_session_key_expiration_limit() {
    let mut session_data = SessionKeyData::default();
    let current_time = 1000000;
    let key = Pubkey::new_unique();

    // Valid: exactly at the limit
    let max_expiration = current_time + SESSION_KEY_EXPIRATION_LIMIT;
    assert!(session_data.set(key, max_expiration, current_time).is_ok());

    // Invalid: exceeds the limit
    let over_limit = current_time + SESSION_KEY_EXPIRATION_LIMIT + 1;
    assert!(session_data.set(key, over_limit, current_time).is_err());
}

#[test]
fn test_session_key_default_pubkey_rejection() {
    let mut session_data = SessionKeyData::default();
    let current_time = 1000000;
    let future_expiration = current_time + 3600;

    // Should reject default pubkey
    let result = session_data.set(Pubkey::default(), future_expiration, current_time);
    assert!(result.is_err());
}

#[test]
fn test_session_key_is_active_boundary_cases() {
    let key = Pubkey::new_unique();
    let current_time = 1000000;

    // Active: not expired
    let active_session = SessionKeyData {
        key,
        expiration: current_time + 1,
    };
    assert!(active_session.is_active(current_time));

    // Inactive: exactly expired
    let expired_session = SessionKeyData {
        key,
        expiration: current_time,
    };
    assert!(!expired_session.is_active(current_time));

    // Inactive: default pubkey
    let default_session = SessionKeyData {
        key: Pubkey::default(),
        expiration: current_time + 1000,
    };
    assert!(!default_session.is_active(current_time));
}

#[test]
fn test_signer_wrapper_v1_v2_compatibility() {
    let v1_signer = LegacySmartAccountSigner {
        key: Pubkey::new_unique(),
        permissions: Permissions::all(),
    };

    // Convert to V2
    let v2_signer = SmartAccountSigner::from_v1(&v1_signer);

    // Convert back to V1
    let back_to_v1 = v2_signer.to_v1().unwrap();

    assert_eq!(v1_signer.key, back_to_v1.key);
    assert_eq!(v1_signer.permissions, back_to_v1.permissions);
}

#[test]
fn test_signer_wrapper_max_signers_validation() {
    // Test that MAX_SIGNERS is within u16::MAX
    assert_eq!(MAX_SIGNERS, u16::MAX as usize);

    // Test deserialization fails with too many signers
    let mut buf = Vec::new();
    let count = (MAX_SIGNERS + 1) as u32;
    let len_bytes = [
        (count & 0xFF) as u8,
        ((count >> 8) & 0xFF) as u8,
        ((count >> 16) & 0xFF) as u8,
        SIGNERS_VERSION_V1,
    ];
    buf.extend_from_slice(&len_bytes);

    let result = SmartAccountSignerWrapper::deserialize(&mut buf.as_slice());
    assert!(result.is_err());
}

#[test]
fn test_signer_serialization_version_handling() {
    // V1 wrapper
    let v1_wrapper = SmartAccountSignerWrapper::V1(vec![
        LegacySmartAccountSigner {
            key: Pubkey::new_unique(),
            permissions: Permissions::all(),
        }
    ]);

    let mut v1_buf = Vec::new();
    v1_wrapper.serialize(&mut v1_buf).unwrap();
    assert_eq!(v1_buf[3], SIGNERS_VERSION_V1);

    // V2 wrapper
    let v2_wrapper = SmartAccountSignerWrapper::V2(vec![
        SmartAccountSigner::Native {
            key: Pubkey::new_unique(),
            permissions: Permissions::all(),
        }
    ]);

    let mut v2_buf = Vec::new();
    v2_wrapper.serialize(&mut v2_buf).unwrap();
    assert_eq!(v2_buf[3], SIGNERS_VERSION_V2);
}

#[test]
fn test_from_raw_data_ed25519_size_validation() {
    let key = Pubkey::new_unique();
    let permissions = Permissions::all();

    // Valid: 32 bytes
    let valid_data = [0xAB; 32];
    let result = SmartAccountSigner::from_raw_data(3, key, permissions, &valid_data);
    assert!(result.is_ok());

    // Invalid: wrong size
    let invalid_data = [0xAB; 31];
    let result = SmartAccountSigner::from_raw_data(3, key, permissions, &invalid_data);
    assert!(result.is_err());

    let invalid_data = [0xAB; 33];
    let result = SmartAccountSigner::from_raw_data(3, key, permissions, &invalid_data);
    assert!(result.is_err());
}

#[test]
fn test_from_raw_data_secp256k1_size_validation() {
    let key = Pubkey::new_unique();
    let permissions = Permissions::all();

    // Valid: 64 bytes (uncompressed pubkey only; eth_address derived on-chain)
    let valid_data = vec![0xAB; 64]; // uncompressed pubkey

    let result = SmartAccountSigner::from_raw_data(2, key, permissions, &valid_data);
    assert!(result.is_ok());

    // Verify eth_address was derived
    if let SmartAccountSigner::Secp256k1 { data, .. } = result.unwrap() {
        assert!(data.has_eth_address);
        // eth_address should be keccak256(pubkey)[12..32], not zero
        assert_ne!(data.eth_address, [0u8; 20]);
    } else {
        panic!("Expected Secp256k1 variant");
    }

    // Invalid: wrong size (old 85-byte format)
    let invalid_data = vec![0xAB; 85];
    let result = SmartAccountSigner::from_raw_data(2, key, permissions, &invalid_data);
    assert!(result.is_err());

    // Invalid: too short
    let invalid_data = vec![0xAB; 63];
    let result = SmartAccountSigner::from_raw_data(2, key, permissions, &invalid_data);
    assert!(result.is_err());
}

#[test]
fn test_from_raw_data_p256_size_validation() {
    let key = Pubkey::new_unique();
    let permissions = Permissions::all();

    // Valid: 74 bytes (33 + 1 + 32 + 8)
    let mut valid_data = vec![0x02; 33]; // compressed pubkey
    valid_data.push(11); // rp_id_len
    valid_data.extend_from_slice(b"example.com"); // rp_id (11 bytes)
    valid_data.extend_from_slice(&[0x00; 21]); // padding to 32 bytes
    valid_data.extend_from_slice(&42u64.to_le_bytes()); // counter

    let result = SmartAccountSigner::from_raw_data(1, key, permissions, &valid_data);
    assert!(result.is_ok());

    // Invalid: wrong size
    let invalid_data = vec![0x02; 73];
    let result = SmartAccountSigner::from_raw_data(1, key, permissions, &invalid_data);
    assert!(result.is_err());
}

#[test]
fn test_from_raw_data_p256_rp_id_len_bounds() {
    let key = Pubkey::new_unique();
    let permissions = Permissions::all();

    // Valid: rp_id_len <= 32
    let mut valid_data = vec![0x02; 33];
    valid_data.push(32); // rp_id_len at max
    valid_data.extend_from_slice(&[0x41; 32]); // rp_id
    valid_data.extend_from_slice(&0u64.to_le_bytes()); // counter

    let result = SmartAccountSigner::from_raw_data(1, key, permissions, &valid_data);
    assert!(result.is_ok());

    // Invalid: rp_id_len > 32
    let mut invalid_data = vec![0x02; 33];
    invalid_data.push(33); // rp_id_len exceeds max
    invalid_data.extend_from_slice(&[0x41; 32]);
    invalid_data.extend_from_slice(&0u64.to_le_bytes());

    let result = SmartAccountSigner::from_raw_data(1, key, permissions, &invalid_data);
    assert!(result.is_err());
}

#[test]
fn test_from_raw_data_native_empty_data() {
    let key = Pubkey::new_unique();
    let permissions = Permissions::all();

    // Valid: empty data for native
    let result = SmartAccountSigner::from_raw_data(0, key, permissions, &[]);
    assert!(result.is_ok());

    // Invalid: non-empty data for native
    let result = SmartAccountSigner::from_raw_data(0, key, permissions, &[0x00]);
    assert!(result.is_err());

    // Invalid: default pubkey for native
    let result = SmartAccountSigner::from_raw_data(0, Pubkey::default(), permissions, &[]);
    assert!(result.is_err());
}

#[test]
fn test_from_raw_data_invalid_permissions() {
    let key = Pubkey::new_unique();

    // Invalid permissions (mask >= 8)
    let invalid_permissions = Permissions { mask: 8 };
    let result = SmartAccountSigner::from_raw_data(0, key, invalid_permissions, &[]);
    assert!(result.is_err());

    let invalid_permissions = Permissions { mask: 255 };
    let result = SmartAccountSigner::from_raw_data(0, key, invalid_permissions, &[]);
    assert!(result.is_err());
}

#[test]
fn test_from_raw_data_invalid_signer_type() {
    let key = Pubkey::new_unique();
    let permissions = Permissions::all();

    // Invalid signer type (>= 4)
    let result = SmartAccountSigner::from_raw_data(4, key, permissions, &[]);
    assert!(result.is_err());

    let result = SmartAccountSigner::from_raw_data(255, key, permissions, &[]);
    assert!(result.is_err());
}

#[test]
fn test_has_same_public_key() {
    let pubkey1 = Pubkey::new_unique();
    let pubkey2 = Pubkey::new_unique();

    let native1 = SmartAccountSigner::Native {
        key: pubkey1,
        permissions: Permissions::all(),
    };

    let native2 = SmartAccountSigner::Native {
        key: pubkey1,
        permissions: Permissions { mask: 0b001 },
    };

    let native3 = SmartAccountSigner::Native {
        key: pubkey2,
        permissions: Permissions::all(),
    };

    // Same native key
    assert!(native1.has_same_public_key(&native2));

    // Different native keys
    assert!(!native1.has_same_public_key(&native3));

    // P256 signers with same compressed pubkey
    let p256_1 = SmartAccountSigner::P256Webauthn {
        permissions: Permissions::all(),
        data: P256WebauthnData {
            compressed_pubkey: [0x02; 33],
            rp_id_len: 8,
            rp_id: [0x00; 32],
            rp_id_hash: [0xAB; 32],
            counter: 0,
            session_key_data: SessionKeyData::default(),
        },
        nonce: 0,
    };

    let p256_2 = SmartAccountSigner::P256Webauthn {
        permissions: Permissions::all(),
        data: P256WebauthnData {
            compressed_pubkey: [0x02; 33], // Same pubkey
            rp_id_len: 8,
            rp_id: [0x00; 32],
            rp_id_hash: [0xAB; 32],
            counter: 0,
            session_key_data: SessionKeyData::default(),
        },
        nonce: 0,
    };

    // Same P256 pubkey
    assert!(p256_1.has_same_public_key(&p256_2));

    // Different signer types never match
    assert!(!native1.has_same_public_key(&p256_1));
}

#[test]
fn test_wrapper_serialized_size_calculation() {
    // V1 wrapper
    let v1_wrapper = SmartAccountSignerWrapper::V1(vec![
        LegacySmartAccountSigner {
            key: Pubkey::new_unique(),
            permissions: Permissions::all(),
        },
        LegacySmartAccountSigner {
            key: Pubkey::new_unique(),
            permissions: Permissions { mask: 0b011 },
        },
    ]);

    let calculated_size = v1_wrapper.serialized_size();
    let mut buf = Vec::new();
    v1_wrapper.serialize(&mut buf).unwrap();
    assert_eq!(calculated_size, buf.len());

    // V2 wrapper with mixed signers
    let v2_wrapper = SmartAccountSignerWrapper::V2(vec![
        SmartAccountSigner::Native {
            key: Pubkey::new_unique(),
            permissions: Permissions::all(),
        },
        SmartAccountSigner::Ed25519External {
            permissions: Permissions::all(),
            data: Ed25519ExternalData {
                external_pubkey: [0xAB; 32],
                session_key_data: SessionKeyData::default(),
            },
            nonce: 0,
        },
    ]);

    let calculated_size = v2_wrapper.serialized_size();
    let mut buf = Vec::new();
    v2_wrapper.serialize(&mut buf).unwrap();
    assert_eq!(calculated_size, buf.len());
}

#[test]
fn test_client_data_json_reconstruction_params() {
    use super::precompile::ClientDataJsonReconstructionParams;

    // Test TYPE_CREATE
    let create_params = ClientDataJsonReconstructionParams::new(
        true, false, false, false, None
    );
    assert!(create_params.is_create());
    assert!(!create_params.is_cross_origin());
    assert!(!create_params.is_http());
    assert!(!create_params.has_google_extra());
    assert_eq!(create_params.get_port(), None);

    // Test TYPE_GET with flags and port
    let get_params = ClientDataJsonReconstructionParams::new(
        false, true, true, true, Some(8080)
    );
    assert!(!get_params.is_create());
    assert!(get_params.is_cross_origin());
    assert!(get_params.is_http());
    assert!(get_params.has_google_extra());
    assert_eq!(get_params.get_port(), Some(8080));
}

#[test]
fn test_wrapper_all_permissions_valid() {
    // Valid V1 wrapper
    let valid_v1 = SmartAccountSignerWrapper::V1(vec![
        LegacySmartAccountSigner {
            key: Pubkey::new_unique(),
            permissions: Permissions { mask: 7 }, // 0b111 - valid
        },
    ]);
    assert!(valid_v1.all_permissions_valid());

    // Invalid V1 wrapper
    let invalid_v1 = SmartAccountSignerWrapper::V1(vec![
        LegacySmartAccountSigner {
            key: Pubkey::new_unique(),
            permissions: Permissions { mask: 8 }, // >= 8 - invalid
        },
    ]);
    assert!(!invalid_v1.all_permissions_valid());

    // Valid V2 wrapper
    let valid_v2 = SmartAccountSignerWrapper::V2(vec![
        SmartAccountSigner::Native {
            key: Pubkey::new_unique(),
            permissions: Permissions { mask: 0 }, // valid
        },
    ]);
    assert!(valid_v2.all_permissions_valid());

    // Invalid V2 wrapper
    let invalid_v2 = SmartAccountSignerWrapper::V2(vec![
        SmartAccountSigner::Native {
            key: Pubkey::new_unique(),
            permissions: Permissions { mask: 15 }, // >= 8 - invalid
        },
    ]);
    assert!(!invalid_v2.all_permissions_valid());
}

#[test]
fn test_wrapper_counter_updates() {
    let signer = SmartAccountSigner::P256Webauthn {
        permissions: Permissions::all(),
        data: P256WebauthnData {
            compressed_pubkey: [0x02; 33],
            rp_id_len: 8,
            rp_id: [0x00; 32],
            rp_id_hash: [0xAB; 32],
            counter: 100,
            session_key_data: SessionKeyData::default(),
        },
        nonce: 0,
    };
    let signer_key = signer.key();
    let mut wrapper = SmartAccountSignerWrapper::V2(vec![signer]);

    // Update counter
    let updates = vec![(signer_key, 101)];
    assert!(wrapper.apply_counter_updates(&updates).is_ok());

    // Verify counter was updated
    if let Some(SmartAccountSigner::P256Webauthn { data, .. }) = wrapper.find_mut(&signer_key) {
        assert_eq!(data.counter, 101);
    } else {
        panic!("Expected P256Webauthn signer");
    }

    // V1 wrapper should fail counter updates
    let mut v1_wrapper = SmartAccountSignerWrapper::V1(vec![
        LegacySmartAccountSigner {
            key: Pubkey::new_unique(),
            permissions: Permissions::all(),
        },
    ]);
    assert!(v1_wrapper.apply_counter_updates(&updates).is_err());
}
