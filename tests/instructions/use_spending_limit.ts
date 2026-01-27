describe("Instructions / use_spending_limit", () => {
  const skip = { it: it.skip };

  // Golden Path Tests
  skip.it("should_use_spending_limit_for_sol_transfer_v1");
  skip.it("should_use_spending_limit_for_spl_transfer_v1");
  skip.it("should_emit_use_spending_limit_event_v1");
  skip.it("should_decrement_remaining_amount_v1");
  skip.it("should_reset_remaining_amount_after_period_v1");

  // Invariant & Safety Tests
  skip.it("should_fail_when_signer_not_authorized_v1");
  skip.it("should_fail_on_invalid_mint_for_sol_spending_limit_v1");
  skip.it("should_fail_on_invalid_mint_for_spl_spending_limit_v1");
  skip.it("should_fail_on_invalid_destination_not_in_allowlist_v1");
  skip.it("should_fail_on_spending_limit_expired_v1");
  skip.it("should_fail_on_spending_limit_exceeded_v1");
  skip.it("should_fail_on_decimals_mismatch_for_sol_v1");
  skip.it("should_fail_when_missing_required_token_accounts_v1");
  skip.it("should_fail_when_missing_token_program_v1");
  skip.it("should_fail_when_missing_system_program_for_sol_v1");

  // Edge Case Tests
  skip.it("should_use_spending_limit_with_exact_remaining_amount_v1");
  skip.it("should_use_spending_limit_with_zero_amount_v1");
  skip.it("should_use_spending_limit_with_one_time_period_v1");
  skip.it("should_use_spending_limit_with_non_expiring_v1");
  skip.it("should_reset_multiple_periods_elapsed_v1");
  // -------------------------------------------------------------------------------------
  // Settings V2 Tests
  // -------------------------------------------------------------------------------------

  skip.it("should_run_with_native_settings_v2");
  skip.it("should_run_with_p256_webauthn_settings_v2");
  skip.it("should_run_with_secp256k1_settings_v2");
  skip.it("should_run_with_ed25519_external_settings_v2");
});
