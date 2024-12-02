/**
 * This code was GENERATED using the solita package.
 * Please DO NOT EDIT THIS FILE, instead rerun solita to update it or write a wrapper to add functionality.
 *
 * See: https://github.com/metaplex-foundation/solita
 */

import * as web3 from '@solana/web3.js'
import * as beet from '@metaplex-foundation/beet'
import * as beetSolana from '@metaplex-foundation/beet-solana'
import { Period, periodBeet } from '../types/Period'

/**
 * Arguments used to create {@link SpendingLimit}
 * @category Accounts
 * @category generated
 */
export type SpendingLimitArgs = {
  multisig: web3.PublicKey
  createKey: web3.PublicKey
  vaultIndex: number
  mint: web3.PublicKey
  amount: beet.bignum
  period: Period
  remainingAmount: beet.bignum
  lastReset: beet.bignum
  bump: number
  members: web3.PublicKey[]
  destinations: web3.PublicKey[]
}

export const spendingLimitDiscriminator = [10, 201, 27, 160, 218, 195, 222, 152]
/**
 * Holds the data for the {@link SpendingLimit} Account and provides de/serialization
 * functionality for that data
 *
 * @category Accounts
 * @category generated
 */
export class SpendingLimit implements SpendingLimitArgs {
  private constructor(
    readonly multisig: web3.PublicKey,
    readonly createKey: web3.PublicKey,
    readonly vaultIndex: number,
    readonly mint: web3.PublicKey,
    readonly amount: beet.bignum,
    readonly period: Period,
    readonly remainingAmount: beet.bignum,
    readonly lastReset: beet.bignum,
    readonly bump: number,
    readonly members: web3.PublicKey[],
    readonly destinations: web3.PublicKey[]
  ) {}

  /**
   * Creates a {@link SpendingLimit} instance from the provided args.
   */
  static fromArgs(args: SpendingLimitArgs) {
    return new SpendingLimit(
      args.multisig,
      args.createKey,
      args.vaultIndex,
      args.mint,
      args.amount,
      args.period,
      args.remainingAmount,
      args.lastReset,
      args.bump,
      args.members,
      args.destinations
    )
  }

  /**
   * Deserializes the {@link SpendingLimit} from the data of the provided {@link web3.AccountInfo}.
   * @returns a tuple of the account data and the offset up to which the buffer was read to obtain it.
   */
  static fromAccountInfo(
    accountInfo: web3.AccountInfo<Buffer>,
    offset = 0
  ): [SpendingLimit, number] {
    return SpendingLimit.deserialize(accountInfo.data, offset)
  }

  /**
   * Retrieves the account info from the provided address and deserializes
   * the {@link SpendingLimit} from its data.
   *
   * @throws Error if no account info is found at the address or if deserialization fails
   */
  static async fromAccountAddress(
    connection: web3.Connection,
    address: web3.PublicKey,
    commitmentOrConfig?: web3.Commitment | web3.GetAccountInfoConfig
  ): Promise<SpendingLimit> {
    const accountInfo = await connection.getAccountInfo(
      address,
      commitmentOrConfig
    )
    if (accountInfo == null) {
      throw new Error(`Unable to find SpendingLimit account at ${address}`)
    }
    return SpendingLimit.fromAccountInfo(accountInfo, 0)[0]
  }

  /**
   * Provides a {@link web3.Connection.getProgramAccounts} config builder,
   * to fetch accounts matching filters that can be specified via that builder.
   *
   * @param programId - the program that owns the accounts we are filtering
   */
  static gpaBuilder(
    programId: web3.PublicKey = new web3.PublicKey(
      'SMRTe6bnZAgJmXt9aJin7XgAzDn1XMHGNy95QATyzpk'
    )
  ) {
    return beetSolana.GpaBuilder.fromStruct(programId, spendingLimitBeet)
  }

  /**
   * Deserializes the {@link SpendingLimit} from the provided data Buffer.
   * @returns a tuple of the account data and the offset up to which the buffer was read to obtain it.
   */
  static deserialize(buf: Buffer, offset = 0): [SpendingLimit, number] {
    return spendingLimitBeet.deserialize(buf, offset)
  }

  /**
   * Serializes the {@link SpendingLimit} into a Buffer.
   * @returns a tuple of the created Buffer and the offset up to which the buffer was written to store it.
   */
  serialize(): [Buffer, number] {
    return spendingLimitBeet.serialize({
      accountDiscriminator: spendingLimitDiscriminator,
      ...this,
    })
  }

  /**
   * Returns the byteSize of a {@link Buffer} holding the serialized data of
   * {@link SpendingLimit} for the provided args.
   *
   * @param args need to be provided since the byte size for this account
   * depends on them
   */
  static byteSize(args: SpendingLimitArgs) {
    const instance = SpendingLimit.fromArgs(args)
    return spendingLimitBeet.toFixedFromValue({
      accountDiscriminator: spendingLimitDiscriminator,
      ...instance,
    }).byteSize
  }

  /**
   * Fetches the minimum balance needed to exempt an account holding
   * {@link SpendingLimit} data from rent
   *
   * @param args need to be provided since the byte size for this account
   * depends on them
   * @param connection used to retrieve the rent exemption information
   */
  static async getMinimumBalanceForRentExemption(
    args: SpendingLimitArgs,
    connection: web3.Connection,
    commitment?: web3.Commitment
  ): Promise<number> {
    return connection.getMinimumBalanceForRentExemption(
      SpendingLimit.byteSize(args),
      commitment
    )
  }

  /**
   * Returns a readable version of {@link SpendingLimit} properties
   * and can be used to convert to JSON and/or logging
   */
  pretty() {
    return {
      multisig: this.multisig.toBase58(),
      createKey: this.createKey.toBase58(),
      vaultIndex: this.vaultIndex,
      mint: this.mint.toBase58(),
      amount: (() => {
        const x = <{ toNumber: () => number }>this.amount
        if (typeof x.toNumber === 'function') {
          try {
            return x.toNumber()
          } catch (_) {
            return x
          }
        }
        return x
      })(),
      period: 'Period.' + Period[this.period],
      remainingAmount: (() => {
        const x = <{ toNumber: () => number }>this.remainingAmount
        if (typeof x.toNumber === 'function') {
          try {
            return x.toNumber()
          } catch (_) {
            return x
          }
        }
        return x
      })(),
      lastReset: (() => {
        const x = <{ toNumber: () => number }>this.lastReset
        if (typeof x.toNumber === 'function') {
          try {
            return x.toNumber()
          } catch (_) {
            return x
          }
        }
        return x
      })(),
      bump: this.bump,
      members: this.members,
      destinations: this.destinations,
    }
  }
}

/**
 * @category Accounts
 * @category generated
 */
export const spendingLimitBeet = new beet.FixableBeetStruct<
  SpendingLimit,
  SpendingLimitArgs & {
    accountDiscriminator: number[] /* size: 8 */
  }
>(
  [
    ['accountDiscriminator', beet.uniformFixedSizeArray(beet.u8, 8)],
    ['multisig', beetSolana.publicKey],
    ['createKey', beetSolana.publicKey],
    ['vaultIndex', beet.u8],
    ['mint', beetSolana.publicKey],
    ['amount', beet.u64],
    ['period', periodBeet],
    ['remainingAmount', beet.u64],
    ['lastReset', beet.i64],
    ['bump', beet.u8],
    ['members', beet.array(beetSolana.publicKey)],
    ['destinations', beet.array(beetSolana.publicKey)],
  ],
  SpendingLimit.fromArgs,
  'SpendingLimit'
)
