describe("Instructions / program_config_init", () => {
  const skip = { it: it.skip };

  // Golden Path Tests
  skip.it("should_initialize_program_config_v1");
  skip.it("should_set_authority_creation_fee_and_treasury_v1");
  skip.it("should_set_smart_account_index_to_zero_v1");

  // Invariant & Safety Tests
  skip.it("should_fail_on_unauthorized_initializer_v1");
  skip.it("should_fail_when_program_config_already_initialized_v1");
  skip.it("should_fail_on_invalid_treasury_pubkey_v1");

  // Edge Case Tests
  skip.it("should_initialize_with_zero_creation_fee_v1");
  skip.it("should_initialize_with_max_creation_fee_v1");
});
