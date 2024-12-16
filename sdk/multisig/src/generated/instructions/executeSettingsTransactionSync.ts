/**
 * This code was GENERATED using the solita package.
 * Please DO NOT EDIT THIS FILE, instead rerun solita to update it or write a wrapper to add functionality.
 *
 * See: https://github.com/metaplex-foundation/solita
 */

import * as beet from '@metaplex-foundation/beet'
import * as web3 from '@solana/web3.js'
import {
  SyncSettingsTransactionArgs,
  syncSettingsTransactionArgsBeet,
} from '../types/SyncSettingsTransactionArgs'

/**
 * @category Instructions
 * @category ExecuteSettingsTransactionSync
 * @category generated
 */
export type ExecuteSettingsTransactionSyncInstructionArgs = {
  args: SyncSettingsTransactionArgs
}
/**
 * @category Instructions
 * @category ExecuteSettingsTransactionSync
 * @category generated
 */
export const executeSettingsTransactionSyncStruct =
  new beet.FixableBeetArgsStruct<
    ExecuteSettingsTransactionSyncInstructionArgs & {
      instructionDiscriminator: number[] /* size: 8 */
    }
  >(
    [
      ['instructionDiscriminator', beet.uniformFixedSizeArray(beet.u8, 8)],
      ['args', syncSettingsTransactionArgsBeet],
    ],
    'ExecuteSettingsTransactionSyncInstructionArgs'
  )
/**
 * Accounts required by the _executeSettingsTransactionSync_ instruction
 *
 * @property [_writable_] settings
 * @property [_writable_, **signer**] rentPayer (optional)
 * @category Instructions
 * @category ExecuteSettingsTransactionSync
 * @category generated
 */
export type ExecuteSettingsTransactionSyncInstructionAccounts = {
  settings: web3.PublicKey
  rentPayer?: web3.PublicKey
  systemProgram?: web3.PublicKey
  anchorRemainingAccounts?: web3.AccountMeta[]
}

export const executeSettingsTransactionSyncInstructionDiscriminator = [
  138, 209, 64, 163, 79, 67, 233, 76,
]

/**
 * Creates a _ExecuteSettingsTransactionSync_ instruction.
 *
 * Optional accounts that are not provided default to the program ID since
 * this was indicated in the IDL from which this instruction was generated.
 *
 * @param accounts that will be accessed while the instruction is processed
 * @param args to provide as instruction data to the program
 *
 * @category Instructions
 * @category ExecuteSettingsTransactionSync
 * @category generated
 */
export function createExecuteSettingsTransactionSyncInstruction(
  accounts: ExecuteSettingsTransactionSyncInstructionAccounts,
  args: ExecuteSettingsTransactionSyncInstructionArgs,
  programId = new web3.PublicKey('SMRTe6bnZAgJmXt9aJin7XgAzDn1XMHGNy95QATyzpk')
) {
  const [data] = executeSettingsTransactionSyncStruct.serialize({
    instructionDiscriminator:
      executeSettingsTransactionSyncInstructionDiscriminator,
    ...args,
  })
  const keys: web3.AccountMeta[] = [
    {
      pubkey: accounts.settings,
      isWritable: true,
      isSigner: false,
    },
    {
      pubkey: accounts.rentPayer ?? programId,
      isWritable: accounts.rentPayer != null,
      isSigner: accounts.rentPayer != null,
    },
    {
      pubkey: accounts.systemProgram ?? programId,
      isWritable: false,
      isSigner: false,
    },
  ]

  if (accounts.anchorRemainingAccounts != null) {
    for (const acc of accounts.anchorRemainingAccounts) {
      keys.push(acc)
    }
  }

  const ix = new web3.TransactionInstruction({
    programId,
    keys,
    data,
  })
  return ix
}
