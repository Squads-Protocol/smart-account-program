/**
 * This code was GENERATED using the solita package.
 * Please DO NOT EDIT THIS FILE, instead rerun solita to update it or write a wrapper to add functionality.
 *
 * See: https://github.com/metaplex-foundation/solita
 */

import * as beet from '@metaplex-foundation/beet'
import * as web3 from '@solana/web3.js'
import {
  MultisigCreateArgsV2,
  multisigCreateArgsV2Beet,
} from '../types/MultisigCreateArgsV2'

/**
 * @category Instructions
 * @category MultisigCreateV2
 * @category generated
 */
export type MultisigCreateV2InstructionArgs = {
  args: MultisigCreateArgsV2
}
/**
 * @category Instructions
 * @category MultisigCreateV2
 * @category generated
 */
export const multisigCreateV2Struct = new beet.FixableBeetArgsStruct<
  MultisigCreateV2InstructionArgs & {
    instructionDiscriminator: number[] /* size: 8 */
  }
>(
  [
    ['instructionDiscriminator', beet.uniformFixedSizeArray(beet.u8, 8)],
    ['args', multisigCreateArgsV2Beet],
  ],
  'MultisigCreateV2InstructionArgs'
)
/**
 * Accounts required by the _multisigCreateV2_ instruction
 *
 * @property [] programConfig
 * @property [_writable_] treasury
 * @property [_writable_] multisig
 * @property [**signer**] createKey
 * @property [_writable_, **signer**] creator
 * @category Instructions
 * @category MultisigCreateV2
 * @category generated
 */
export type MultisigCreateV2InstructionAccounts = {
  programConfig: web3.PublicKey
  treasury: web3.PublicKey
  multisig: web3.PublicKey
  createKey: web3.PublicKey
  creator: web3.PublicKey
  systemProgram?: web3.PublicKey
  anchorRemainingAccounts?: web3.AccountMeta[]
}

export const multisigCreateV2InstructionDiscriminator = [
  50, 221, 199, 93, 40, 245, 139, 233,
]

/**
 * Creates a _MultisigCreateV2_ instruction.
 *
 * @param accounts that will be accessed while the instruction is processed
 * @param args to provide as instruction data to the program
 *
 * @category Instructions
 * @category MultisigCreateV2
 * @category generated
 */
export function createMultisigCreateV2Instruction(
  accounts: MultisigCreateV2InstructionAccounts,
  args: MultisigCreateV2InstructionArgs,
  programId = new web3.PublicKey('SMRTe6bnZAgJmXt9aJin7XgAzDn1XMHGNy95QATyzpk')
) {
  const [data] = multisigCreateV2Struct.serialize({
    instructionDiscriminator: multisigCreateV2InstructionDiscriminator,
    ...args,
  })
  const keys: web3.AccountMeta[] = [
    {
      pubkey: accounts.programConfig,
      isWritable: false,
      isSigner: false,
    },
    {
      pubkey: accounts.treasury,
      isWritable: true,
      isSigner: false,
    },
    {
      pubkey: accounts.multisig,
      isWritable: true,
      isSigner: false,
    },
    {
      pubkey: accounts.createKey,
      isWritable: false,
      isSigner: true,
    },
    {
      pubkey: accounts.creator,
      isWritable: true,
      isSigner: true,
    },
    {
      pubkey: accounts.systemProgram ?? web3.SystemProgram.programId,
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
