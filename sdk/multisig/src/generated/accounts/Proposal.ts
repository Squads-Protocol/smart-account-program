/**
 * This code was GENERATED using the solita package.
 * Please DO NOT EDIT THIS FILE, instead rerun solita to update it or write a wrapper to add functionality.
 *
 * See: https://github.com/metaplex-foundation/solita
 */

import * as web3 from '@solana/web3.js'
import * as beet from '@metaplex-foundation/beet'
import * as beetSolana from '@metaplex-foundation/beet-solana'
import { ProposalStatus, proposalStatusBeet } from '../types/ProposalStatus'

/**
 * Arguments used to create {@link Proposal}
 * @category Accounts
 * @category generated
 */
export type ProposalArgs = {
  multisig: web3.PublicKey
  transactionIndex: beet.bignum
  status: ProposalStatus
  bump: number
  approved: web3.PublicKey[]
  rejected: web3.PublicKey[]
  cancelled: web3.PublicKey[]
}

export const proposalDiscriminator = [26, 94, 189, 187, 116, 136, 53, 33]
/**
 * Holds the data for the {@link Proposal} Account and provides de/serialization
 * functionality for that data
 *
 * @category Accounts
 * @category generated
 */
export class Proposal implements ProposalArgs {
  private constructor(
    readonly multisig: web3.PublicKey,
    readonly transactionIndex: beet.bignum,
    readonly status: ProposalStatus,
    readonly bump: number,
    readonly approved: web3.PublicKey[],
    readonly rejected: web3.PublicKey[],
    readonly cancelled: web3.PublicKey[]
  ) {}

  /**
   * Creates a {@link Proposal} instance from the provided args.
   */
  static fromArgs(args: ProposalArgs) {
    return new Proposal(
      args.multisig,
      args.transactionIndex,
      args.status,
      args.bump,
      args.approved,
      args.rejected,
      args.cancelled
    )
  }

  /**
   * Deserializes the {@link Proposal} from the data of the provided {@link web3.AccountInfo}.
   * @returns a tuple of the account data and the offset up to which the buffer was read to obtain it.
   */
  static fromAccountInfo(
    accountInfo: web3.AccountInfo<Buffer>,
    offset = 0
  ): [Proposal, number] {
    return Proposal.deserialize(accountInfo.data, offset)
  }

  /**
   * Retrieves the account info from the provided address and deserializes
   * the {@link Proposal} from its data.
   *
   * @throws Error if no account info is found at the address or if deserialization fails
   */
  static async fromAccountAddress(
    connection: web3.Connection,
    address: web3.PublicKey,
    commitmentOrConfig?: web3.Commitment | web3.GetAccountInfoConfig
  ): Promise<Proposal> {
    const accountInfo = await connection.getAccountInfo(
      address,
      commitmentOrConfig
    )
    if (accountInfo == null) {
      throw new Error(`Unable to find Proposal account at ${address}`)
    }
    return Proposal.fromAccountInfo(accountInfo, 0)[0]
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
    return beetSolana.GpaBuilder.fromStruct(programId, proposalBeet)
  }

  /**
   * Deserializes the {@link Proposal} from the provided data Buffer.
   * @returns a tuple of the account data and the offset up to which the buffer was read to obtain it.
   */
  static deserialize(buf: Buffer, offset = 0): [Proposal, number] {
    return proposalBeet.deserialize(buf, offset)
  }

  /**
   * Serializes the {@link Proposal} into a Buffer.
   * @returns a tuple of the created Buffer and the offset up to which the buffer was written to store it.
   */
  serialize(): [Buffer, number] {
    return proposalBeet.serialize({
      accountDiscriminator: proposalDiscriminator,
      ...this,
    })
  }

  /**
   * Returns the byteSize of a {@link Buffer} holding the serialized data of
   * {@link Proposal} for the provided args.
   *
   * @param args need to be provided since the byte size for this account
   * depends on them
   */
  static byteSize(args: ProposalArgs) {
    const instance = Proposal.fromArgs(args)
    return proposalBeet.toFixedFromValue({
      accountDiscriminator: proposalDiscriminator,
      ...instance,
    }).byteSize
  }

  /**
   * Fetches the minimum balance needed to exempt an account holding
   * {@link Proposal} data from rent
   *
   * @param args need to be provided since the byte size for this account
   * depends on them
   * @param connection used to retrieve the rent exemption information
   */
  static async getMinimumBalanceForRentExemption(
    args: ProposalArgs,
    connection: web3.Connection,
    commitment?: web3.Commitment
  ): Promise<number> {
    return connection.getMinimumBalanceForRentExemption(
      Proposal.byteSize(args),
      commitment
    )
  }

  /**
   * Returns a readable version of {@link Proposal} properties
   * and can be used to convert to JSON and/or logging
   */
  pretty() {
    return {
      multisig: this.multisig.toBase58(),
      transactionIndex: (() => {
        const x = <{ toNumber: () => number }>this.transactionIndex
        if (typeof x.toNumber === 'function') {
          try {
            return x.toNumber()
          } catch (_) {
            return x
          }
        }
        return x
      })(),
      status: this.status.__kind,
      bump: this.bump,
      approved: this.approved,
      rejected: this.rejected,
      cancelled: this.cancelled,
    }
  }
}

/**
 * @category Accounts
 * @category generated
 */
export const proposalBeet = new beet.FixableBeetStruct<
  Proposal,
  ProposalArgs & {
    accountDiscriminator: number[] /* size: 8 */
  }
>(
  [
    ['accountDiscriminator', beet.uniformFixedSizeArray(beet.u8, 8)],
    ['multisig', beetSolana.publicKey],
    ['transactionIndex', beet.u64],
    ['status', proposalStatusBeet],
    ['bump', beet.u8],
    ['approved', beet.array(beetSolana.publicKey)],
    ['rejected', beet.array(beetSolana.publicKey)],
    ['cancelled', beet.array(beetSolana.publicKey)],
  ],
  Proposal.fromArgs,
  'Proposal'
)
