describe("Instructions / authority_spending_limit_remove", () => {
  const skip = { it: it.skip };

  // Golden Path Tests
  skip.it("should_remove_spending_limit_v1");
  skip.it("should_close_spending_limit_account_v1");
  skip.it("should_emit_authority_settings_event_v1");

  // Invariant & Safety Tests
  skip.it("should_fail_on_unauthorized_settings_authority_v1");
  skip.it("should_fail_on_spending_limit_for_other_settings_v1");
  skip.it("should_fail_on_invalid_spending_limit_account_v1");

  // Edge Case Tests
  skip.it("should_remove_spending_limit_with_rent_collector_different_from_authority_v1");
  // -------------------------------------------------------------------------------------
  // Settings V2 Tests
  // -------------------------------------------------------------------------------------

  skip.it("should_run_with_native_settings_v2");
  skip.it("should_run_with_p256_webauthn_settings_v2");
  skip.it("should_run_with_secp256k1_settings_v2");
  skip.it("should_run_with_ed25519_external_settings_v2");
});
