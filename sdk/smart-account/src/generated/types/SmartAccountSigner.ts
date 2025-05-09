/**
 * This code was GENERATED using the solita package.
 * Please DO NOT EDIT THIS FILE, instead rerun solita to update it or write a wrapper to add functionality.
 *
 * See: https://github.com/metaplex-foundation/solita
 */

import * as web3 from '@solana/web3.js'
import * as beetSolana from '@metaplex-foundation/beet-solana'
import * as beet from '@metaplex-foundation/beet'
import { Permissions, permissionsBeet } from './Permissions'
export type SmartAccountSigner = {
  key: web3.PublicKey
  permissions: Permissions
}

/**
 * @category userTypes
 * @category generated
 */
export const smartAccountSignerBeet =
  new beet.BeetArgsStruct<SmartAccountSigner>(
    [
      ['key', beetSolana.publicKey],
      ['permissions', permissionsBeet],
    ],
    'SmartAccountSigner'
  )
