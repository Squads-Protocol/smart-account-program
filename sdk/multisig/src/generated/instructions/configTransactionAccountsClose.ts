/**
 * This code was GENERATED using the solita package.
 * Please DO NOT EDIT THIS FILE, instead rerun solita to update it or write a wrapper to add functionality.
 *
 * See: https://github.com/metaplex-foundation/solita
 */

import * as beet from '@metaplex-foundation/beet'
import * as web3 from '@solana/web3.js'

/**
 * @category Instructions
 * @category ConfigTransactionAccountsClose
 * @category generated
 */
export const configTransactionAccountsCloseStruct = new beet.BeetArgsStruct<{
  instructionDiscriminator: number[] /* size: 8 */
}>(
  [['instructionDiscriminator', beet.uniformFixedSizeArray(beet.u8, 8)]],
  'ConfigTransactionAccountsCloseInstructionArgs'
)
/**
 * Accounts required by the _configTransactionAccountsClose_ instruction
 *
 * @property [] multisig
 * @property [_writable_] proposal
 * @property [_writable_] transaction
 * @property [_writable_] rentCollector
 * @category Instructions
 * @category ConfigTransactionAccountsClose
 * @category generated
 */
export type ConfigTransactionAccountsCloseInstructionAccounts = {
  multisig: web3.PublicKey
  proposal: web3.PublicKey
  transaction: web3.PublicKey
  rentCollector: web3.PublicKey
  systemProgram?: web3.PublicKey
  anchorRemainingAccounts?: web3.AccountMeta[]
}

export const configTransactionAccountsCloseInstructionDiscriminator = [
  80, 203, 84, 53, 151, 112, 187, 186,
]

/**
 * Creates a _ConfigTransactionAccountsClose_ instruction.
 *
 * @param accounts that will be accessed while the instruction is processed
 * @category Instructions
 * @category ConfigTransactionAccountsClose
 * @category generated
 */
export function createConfigTransactionAccountsCloseInstruction(
  accounts: ConfigTransactionAccountsCloseInstructionAccounts,
  programId = new web3.PublicKey('SMRTe6bnZAgJmXt9aJin7XgAzDn1XMHGNy95QATyzpk')
) {
  const [data] = configTransactionAccountsCloseStruct.serialize({
    instructionDiscriminator:
      configTransactionAccountsCloseInstructionDiscriminator,
  })
  const keys: web3.AccountMeta[] = [
    {
      pubkey: accounts.multisig,
      isWritable: false,
      isSigner: false,
    },
    {
      pubkey: accounts.proposal,
      isWritable: true,
      isSigner: false,
    },
    {
      pubkey: accounts.transaction,
      isWritable: true,
      isSigner: false,
    },
    {
      pubkey: accounts.rentCollector,
      isWritable: true,
      isSigner: false,
    },
    {
      pubkey: accounts.systemProgram ?? web3.SystemProgram.programId,
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
