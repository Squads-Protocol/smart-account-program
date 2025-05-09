/**
 * This code was GENERATED using the solita package.
 * Please DO NOT EDIT THIS FILE, instead rerun solita to update it or write a wrapper to add functionality.
 *
 * See: https://github.com/metaplex-foundation/solita
 */

import * as beet from '@metaplex-foundation/beet'
import * as web3 from '@solana/web3.js'
import {
  ProgramConfigSetAuthorityArgs,
  programConfigSetAuthorityArgsBeet,
} from '../types/ProgramConfigSetAuthorityArgs'

/**
 * @category Instructions
 * @category SetProgramConfigAuthority
 * @category generated
 */
export type SetProgramConfigAuthorityInstructionArgs = {
  args: ProgramConfigSetAuthorityArgs
}
/**
 * @category Instructions
 * @category SetProgramConfigAuthority
 * @category generated
 */
export const setProgramConfigAuthorityStruct = new beet.BeetArgsStruct<
  SetProgramConfigAuthorityInstructionArgs & {
    instructionDiscriminator: number[] /* size: 8 */
  }
>(
  [
    ['instructionDiscriminator', beet.uniformFixedSizeArray(beet.u8, 8)],
    ['args', programConfigSetAuthorityArgsBeet],
  ],
  'SetProgramConfigAuthorityInstructionArgs'
)
/**
 * Accounts required by the _setProgramConfigAuthority_ instruction
 *
 * @property [_writable_] programConfig
 * @property [**signer**] authority
 * @category Instructions
 * @category SetProgramConfigAuthority
 * @category generated
 */
export type SetProgramConfigAuthorityInstructionAccounts = {
  programConfig: web3.PublicKey
  authority: web3.PublicKey
  anchorRemainingAccounts?: web3.AccountMeta[]
}

export const setProgramConfigAuthorityInstructionDiscriminator = [
  130, 40, 234, 111, 237, 155, 246, 203,
]

/**
 * Creates a _SetProgramConfigAuthority_ instruction.
 *
 * @param accounts that will be accessed while the instruction is processed
 * @param args to provide as instruction data to the program
 *
 * @category Instructions
 * @category SetProgramConfigAuthority
 * @category generated
 */
export function createSetProgramConfigAuthorityInstruction(
  accounts: SetProgramConfigAuthorityInstructionAccounts,
  args: SetProgramConfigAuthorityInstructionArgs,
  programId = new web3.PublicKey('SMRTzfY6DfH5ik3TKiyLFfXexV8uSG3d2UksSCYdunG')
) {
  const [data] = setProgramConfigAuthorityStruct.serialize({
    instructionDiscriminator: setProgramConfigAuthorityInstructionDiscriminator,
    ...args,
  })
  const keys: web3.AccountMeta[] = [
    {
      pubkey: accounts.programConfig,
      isWritable: true,
      isSigner: false,
    },
    {
      pubkey: accounts.authority,
      isWritable: false,
      isSigner: true,
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
