describe("Instructions / smart_account_create", () => {
  const skip = { it: it.skip };

  // Golden Path Tests (V1 + V2 parity)
  skip.it("should_create_smart_account_v1");
  skip.it("should_create_smart_account_v2");
  skip.it("should_emit_create_smart_account_event_v1");
  skip.it("should_emit_create_smart_account_event_v2");
  skip.it("should_increment_program_config_index_v1");
  skip.it("should_increment_program_config_index_v2");
  skip.it("should_transfer_creation_fee_when_configured_v1");
  skip.it("should_transfer_creation_fee_when_configured_v2");
  skip.it("should_sort_signers_by_pubkey_v1");
  skip.it("should_sort_signers_by_pubkey_v2");
  skip.it("should_set_initial_settings_state_v1");
  skip.it("should_set_initial_settings_state_v2");

  // Invariant & Safety Tests (V1 + V2 parity)
  skip.it("should_fail_on_invalid_treasury_account_v1");
  skip.it("should_fail_on_invalid_treasury_account_v2");
  skip.it("should_fail_on_invalid_threshold_v1");
  skip.it("should_fail_on_invalid_threshold_v2");
  skip.it("should_fail_on_duplicate_signers_v1");
  skip.it("should_fail_on_duplicate_signers_v2");
  skip.it("should_fail_on_empty_signers_v1");
  skip.it("should_fail_on_empty_signers_v2");
  skip.it("should_fail_on_invalid_signer_permissions_v1");
  skip.it("should_fail_on_invalid_signer_permissions_v2");
  skip.it("should_fail_on_insufficient_creator_balance_for_fee_v1");
  skip.it("should_fail_on_insufficient_creator_balance_for_fee_v2");

  // Edge Case Tests (V1 + V2 parity)
  skip.it("should_create_autonomous_account_with_no_settings_authority_v1");
  skip.it("should_create_autonomous_account_with_no_settings_authority_v2");
  skip.it("should_create_controlled_account_with_settings_authority_v1");
  skip.it("should_create_controlled_account_with_settings_authority_v2");
  skip.it("should_create_with_min_threshold_v1");
  skip.it("should_create_with_min_threshold_v2");
  skip.it("should_create_with_max_threshold_v1");
  skip.it("should_create_with_max_threshold_v2");
  skip.it("should_create_with_zero_timelock_v1");
  skip.it("should_create_with_zero_timelock_v2");
  skip.it("should_create_with_max_timelock_v1");
  skip.it("should_create_with_max_timelock_v2");
  skip.it("should_create_with_rent_collector_none_v1");
  skip.it("should_create_with_rent_collector_none_v2");
  // -------------------------------------------------------------------------------------
  // Settings V2 Tests
  // -------------------------------------------------------------------------------------

  skip.it("should_run_with_native_settings_v2");
  skip.it("should_run_with_p256_webauthn_settings_v2");
  skip.it("should_run_with_secp256k1_settings_v2");
  skip.it("should_run_with_ed25519_external_settings_v2");
});
