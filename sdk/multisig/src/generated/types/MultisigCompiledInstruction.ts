/**
 * This code was GENERATED using the solita package.
 * Please DO NOT EDIT THIS FILE, instead rerun solita to update it or write a wrapper to add functionality.
 *
 * See: https://github.com/metaplex-foundation/solita
 */

import * as beet from '@metaplex-foundation/beet'
export type MultisigCompiledInstruction = {
  programIdIndex: number
  accountIndexes: Uint8Array
  data: Uint8Array
}

/**
 * @category userTypes
 * @category generated
 */
export const multisigCompiledInstructionBeet =
  new beet.FixableBeetArgsStruct<MultisigCompiledInstruction>(
    [
      ['programIdIndex', beet.u8],
      ['accountIndexes', beet.bytes],
      ['data', beet.bytes],
    ],
    'MultisigCompiledInstruction'
  )
