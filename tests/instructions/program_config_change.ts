describe("Instructions / program_config_change", () => {
  const skip = { it: it.skip };

  // Golden Path Tests
  skip.it("should_set_program_config_authority_v1");
  skip.it("should_set_smart_account_creation_fee_v1");
  skip.it("should_set_treasury_v1");

  // Invariant & Safety Tests
  skip.it("should_fail_on_unauthorized_authority_v1");
  skip.it("should_fail_on_invalid_program_config_account_v1");
  skip.it("should_fail_on_invalid_new_authority_v1");

  // Edge Case Tests
  skip.it("should_set_creation_fee_to_zero_v1");
  skip.it("should_set_creation_fee_to_max_u64_v1");
  skip.it("should_set_treasury_to_current_value_v1");
});
