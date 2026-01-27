describe("Instructions / settings_migrate_signers", () => {
  const skip = { it: it.skip };

  // Golden Path Tests
  skip.it("should_migrate_signers_from_v1_to_v2_v1");
  skip.it("should_realloc_settings_for_v2_signers_v1");
  skip.it("should_emit_signers_migrated_event_v1");

  // Invariant & Safety Tests
  skip.it("should_fail_on_unauthorized_settings_authority_v1");
  skip.it("should_fail_when_already_migrated_v1");
  skip.it("should_fail_when_missing_payer_v1");
  skip.it("should_fail_when_missing_system_program_v1");

  // Edge Case Tests
  skip.it("should_migrate_with_min_signer_set_v1");
  skip.it("should_migrate_with_max_signer_set_v1");
  // -------------------------------------------------------------------------------------
  // Settings V2 Tests
  // -------------------------------------------------------------------------------------

  skip.it("should_run_with_native_settings_v2");
  skip.it("should_run_with_p256_webauthn_settings_v2");
  skip.it("should_run_with_secp256k1_settings_v2");
  skip.it("should_run_with_ed25519_external_settings_v2");
});
