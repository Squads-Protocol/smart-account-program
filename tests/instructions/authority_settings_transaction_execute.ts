describe("Instructions / authority_settings_transaction_execute", () => {
  const skip = { it: it.skip };

  // Golden Path Tests
  skip.it("should_add_signer_v1");
  skip.it("should_add_signer_v2");
  skip.it("should_remove_signer_v1");
  skip.it("should_remove_signer_v2");
  skip.it("should_change_threshold_v1");
  skip.it("should_set_time_lock_v1");
  skip.it("should_set_new_settings_authority_v1");
  skip.it("should_set_archival_authority_v1");
  skip.it("should_emit_authority_settings_event_v1");
  skip.it("should_emit_authority_settings_event_v2");

  // Invariant & Safety Tests
  skip.it("should_fail_on_unauthorized_settings_authority_v1");
  skip.it("should_fail_on_unauthorized_settings_authority_v2");
  skip.it("should_fail_add_signer_on_duplicate_signer_v1");
  skip.it("should_fail_add_signer_on_duplicate_signer_v2");
  skip.it("should_fail_remove_signer_on_last_signer_v1");
  skip.it("should_fail_remove_signer_on_last_signer_v2");
  skip.it("should_fail_remove_signer_on_nonexistent_signer_v1");
  skip.it("should_fail_remove_signer_on_nonexistent_signer_v2");
  skip.it("should_fail_change_threshold_invalid_value_v1");
  skip.it("should_fail_set_time_lock_invalid_value_v1");
  skip.it("should_fail_on_missing_rent_payer_when_realloc_needed_v1");
  skip.it("should_fail_on_missing_rent_payer_when_realloc_needed_v2");
  skip.it("should_fail_on_missing_system_program_when_realloc_needed_v1");
  skip.it("should_fail_on_missing_system_program_when_realloc_needed_v2");
  skip.it("should_fail_add_signer_v2_when_not_migrated_v2");
  skip.it("should_fail_remove_signer_v2_when_not_migrated_v2");
  skip.it("should_fail_set_archival_authority_not_implemented_v1");

  // Edge Case Tests
  skip.it("should_add_signer_with_min_permissions_v1");
  skip.it("should_add_signer_with_max_permissions_v1");
  skip.it("should_add_signer_with_external_signer_type_v2");
  skip.it("should_add_signer_with_invalid_signer_data_v2");
  skip.it("should_remove_signer_and_update_threshold_if_needed_v1");
  skip.it("should_change_threshold_to_min_v1");
  skip.it("should_change_threshold_to_max_v1");
  skip.it("should_set_time_lock_to_zero_v1");
  skip.it("should_set_time_lock_to_max_v1");
  skip.it("should_set_new_settings_authority_to_none_v1");
});
