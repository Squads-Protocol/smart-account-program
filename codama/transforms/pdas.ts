import {
  constantPdaSeedNodeFromString,
  numberTypeNode,
  pdaNode,
  publicKeyTypeNode,
  variablePdaSeedNode,
  type PdaNode,
} from "codama";

/**
 * PDAs ported from `sdk/smart-account/src/pda.ts` (Anchor pre-0.30 IDLs do
 * not carry seed metadata, so codama cannot infer these from the JSON).
 * Names are kept camelCase to match codama's renderer conventions.
 */
export const smartAccountPdas = (): PdaNode[] => [
  pdaNode({
    name: "programConfig",
    seeds: [
      constantPdaSeedNodeFromString("utf8", "smart_account"),
      constantPdaSeedNodeFromString("utf8", "program_config"),
    ],
  }),
  pdaNode({
    name: "settings",
    seeds: [
      constantPdaSeedNodeFromString("utf8", "smart_account"),
      constantPdaSeedNodeFromString("utf8", "settings"),
      variablePdaSeedNode("accountIndex", numberTypeNode("u128")),
    ],
  }),
  pdaNode({
    name: "smartAccount",
    seeds: [
      constantPdaSeedNodeFromString("utf8", "smart_account"),
      variablePdaSeedNode("settings", publicKeyTypeNode()),
      constantPdaSeedNodeFromString("utf8", "smart_account"),
      variablePdaSeedNode("accountIndex", numberTypeNode("u8")),
    ],
  }),
  pdaNode({
    name: "ephemeralSigner",
    seeds: [
      constantPdaSeedNodeFromString("utf8", "smart_account"),
      variablePdaSeedNode("transaction", publicKeyTypeNode()),
      constantPdaSeedNodeFromString("utf8", "ephemeral_signer"),
      variablePdaSeedNode("ephemeralSignerIndex", numberTypeNode("u8")),
    ],
  }),
  pdaNode({
    name: "transaction",
    seeds: [
      constantPdaSeedNodeFromString("utf8", "smart_account"),
      variablePdaSeedNode("settings", publicKeyTypeNode()),
      constantPdaSeedNodeFromString("utf8", "transaction"),
      variablePdaSeedNode("transactionIndex", numberTypeNode("u64")),
    ],
  }),
  pdaNode({
    name: "proposal",
    seeds: [
      constantPdaSeedNodeFromString("utf8", "smart_account"),
      variablePdaSeedNode("settings", publicKeyTypeNode()),
      constantPdaSeedNodeFromString("utf8", "transaction"),
      variablePdaSeedNode("transactionIndex", numberTypeNode("u64")),
      constantPdaSeedNodeFromString("utf8", "proposal"),
    ],
  }),
  pdaNode({
    name: "batchTransaction",
    seeds: [
      constantPdaSeedNodeFromString("utf8", "smart_account"),
      variablePdaSeedNode("settings", publicKeyTypeNode()),
      constantPdaSeedNodeFromString("utf8", "transaction"),
      variablePdaSeedNode("batchIndex", numberTypeNode("u64")),
      constantPdaSeedNodeFromString("utf8", "batch_transaction"),
      variablePdaSeedNode("transactionIndex", numberTypeNode("u32")),
    ],
  }),
  pdaNode({
    name: "spendingLimit",
    seeds: [
      constantPdaSeedNodeFromString("utf8", "smart_account"),
      variablePdaSeedNode("settings", publicKeyTypeNode()),
      constantPdaSeedNodeFromString("utf8", "spending_limit"),
      variablePdaSeedNode("seed", publicKeyTypeNode()),
    ],
  }),
  pdaNode({
    name: "policy",
    seeds: [
      constantPdaSeedNodeFromString("utf8", "smart_account"),
      constantPdaSeedNodeFromString("utf8", "policy"),
      variablePdaSeedNode("settings", publicKeyTypeNode()),
      variablePdaSeedNode("policySeed", numberTypeNode("u64")),
    ],
  }),
];
