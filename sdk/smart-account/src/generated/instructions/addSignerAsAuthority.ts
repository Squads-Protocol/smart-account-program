/**
 * This code was GENERATED using the solita package.
 * Please DO NOT EDIT THIS FILE, instead rerun solita to update it or write a wrapper to add functionality.
 *
 * See: https://github.com/metaplex-foundation/solita
 */

import * as beet from '@metaplex-foundation/beet'
import * as web3 from '@solana/web3.js'
import { AddSignerArgs, addSignerArgsBeet } from '../types/AddSignerArgs'

/**
 * @category Instructions
 * @category AddSignerAsAuthority
 * @category generated
 */
export type AddSignerAsAuthorityInstructionArgs = {
  args: AddSignerArgs
}
/**
 * @category Instructions
 * @category AddSignerAsAuthority
 * @category generated
 */
export const addSignerAsAuthorityStruct = new beet.FixableBeetArgsStruct<
  AddSignerAsAuthorityInstructionArgs & {
    instructionDiscriminator: number[] /* size: 8 */
  }
>(
  [
    ['instructionDiscriminator', beet.uniformFixedSizeArray(beet.u8, 8)],
    ['args', addSignerArgsBeet],
  ],
  'AddSignerAsAuthorityInstructionArgs'
)
/**
 * Accounts required by the _addSignerAsAuthority_ instruction
 *
 * @property [_writable_] settings
 * @property [**signer**] settingsAuthority
 * @property [_writable_, **signer**] rentPayer (optional)
 * @property [] program
 * @category Instructions
 * @category AddSignerAsAuthority
 * @category generated
 */
export type AddSignerAsAuthorityInstructionAccounts = {
  settings: web3.PublicKey
  settingsAuthority: web3.PublicKey
  rentPayer?: web3.PublicKey
  systemProgram?: web3.PublicKey
  program: web3.PublicKey
  anchorRemainingAccounts?: web3.AccountMeta[]
}

export const addSignerAsAuthorityInstructionDiscriminator = [
  80, 198, 228, 154, 7, 234, 99, 56,
]

/**
 * Creates a _AddSignerAsAuthority_ instruction.
 *
 * Optional accounts that are not provided default to the program ID since
 * this was indicated in the IDL from which this instruction was generated.
 *
 * @param accounts that will be accessed while the instruction is processed
 * @param args to provide as instruction data to the program
 *
 * @category Instructions
 * @category AddSignerAsAuthority
 * @category generated
 */
export function createAddSignerAsAuthorityInstruction(
  accounts: AddSignerAsAuthorityInstructionAccounts,
  args: AddSignerAsAuthorityInstructionArgs,
  programId = new web3.PublicKey('SMRTzfY6DfH5ik3TKiyLFfXexV8uSG3d2UksSCYdunG')
) {
  const [data] = addSignerAsAuthorityStruct.serialize({
    instructionDiscriminator: addSignerAsAuthorityInstructionDiscriminator,
    ...args,
  })
  const keys: web3.AccountMeta[] = [
    {
      pubkey: accounts.settings,
      isWritable: true,
      isSigner: false,
    },
    {
      pubkey: accounts.settingsAuthority,
      isWritable: false,
      isSigner: true,
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
    {
      pubkey: accounts.program,
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
