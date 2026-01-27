describe("Instructions / authority_spending_limit_add", () => {
  const skip = { it: it.skip };

  // Golden Path Tests
  skip.it("should_add_spending_limit_v1");
  skip.it("should_set_spending_limit_fields_v1");
  skip.it("should_emit_authority_settings_event_v1");

  // Invariant & Safety Tests
  skip.it("should_fail_on_unauthorized_settings_authority_v1");
  skip.it("should_fail_on_expired_spending_limit_v1");
  skip.it("should_fail_on_invalid_spending_limit_account_seeds_v1");
  skip.it("should_fail_on_duplicate_signers_in_spending_limit_v1");
  skip.it("should_fail_with_insufficient_rent_payer_balance_v1");

  // Edge Case Tests
  skip.it("should_add_spending_limit_with_empty_signers_v1");
  skip.it("should_add_spending_limit_with_empty_destinations_v1");
  skip.it("should_add_spending_limit_with_max_signers_v1");
  skip.it("should_add_spending_limit_with_max_destinations_v1");
  skip.it("should_add_spending_limit_with_one_time_period_v1");
  skip.it("should_add_spending_limit_with_non_expiring_v1");
  // -------------------------------------------------------------------------------------
  // Settings V2 Tests
  // -------------------------------------------------------------------------------------

  skip.it("should_run_with_native_settings_v2");
  skip.it("should_run_with_p256_webauthn_settings_v2");
  skip.it("should_run_with_secp256k1_settings_v2");
  skip.it("should_run_with_ed25519_external_settings_v2");
});
