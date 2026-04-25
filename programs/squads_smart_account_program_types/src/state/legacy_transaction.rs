use solana_program::pubkey::Pubkey;

use crate::errors::SmartAccountError;
use crate::instructions::{CompiledInstruction, MessageAddressTableLookup, TransactionMessage};

#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
#[derive(Clone, Default)]
pub struct LegacyTransaction {
    /// The consensus account this belongs to.
    pub smart_account_settings: Pubkey,
    /// Signer of the Smart Account who submitted the transaction.
    pub creator: Pubkey,
    /// The rent collector for the transaction account.
    pub rent_collector: Pubkey,
    /// Index of this transaction within the smart account.
    pub index: u64,
    /// bump for the transaction seeds.
    pub bump: u8,
    /// The account index of the smart account this transaction belongs to.
    pub account_index: u8,
    /// Derivation bump of the smart account PDA this transaction belongs to.
    pub account_bump: u8,
    /// Derivation bumps for additional signers.
    pub ephemeral_signer_bumps: Vec<u8>,
    /// data required for executing the transaction.
    pub message: SmartAccountTransactionMessage,
}

impl LegacyTransaction {
    pub const DISCRIMINATOR: [u8; 8] = [0x90, 0x93, 0x41, 0xa9, 0x91, 0xe3, 0x39, 0x33];

    /// Reduces the transaction to its default empty value and moves
    /// ownership of the data to the caller.
    pub fn take(&mut self) -> LegacyTransaction {
        core::mem::take(self)
    }
}

#[cfg(feature = "borsh")]
impl LegacyTransaction {
    pub fn try_deserialize(data: &[u8]) -> Result<Self, borsh::maybestd::io::Error> {
        if data.len() < 8 || data[..8] != Self::DISCRIMINATOR {
            return Err(borsh::maybestd::io::Error::new(
                borsh::maybestd::io::ErrorKind::InvalidData,
                "discriminator mismatch",
            ));
        }
        let mut body = &data[8..];
        <Self as borsh::BorshDeserialize>::deserialize(&mut body)
    }

    pub fn size(
        ephemeral_signers_length: u8,
        transaction_message: &[u8],
    ) -> Result<usize, borsh::maybestd::io::Error> {
        let tm =
            <TransactionMessage as borsh::BorshDeserialize>::try_from_slice(transaction_message)?;
        let sm: SmartAccountTransactionMessage = tm.try_into().map_err(|_| {
            borsh::maybestd::io::Error::new(
                borsh::maybestd::io::ErrorKind::InvalidData,
                "invalid transaction message",
            )
        })?;
        let message_size = borsh::to_vec(&sm)?.len();
        Ok(8 +   // anchor account discriminator
            32 +  // settings
            32 +  // creator
            32 +  // rent_collector
            8 +   // index
            1 +   // bump
            1 +   // account_index
            1 +   // account_bump
            (4 + usize::from(ephemeral_signers_length)) +   // ephemeral_signers_bumps vec
            message_size)
    }
}

#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
#[derive(Clone, Default, Eq, PartialEq)]
pub struct SmartAccountTransactionMessage {
    pub num_signers: u8,
    pub num_writable_signers: u8,
    pub num_writable_non_signers: u8,
    pub account_keys: Vec<Pubkey>,
    pub instructions: Vec<SmartAccountCompiledInstruction>,
    pub address_table_lookups: Vec<SmartAccountMessageAddressTableLookup>,
}

impl SmartAccountTransactionMessage {
    pub fn num_all_account_keys(&self) -> usize {
        let num_account_keys_from_lookups = self
            .address_table_lookups
            .iter()
            .map(|lookup| lookup.writable_indexes.len() + lookup.readonly_indexes.len())
            .sum::<usize>();
        self.account_keys.len() + num_account_keys_from_lookups
    }

    pub fn is_static_writable_index(&self, key_index: usize) -> bool {
        let num_account_keys = self.account_keys.len();
        let num_signers = usize::from(self.num_signers);
        let num_writable_signers = usize::from(self.num_writable_signers);
        let num_writable_non_signers = usize::from(self.num_writable_non_signers);

        if key_index >= num_account_keys {
            return false;
        }
        if key_index < num_writable_signers {
            return true;
        }
        if key_index >= num_signers {
            let index_into_non_signers = key_index.saturating_sub(num_signers);
            return index_into_non_signers < num_writable_non_signers;
        }
        false
    }

    pub fn is_signer_index(&self, key_index: usize) -> bool {
        key_index < usize::from(self.num_signers)
    }
}

impl TryFrom<TransactionMessage> for SmartAccountTransactionMessage {
    type Error = SmartAccountError;

    fn try_from(message: TransactionMessage) -> Result<Self, Self::Error> {
        let account_keys: Vec<Pubkey> = message.account_keys.into();
        let instructions: Vec<CompiledInstruction> = message.instructions.into();
        let instructions: Vec<SmartAccountCompiledInstruction> = instructions
            .into_iter()
            .map(SmartAccountCompiledInstruction::from)
            .collect();
        let address_table_lookups: Vec<MessageAddressTableLookup> =
            message.address_table_lookups.into();

        let num_all_account_keys = account_keys.len()
            + address_table_lookups
                .iter()
                .map(|lookup| lookup.writable_indexes.len() + lookup.readonly_indexes.len())
                .sum::<usize>();

        if usize::from(message.num_signers) > account_keys.len() {
            return Err(SmartAccountError::InvalidTransactionMessage);
        }
        if message.num_writable_signers > message.num_signers {
            return Err(SmartAccountError::InvalidTransactionMessage);
        }
        if usize::from(message.num_writable_non_signers)
            > account_keys
                .len()
                .saturating_sub(usize::from(message.num_signers))
        {
            return Err(SmartAccountError::InvalidTransactionMessage);
        }

        for instruction in &instructions {
            if usize::from(instruction.program_id_index) >= num_all_account_keys {
                return Err(SmartAccountError::InvalidTransactionMessage);
            }
            for account_index in &instruction.account_indexes {
                if usize::from(*account_index) >= num_all_account_keys {
                    return Err(SmartAccountError::InvalidTransactionMessage);
                }
            }
        }

        Ok(Self {
            num_signers: message.num_signers,
            num_writable_signers: message.num_writable_signers,
            num_writable_non_signers: message.num_writable_non_signers,
            account_keys,
            instructions,
            address_table_lookups: address_table_lookups
                .into_iter()
                .map(SmartAccountMessageAddressTableLookup::from)
                .collect(),
        })
    }
}

#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
#[derive(Clone, Eq, PartialEq)]
pub struct SmartAccountCompiledInstruction {
    pub program_id_index: u8,
    pub account_indexes: Vec<u8>,
    pub data: Vec<u8>,
}

impl From<CompiledInstruction> for SmartAccountCompiledInstruction {
    fn from(ci: CompiledInstruction) -> Self {
        Self {
            program_id_index: ci.program_id_index,
            account_indexes: ci.account_indexes.into(),
            data: ci.data.into(),
        }
    }
}

#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
#[derive(Clone, Eq, PartialEq)]
pub struct SmartAccountMessageAddressTableLookup {
    pub account_key: Pubkey,
    pub writable_indexes: Vec<u8>,
    pub readonly_indexes: Vec<u8>,
}

impl From<MessageAddressTableLookup> for SmartAccountMessageAddressTableLookup {
    fn from(m: MessageAddressTableLookup) -> Self {
        Self {
            account_key: m.account_key,
            writable_indexes: m.writable_indexes.into(),
            readonly_indexes: m.readonly_indexes.into(),
        }
    }
}
