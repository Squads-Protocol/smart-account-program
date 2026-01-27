describe("Instructions / log_event", () => {
  const skip = { it: it.skip };

  // Golden Path Tests
  skip.it("should_log_event_with_valid_log_authority_v1");
  skip.it("should_accept_allowed_discriminator_settings_v1");
  skip.it("should_accept_allowed_discriminator_policy_v1");
  skip.it("should_accept_allowed_discriminator_proposal_v1");
  skip.it("should_accept_allowed_discriminator_transaction_v1");
  skip.it("should_accept_allowed_discriminator_settings_transaction_v1");
  skip.it("should_accept_allowed_discriminator_program_config_v1");

  // Invariant & Safety Tests
  skip.it("should_fail_on_invalid_log_authority_owner_v1");
  skip.it("should_fail_on_zero_initialized_log_authority_data_v1");
  skip.it("should_fail_on_unallowed_discriminator_v1");
  skip.it("should_fail_on_missing_discriminator_bytes_v1");
  skip.it("should_reject_log_event_instruction_from_program_execution_v1");

  // Edge Case Tests
  skip.it("should_allow_minimal_event_payload_v1");
  skip.it("should_allow_max_event_payload_v1");
});
