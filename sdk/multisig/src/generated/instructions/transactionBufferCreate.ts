/**
 * This code was GENERATED using the solita package.
 * Please DO NOT EDIT THIS FILE, instead rerun solita to update it or write a wrapper to add functionality.
 *
 * See: https://github.com/metaplex-foundation/solita
 */

import * as beet from '@metaplex-foundation/beet'
import * as web3 from '@solana/web3.js'
import {
  TransactionBufferCreateArgs,
  transactionBufferCreateArgsBeet,
} from '../types/TransactionBufferCreateArgs'

/**
 * @category Instructions
 * @category TransactionBufferCreate
 * @category generated
 */
export type TransactionBufferCreateInstructionArgs = {
  args: TransactionBufferCreateArgs
}
/**
 * @category Instructions
 * @category TransactionBufferCreate
 * @category generated
 */
export const transactionBufferCreateStruct = new beet.FixableBeetArgsStruct<
  TransactionBufferCreateInstructionArgs & {
    instructionDiscriminator: number[] /* size: 8 */
  }
>(
  [
    ['instructionDiscriminator', beet.uniformFixedSizeArray(beet.u8, 8)],
    ['args', transactionBufferCreateArgsBeet],
  ],
  'TransactionBufferCreateInstructionArgs'
)
/**
 * Accounts required by the _transactionBufferCreate_ instruction
 *
 * @property [] multisig
 * @property [_writable_] transactionBuffer
 * @property [**signer**] creator
 * @property [_writable_, **signer**] rentPayer
 * @category Instructions
 * @category TransactionBufferCreate
 * @category generated
 */
export type TransactionBufferCreateInstructionAccounts = {
  multisig: web3.PublicKey
  transactionBuffer: web3.PublicKey
  creator: web3.PublicKey
  rentPayer: web3.PublicKey
  systemProgram?: web3.PublicKey
  anchorRemainingAccounts?: web3.AccountMeta[]
}

export const transactionBufferCreateInstructionDiscriminator = [
  245, 201, 113, 108, 37, 63, 29, 89,
]

/**
 * Creates a _TransactionBufferCreate_ instruction.
 *
 * @param accounts that will be accessed while the instruction is processed
 * @param args to provide as instruction data to the program
 *
 * @category Instructions
 * @category TransactionBufferCreate
 * @category generated
 */
export function createTransactionBufferCreateInstruction(
  accounts: TransactionBufferCreateInstructionAccounts,
  args: TransactionBufferCreateInstructionArgs,
  programId = new web3.PublicKey('SMRTe6bnZAgJmXt9aJin7XgAzDn1XMHGNy95QATyzpk')
) {
  const [data] = transactionBufferCreateStruct.serialize({
    instructionDiscriminator: transactionBufferCreateInstructionDiscriminator,
    ...args,
  })
  const keys: web3.AccountMeta[] = [
    {
      pubkey: accounts.multisig,
      isWritable: false,
      isSigner: false,
    },
    {
      pubkey: accounts.transactionBuffer,
      isWritable: true,
      isSigner: false,
    },
    {
      pubkey: accounts.creator,
      isWritable: false,
      isSigner: true,
    },
    {
      pubkey: accounts.rentPayer,
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
