/**
 * This code was GENERATED using the solita package.
 * Please DO NOT EDIT THIS FILE, instead rerun solita to update it or write a wrapper to add functionality.
 *
 * See: https://github.com/metaplex-foundation/solita
 */

import * as beet from '@metaplex-foundation/beet'
import * as web3 from '@solana/web3.js'
import {
  ProgramConfigSetTreasuryArgs,
  programConfigSetTreasuryArgsBeet,
} from '../types/ProgramConfigSetTreasuryArgs'

/**
 * @category Instructions
 * @category SetProgramConfigTreasury
 * @category generated
 */
export type SetProgramConfigTreasuryInstructionArgs = {
  args: ProgramConfigSetTreasuryArgs
}
/**
 * @category Instructions
 * @category SetProgramConfigTreasury
 * @category generated
 */
export const setProgramConfigTreasuryStruct = new beet.BeetArgsStruct<
  SetProgramConfigTreasuryInstructionArgs & {
    instructionDiscriminator: number[] /* size: 8 */
  }
>(
  [
    ['instructionDiscriminator', beet.uniformFixedSizeArray(beet.u8, 8)],
    ['args', programConfigSetTreasuryArgsBeet],
  ],
  'SetProgramConfigTreasuryInstructionArgs'
)
/**
 * Accounts required by the _setProgramConfigTreasury_ instruction
 *
 * @property [_writable_] programConfig
 * @property [**signer**] authority
 * @category Instructions
 * @category SetProgramConfigTreasury
 * @category generated
 */
export type SetProgramConfigTreasuryInstructionAccounts = {
  programConfig: web3.PublicKey
  authority: web3.PublicKey
  anchorRemainingAccounts?: web3.AccountMeta[]
}

export const setProgramConfigTreasuryInstructionDiscriminator = [
  244, 119, 192, 190, 182, 101, 227, 189,
]

/**
 * Creates a _SetProgramConfigTreasury_ instruction.
 *
 * @param accounts that will be accessed while the instruction is processed
 * @param args to provide as instruction data to the program
 *
 * @category Instructions
 * @category SetProgramConfigTreasury
 * @category generated
 */
export function createSetProgramConfigTreasuryInstruction(
  accounts: SetProgramConfigTreasuryInstructionAccounts,
  args: SetProgramConfigTreasuryInstructionArgs,
  programId = new web3.PublicKey('SMRTzfY6DfH5ik3TKiyLFfXexV8uSG3d2UksSCYdunG')
) {
  const [data] = setProgramConfigTreasuryStruct.serialize({
    instructionDiscriminator: setProgramConfigTreasuryInstructionDiscriminator,
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
