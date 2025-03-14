/**
 * This code was GENERATED using the solita package.
 * Please DO NOT EDIT THIS FILE, instead rerun solita to update it or write a wrapper to add functionality.
 *
 * See: https://github.com/metaplex-foundation/solita
 */

import * as beet from '@metaplex-foundation/beet'
import * as web3 from '@solana/web3.js'
import {
  VoteOnProposalArgs,
  voteOnProposalArgsBeet,
} from '../types/VoteOnProposalArgs'

/**
 * @category Instructions
 * @category RejectProposal
 * @category generated
 */
export type RejectProposalInstructionArgs = {
  args: VoteOnProposalArgs
}
/**
 * @category Instructions
 * @category RejectProposal
 * @category generated
 */
export const rejectProposalStruct = new beet.FixableBeetArgsStruct<
  RejectProposalInstructionArgs & {
    instructionDiscriminator: number[] /* size: 8 */
  }
>(
  [
    ['instructionDiscriminator', beet.uniformFixedSizeArray(beet.u8, 8)],
    ['args', voteOnProposalArgsBeet],
  ],
  'RejectProposalInstructionArgs'
)
/**
 * Accounts required by the _rejectProposal_ instruction
 *
 * @property [] settings
 * @property [_writable_, **signer**] signer
 * @property [_writable_] proposal
 * @category Instructions
 * @category RejectProposal
 * @category generated
 */
export type RejectProposalInstructionAccounts = {
  settings: web3.PublicKey
  signer: web3.PublicKey
  proposal: web3.PublicKey
  systemProgram?: web3.PublicKey
  anchorRemainingAccounts?: web3.AccountMeta[]
}

export const rejectProposalInstructionDiscriminator = [
  114, 162, 164, 82, 191, 11, 102, 25,
]

/**
 * Creates a _RejectProposal_ instruction.
 *
 * Optional accounts that are not provided default to the program ID since
 * this was indicated in the IDL from which this instruction was generated.
 *
 * @param accounts that will be accessed while the instruction is processed
 * @param args to provide as instruction data to the program
 *
 * @category Instructions
 * @category RejectProposal
 * @category generated
 */
export function createRejectProposalInstruction(
  accounts: RejectProposalInstructionAccounts,
  args: RejectProposalInstructionArgs,
  programId = new web3.PublicKey('SMRTzfY6DfH5ik3TKiyLFfXexV8uSG3d2UksSCYdunG')
) {
  const [data] = rejectProposalStruct.serialize({
    instructionDiscriminator: rejectProposalInstructionDiscriminator,
    ...args,
  })
  const keys: web3.AccountMeta[] = [
    {
      pubkey: accounts.settings,
      isWritable: false,
      isSigner: false,
    },
    {
      pubkey: accounts.signer,
      isWritable: true,
      isSigner: true,
    },
    {
      pubkey: accounts.proposal,
      isWritable: true,
      isSigner: false,
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
