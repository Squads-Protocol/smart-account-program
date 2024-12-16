/**
 * This code was GENERATED using the solita package.
 * Please DO NOT EDIT THIS FILE, instead rerun solita to update it or write a wrapper to add functionality.
 *
 * See: https://github.com/metaplex-foundation/solita
 */

import * as beet from '@metaplex-foundation/beet'
import { SettingsAction, settingsActionBeet } from './SettingsAction'
export type CreateSettingsTransactionArgs = {
  actions: SettingsAction[]
  memo: beet.COption<string>
}

/**
 * @category userTypes
 * @category generated
 */
export const createSettingsTransactionArgsBeet =
  new beet.FixableBeetArgsStruct<CreateSettingsTransactionArgs>(
    [
      ['actions', beet.array(settingsActionBeet)],
      ['memo', beet.coption(beet.utf8String)],
    ],
    'CreateSettingsTransactionArgs'
  )
