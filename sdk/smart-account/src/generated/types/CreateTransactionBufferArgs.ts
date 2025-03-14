/**
 * This code was GENERATED using the solita package.
 * Please DO NOT EDIT THIS FILE, instead rerun solita to update it or write a wrapper to add functionality.
 *
 * See: https://github.com/metaplex-foundation/solita
 */

import * as beet from '@metaplex-foundation/beet'
export type CreateTransactionBufferArgs = {
  bufferIndex: number
  accountIndex: number
  finalBufferHash: number[] /* size: 32 */
  finalBufferSize: number
  buffer: Uint8Array
}

/**
 * @category userTypes
 * @category generated
 */
export const createTransactionBufferArgsBeet =
  new beet.FixableBeetArgsStruct<CreateTransactionBufferArgs>(
    [
      ['bufferIndex', beet.u8],
      ['accountIndex', beet.u8],
      ['finalBufferHash', beet.uniformFixedSizeArray(beet.u8, 32)],
      ['finalBufferSize', beet.u16],
      ['buffer', beet.bytes],
    ],
    'CreateTransactionBufferArgs'
  )
