/**
 * This code was GENERATED using the solita package.
 * Please DO NOT EDIT THIS FILE, instead rerun solita to update it or write a wrapper to add functionality.
 *
 * See: https://github.com/metaplex-foundation/solita
 */

import * as beet from '@metaplex-foundation/beet'
export type ProposalCreateArgs = {
  transactionIndex: beet.bignum
  draft: boolean
}

/**
 * @category userTypes
 * @category generated
 */
export const proposalCreateArgsBeet =
  new beet.BeetArgsStruct<ProposalCreateArgs>(
    [
      ['transactionIndex', beet.u64],
      ['draft', beet.bool],
    ],
    'ProposalCreateArgs'
  )
