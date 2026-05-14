/**
 * Defined-type names that must be dropped from the parsed IDL because they
 * either reference undefined sibling types (the `*Event` enum variants point
 * at concrete event payload types that the Anchor IDL never emits) or are
 * known to have IDL-representation issues that the program serializes
 * differently from what Anchor records.
 *
 * Inherited from `sdk/smart-account/.solitarc.js`; pruned to only include
 * names actually present in the current IDL (verified against
 * `idl/squads_smart_account_program.json`).  Names that the old config
 * blocked because of `SmallVec` issues (TransactionMessage, CompiledInstruction,
 * MessageAddressTableLookup) are **not** dropped here — codama now models
 * them correctly via the small-vec transform.
 */
export const ignoredDefinedTypes = new Set<string>([
  // Enum whose variants reference undefined payload types — keeping it would
  // produce dangling links in the rendered SDK.
  "SmartAccountEvent",
  // Permission flag set; the Anchor IDL emits it as a u8 alias which loses
  // the bitflag semantics.  Consumers should reach for the program enum or a
  // bespoke helper rather than the generated type.
  "Permission",
]);
