/**
 * This code was GENERATED using the solita package.
 * Please DO NOT EDIT THIS FILE, instead rerun solita to update it or write a wrapper to add functionality.
 *
 * See: https://github.com/metaplex-foundation/solita
 */

import * as beet from '@metaplex-foundation/beet'
import * as web3 from '@solana/web3.js'
import {
  SetRentCollectorArgs,
  setRentCollectorArgsBeet,
} from '../types/SetRentCollectorArgs'

/**
 * @category Instructions
 * @category SetRentCollectorAsAuthority
 * @category generated
 */
export type SetRentCollectorAsAuthorityInstructionArgs = {
  args: SetRentCollectorArgs
}
/**
 * @category Instructions
 * @category SetRentCollectorAsAuthority
 * @category generated
 */
export const setRentCollectorAsAuthorityStruct = new beet.FixableBeetArgsStruct<
  SetRentCollectorAsAuthorityInstructionArgs & {
    instructionDiscriminator: number[] /* size: 8 */
  }
>(
  [
    ['instructionDiscriminator', beet.uniformFixedSizeArray(beet.u8, 8)],
    ['args', setRentCollectorArgsBeet],
  ],
  'SetRentCollectorAsAuthorityInstructionArgs'
)
/**
 * Accounts required by the _setRentCollectorAsAuthority_ instruction
 *
 * @property [_writable_] settings
 * @property [**signer**] settingsAuthority
 * @property [_writable_, **signer**] rentPayer (optional)
 * @category Instructions
 * @category SetRentCollectorAsAuthority
 * @category generated
 */
export type SetRentCollectorAsAuthorityInstructionAccounts = {
  settings: web3.PublicKey
  settingsAuthority: web3.PublicKey
  rentPayer?: web3.PublicKey
  systemProgram?: web3.PublicKey
  anchorRemainingAccounts?: web3.AccountMeta[]
}

export const setRentCollectorAsAuthorityInstructionDiscriminator = [
  58, 37, 73, 151, 249, 52, 252, 128,
]

/**
 * Creates a _SetRentCollectorAsAuthority_ instruction.
 *
 * Optional accounts that are not provided default to the program ID since
 * this was indicated in the IDL from which this instruction was generated.
 *
 * @param accounts that will be accessed while the instruction is processed
 * @param args to provide as instruction data to the program
 *
 * @category Instructions
 * @category SetRentCollectorAsAuthority
 * @category generated
 */
export function createSetRentCollectorAsAuthorityInstruction(
  accounts: SetRentCollectorAsAuthorityInstructionAccounts,
  args: SetRentCollectorAsAuthorityInstructionArgs,
  programId = new web3.PublicKey('SMRTe6bnZAgJmXt9aJin7XgAzDn1XMHGNy95QATyzpk')
) {
  const [data] = setRentCollectorAsAuthorityStruct.serialize({
    instructionDiscriminator:
      setRentCollectorAsAuthorityInstructionDiscriminator,
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
