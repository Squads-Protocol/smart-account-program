/**
 * This code was GENERATED using the solita package.
 * Please DO NOT EDIT THIS FILE, instead rerun solita to update it or write a wrapper to add functionality.
 *
 * See: https://github.com/metaplex-foundation/solita
 */

import * as beet from '@metaplex-foundation/beet'
import * as web3 from '@solana/web3.js'

/**
 * @category Instructions
 * @category MultisigCreate
 * @category generated
 */
export const multisigCreateStruct = new beet.BeetArgsStruct<{
  instructionDiscriminator: number[] /* size: 8 */
}>(
  [['instructionDiscriminator', beet.uniformFixedSizeArray(beet.u8, 8)]],
  'MultisigCreateInstructionArgs'
)
/**
 * Accounts required by the _multisigCreate_ instruction
 *
 * @property [] null
 * @category Instructions
 * @category MultisigCreate
 * @category generated
 */
export type MultisigCreateInstructionAccounts = {
  null: web3.PublicKey
  anchorRemainingAccounts?: web3.AccountMeta[]
}

export const multisigCreateInstructionDiscriminator = [
  122, 77, 80, 159, 84, 88, 90, 197,
]

/**
 * Creates a _MultisigCreate_ instruction.
 *
 * @param accounts that will be accessed while the instruction is processed
 * @category Instructions
 * @category MultisigCreate
 * @category generated
 */
export function createMultisigCreateInstruction(
  accounts: MultisigCreateInstructionAccounts,
  programId = new web3.PublicKey('SMRTe6bnZAgJmXt9aJin7XgAzDn1XMHGNy95QATyzpk')
) {
  const [data] = multisigCreateStruct.serialize({
    instructionDiscriminator: multisigCreateInstructionDiscriminator,
  })
  const keys: web3.AccountMeta[] = [
    {
      pubkey: accounts.null,
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
