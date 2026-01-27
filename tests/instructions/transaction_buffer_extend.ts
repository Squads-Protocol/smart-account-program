describe("Instructions / transaction_buffer_extend", () => {
  const skip = { it: it.skip };

  // Golden Path Tests (V1 + V2 parity)
  skip.it("should_extend_transaction_buffer_v1");
  skip.it("should_extend_transaction_buffer_v2");
  skip.it("should_append_buffer_data_v1");
  skip.it("should_append_buffer_data_v2");

  // Invariant & Safety Tests (V1 + V2 parity)
  skip.it("should_fail_on_invalid_consensus_account_derivation_v1");
  skip.it("should_fail_on_invalid_consensus_account_derivation_v2");
  skip.it("should_fail_on_creator_not_matching_buffer_creator_v1");
  skip.it("should_fail_on_creator_not_matching_buffer_creator_v2");
  skip.it("should_fail_on_not_a_signer_v1");
  skip.it("should_fail_on_not_a_signer_v2");
  skip.it("should_fail_on_missing_initiate_permission_v1");
  skip.it("should_fail_on_missing_initiate_permission_v2");
  skip.it("should_fail_on_final_buffer_size_exceeded_v1");
  skip.it("should_fail_on_final_buffer_size_exceeded_v2");
  skip.it("should_fail_on_invalid_v2_context_signature_v2");
  skip.it("should_fail_on_invalid_webauthn_client_data_v2");

  // Edge Case Tests (V1 + V2 parity)
  skip.it("should_extend_with_empty_buffer_v1");
  skip.it("should_extend_with_empty_buffer_v2");
  skip.it("should_extend_to_final_size_v1");
  skip.it("should_extend_to_final_size_v2");
  skip.it("should_fail_when_extension_exceeds_remaining_space_v1");
  skip.it("should_fail_when_extension_exceeds_remaining_space_v2");
});
