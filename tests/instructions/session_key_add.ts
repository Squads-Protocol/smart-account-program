describe("Instructions / session_key_add", () => {
  const skip = { it: it.skip };

  // Golden Path Tests
  skip.it("should_add_session_key_for_external_signer_v1");
  skip.it("should_realloc_settings_if_needed_v1");
  skip.it("should_emit_authority_settings_event_v1");

  // Invariant & Safety Tests
  skip.it("should_fail_when_signers_not_v2_v1");
  skip.it("should_fail_when_parent_signer_not_found_v1");
  skip.it("should_fail_when_parent_signer_is_native_v1");
  skip.it("should_fail_on_invalid_v2_context_signature_v1");
  skip.it("should_fail_on_invalid_webauthn_client_data_v1");
  skip.it("should_fail_on_invalid_session_key_pubkey_v1");
  skip.it("should_fail_on_expired_session_key_v1");
  skip.it("should_fail_on_session_key_expiration_too_far_v1");
  skip.it("should_fail_when_missing_rent_payer_for_realloc_v1");
  skip.it("should_fail_when_missing_system_program_for_realloc_v1");

  // Edge Case Tests
  skip.it("should_add_session_key_with_min_expiration_v1");
  skip.it("should_add_session_key_with_max_allowed_expiration_v1");
});
