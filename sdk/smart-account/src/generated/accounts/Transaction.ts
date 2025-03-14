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
  SmartAccountTransactionMessage,
  smartAccountTransactionMessageBeet,
} from '../types/SmartAccountTransactionMessage'

/**
 * Arguments used to create {@link Transaction}
 * @category Accounts
 * @category generated
 */
export type TransactionArgs = {
  settings: web3.PublicKey
  creator: web3.PublicKey
  rentCollector: web3.PublicKey
  index: beet.bignum
  bump: number
  accountIndex: number
  accountBump: number
  ephemeralSignerBumps: Uint8Array
  message: SmartAccountTransactionMessage
}

export const transactionDiscriminator = [11, 24, 174, 129, 203, 117, 242, 23]
/**
 * Holds the data for the {@link Transaction} Account and provides de/serialization
 * functionality for that data
 *
 * @category Accounts
 * @category generated
 */
export class Transaction implements TransactionArgs {
  private constructor(
    readonly settings: web3.PublicKey,
    readonly creator: web3.PublicKey,
    readonly rentCollector: web3.PublicKey,
    readonly index: beet.bignum,
    readonly bump: number,
    readonly accountIndex: number,
    readonly accountBump: number,
    readonly ephemeralSignerBumps: Uint8Array,
    readonly message: SmartAccountTransactionMessage
  ) {}

  /**
   * Creates a {@link Transaction} instance from the provided args.
   */
  static fromArgs(args: TransactionArgs) {
    return new Transaction(
      args.settings,
      args.creator,
      args.rentCollector,
      args.index,
      args.bump,
      args.accountIndex,
      args.accountBump,
      args.ephemeralSignerBumps,
      args.message
    )
  }

  /**
   * Deserializes the {@link Transaction} from the data of the provided {@link web3.AccountInfo}.
   * @returns a tuple of the account data and the offset up to which the buffer was read to obtain it.
   */
  static fromAccountInfo(
    accountInfo: web3.AccountInfo<Buffer>,
    offset = 0
  ): [Transaction, number] {
    return Transaction.deserialize(accountInfo.data, offset)
  }

  /**
   * Retrieves the account info from the provided address and deserializes
   * the {@link Transaction} from its data.
   *
   * @throws Error if no account info is found at the address or if deserialization fails
   */
  static async fromAccountAddress(
    connection: web3.Connection,
    address: web3.PublicKey,
    commitmentOrConfig?: web3.Commitment | web3.GetAccountInfoConfig
  ): Promise<Transaction> {
    const accountInfo = await connection.getAccountInfo(
      address,
      commitmentOrConfig
    )
    if (accountInfo == null) {
      throw new Error(`Unable to find Transaction account at ${address}`)
    }
    return Transaction.fromAccountInfo(accountInfo, 0)[0]
  }

  /**
   * Provides a {@link web3.Connection.getProgramAccounts} config builder,
   * to fetch accounts matching filters that can be specified via that builder.
   *
   * @param programId - the program that owns the accounts we are filtering
   */
  static gpaBuilder(
    programId: web3.PublicKey = new web3.PublicKey(
      'SMRTzfY6DfH5ik3TKiyLFfXexV8uSG3d2UksSCYdunG'
    )
  ) {
    return beetSolana.GpaBuilder.fromStruct(programId, transactionBeet)
  }

  /**
   * Deserializes the {@link Transaction} from the provided data Buffer.
   * @returns a tuple of the account data and the offset up to which the buffer was read to obtain it.
   */
  static deserialize(buf: Buffer, offset = 0): [Transaction, number] {
    return transactionBeet.deserialize(buf, offset)
  }

  /**
   * Serializes the {@link Transaction} into a Buffer.
   * @returns a tuple of the created Buffer and the offset up to which the buffer was written to store it.
   */
  serialize(): [Buffer, number] {
    return transactionBeet.serialize({
      accountDiscriminator: transactionDiscriminator,
      ...this,
    })
  }

  /**
   * Returns the byteSize of a {@link Buffer} holding the serialized data of
   * {@link Transaction} for the provided args.
   *
   * @param args need to be provided since the byte size for this account
   * depends on them
   */
  static byteSize(args: TransactionArgs) {
    const instance = Transaction.fromArgs(args)
    return transactionBeet.toFixedFromValue({
      accountDiscriminator: transactionDiscriminator,
      ...instance,
    }).byteSize
  }

  /**
   * Fetches the minimum balance needed to exempt an account holding
   * {@link Transaction} data from rent
   *
   * @param args need to be provided since the byte size for this account
   * depends on them
   * @param connection used to retrieve the rent exemption information
   */
  static async getMinimumBalanceForRentExemption(
    args: TransactionArgs,
    connection: web3.Connection,
    commitment?: web3.Commitment
  ): Promise<number> {
    return connection.getMinimumBalanceForRentExemption(
      Transaction.byteSize(args),
      commitment
    )
  }

  /**
   * Returns a readable version of {@link Transaction} properties
   * and can be used to convert to JSON and/or logging
   */
  pretty() {
    return {
      settings: this.settings.toBase58(),
      creator: this.creator.toBase58(),
      rentCollector: this.rentCollector.toBase58(),
      index: (() => {
        const x = <{ toNumber: () => number }>this.index
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
      accountIndex: this.accountIndex,
      accountBump: this.accountBump,
      ephemeralSignerBumps: this.ephemeralSignerBumps,
      message: this.message,
    }
  }
}

/**
 * @category Accounts
 * @category generated
 */
export const transactionBeet = new beet.FixableBeetStruct<
  Transaction,
  TransactionArgs & {
    accountDiscriminator: number[] /* size: 8 */
  }
>(
  [
    ['accountDiscriminator', beet.uniformFixedSizeArray(beet.u8, 8)],
    ['settings', beetSolana.publicKey],
    ['creator', beetSolana.publicKey],
    ['rentCollector', beetSolana.publicKey],
    ['index', beet.u64],
    ['bump', beet.u8],
    ['accountIndex', beet.u8],
    ['accountBump', beet.u8],
    ['ephemeralSignerBumps', beet.bytes],
    ['message', smartAccountTransactionMessageBeet],
  ],
  Transaction.fromArgs,
  'Transaction'
)
