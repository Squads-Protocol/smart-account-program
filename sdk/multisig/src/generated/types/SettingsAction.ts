/**
 * This code was GENERATED using the solita package.
 * Please DO NOT EDIT THIS FILE, instead rerun solita to update it or write a wrapper to add functionality.
 *
 * See: https://github.com/metaplex-foundation/solita
 */

import * as web3 from '@solana/web3.js'
import * as beet from '@metaplex-foundation/beet'
import * as beetSolana from '@metaplex-foundation/beet-solana'
import {
  SmartAccountSigner,
  smartAccountSignerBeet,
} from './SmartAccountSigner'
import { Period, periodBeet } from './Period'
/**
 * This type is used to derive the {@link SettingsAction} type as well as the de/serializer.
 * However don't refer to it in your code but use the {@link SettingsAction} type instead.
 *
 * @category userTypes
 * @category enums
 * @category generated
 * @private
 */
export type SettingsActionRecord = {
  AddSigner: { newSigner: SmartAccountSigner }
  RemoveSigner: { oldSigner: web3.PublicKey }
  ChangeThreshold: { newThreshold: number }
  SetTimeLock: { newTimeLock: number }
  AddSpendingLimit: {
    seed: web3.PublicKey
    accountIndex: number
    mint: web3.PublicKey
    amount: beet.bignum
    period: Period
    signers: web3.PublicKey[]
    destinations: web3.PublicKey[]
  }
  RemoveSpendingLimit: { spendingLimit: web3.PublicKey }
  SetRentCollector: { newRentCollector: beet.COption<web3.PublicKey> }
}

/**
 * Union type respresenting the SettingsAction data enum defined in Rust.
 *
 * NOTE: that it includes a `__kind` property which allows to narrow types in
 * switch/if statements.
 * Additionally `isSettingsAction*` type guards are exposed below to narrow to a specific variant.
 *
 * @category userTypes
 * @category enums
 * @category generated
 */
export type SettingsAction = beet.DataEnumKeyAsKind<SettingsActionRecord>

export const isSettingsActionAddSigner = (
  x: SettingsAction
): x is SettingsAction & { __kind: 'AddSigner' } => x.__kind === 'AddSigner'
export const isSettingsActionRemoveSigner = (
  x: SettingsAction
): x is SettingsAction & { __kind: 'RemoveSigner' } =>
  x.__kind === 'RemoveSigner'
export const isSettingsActionChangeThreshold = (
  x: SettingsAction
): x is SettingsAction & { __kind: 'ChangeThreshold' } =>
  x.__kind === 'ChangeThreshold'
export const isSettingsActionSetTimeLock = (
  x: SettingsAction
): x is SettingsAction & { __kind: 'SetTimeLock' } => x.__kind === 'SetTimeLock'
export const isSettingsActionAddSpendingLimit = (
  x: SettingsAction
): x is SettingsAction & { __kind: 'AddSpendingLimit' } =>
  x.__kind === 'AddSpendingLimit'
export const isSettingsActionRemoveSpendingLimit = (
  x: SettingsAction
): x is SettingsAction & { __kind: 'RemoveSpendingLimit' } =>
  x.__kind === 'RemoveSpendingLimit'
export const isSettingsActionSetRentCollector = (
  x: SettingsAction
): x is SettingsAction & { __kind: 'SetRentCollector' } =>
  x.__kind === 'SetRentCollector'

/**
 * @category userTypes
 * @category generated
 */
export const settingsActionBeet = beet.dataEnum<SettingsActionRecord>([
  [
    'AddSigner',
    new beet.BeetArgsStruct<SettingsActionRecord['AddSigner']>(
      [['newSigner', smartAccountSignerBeet]],
      'SettingsActionRecord["AddSigner"]'
    ),
  ],

  [
    'RemoveSigner',
    new beet.BeetArgsStruct<SettingsActionRecord['RemoveSigner']>(
      [['oldSigner', beetSolana.publicKey]],
      'SettingsActionRecord["RemoveSigner"]'
    ),
  ],

  [
    'ChangeThreshold',
    new beet.BeetArgsStruct<SettingsActionRecord['ChangeThreshold']>(
      [['newThreshold', beet.u16]],
      'SettingsActionRecord["ChangeThreshold"]'
    ),
  ],

  [
    'SetTimeLock',
    new beet.BeetArgsStruct<SettingsActionRecord['SetTimeLock']>(
      [['newTimeLock', beet.u32]],
      'SettingsActionRecord["SetTimeLock"]'
    ),
  ],

  [
    'AddSpendingLimit',
    new beet.FixableBeetArgsStruct<SettingsActionRecord['AddSpendingLimit']>(
      [
        ['seed', beetSolana.publicKey],
        ['accountIndex', beet.u8],
        ['mint', beetSolana.publicKey],
        ['amount', beet.u64],
        ['period', periodBeet],
        ['signers', beet.array(beetSolana.publicKey)],
        ['destinations', beet.array(beetSolana.publicKey)],
      ],
      'SettingsActionRecord["AddSpendingLimit"]'
    ),
  ],

  [
    'RemoveSpendingLimit',
    new beet.BeetArgsStruct<SettingsActionRecord['RemoveSpendingLimit']>(
      [['spendingLimit', beetSolana.publicKey]],
      'SettingsActionRecord["RemoveSpendingLimit"]'
    ),
  ],

  [
    'SetRentCollector',
    new beet.FixableBeetArgsStruct<SettingsActionRecord['SetRentCollector']>(
      [['newRentCollector', beet.coption(beetSolana.publicKey)]],
      'SettingsActionRecord["SetRentCollector"]'
    ),
  ],
]) as beet.FixableBeet<SettingsAction, SettingsAction>
