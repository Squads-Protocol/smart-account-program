//! Wire-format `TransactionMessage` + a builder that turns a list of
//! `solana_instruction::Instruction`s into one ready to feed to
//! `create_transaction` / `add_transaction_to_batch`.
//!
//! **Why this exists**: codama renders `TransactionMessage::account_keys` as
//! the named-but-empty struct `SmallVecU8Pubkey {}`, which is a renderer
//! limitation — `codama/transforms/small-vec.ts` rewrites the type to a
//! length-prefixed array node, but `@codama/renderers-rust` doesn't carry
//! that representation through to the emitted code. We supply a corrected
//! shape here so callers can actually construct on-wire-compatible bytes.
//!
//! The struct field layout mirrors the program's `TransactionMessage` exactly
//! (u8-prefixed `account_keys`, `instructions`, `address_table_lookups`, plus
//! u8-prefixed `account_indexes` and u16-prefixed `data` inside each
//! `CompiledInstruction`).

use std::collections::BTreeMap;
use std::io;
use std::marker::PhantomData;

use borsh::{BorshDeserialize, BorshSerialize};
use solana_address::Address;
use solana_instruction::Instruction;

use crate::generated::types::{
    SmartAccountCompiledInstruction, SmartAccountMessageAddressTableLookup,
    SmartAccountTransactionMessage,
};

/// Borsh-encodes as `[L-prefix bytes ...] ++ [T*]`. The codama-generated
/// `SmallVecU*` marker structs are empty and serialize/deserialize as zero
/// bytes, so we replace them in our local `TransactionMessage` and
/// `CompiledInstruction` types.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SmallVec<L, T>(Vec<T>, PhantomData<L>);

impl<L, T> SmallVec<L, T> {
    pub fn len(&self) -> usize {
        self.0.len()
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    pub fn into_inner(self) -> Vec<T> {
        self.0
    }
    pub fn as_slice(&self) -> &[T] {
        &self.0
    }
}

impl<L, T> From<Vec<T>> for SmallVec<L, T> {
    fn from(v: Vec<T>) -> Self {
        Self(v, PhantomData)
    }
}

impl<L, T> From<SmallVec<L, T>> for Vec<T> {
    fn from(v: SmallVec<L, T>) -> Self {
        v.0
    }
}

impl<T: BorshSerialize> BorshSerialize for SmallVec<u8, T> {
    fn serialize<W: io::Write>(&self, writer: &mut W) -> io::Result<()> {
        let len = u8::try_from(self.0.len())
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "length exceeds u8::MAX"))?;
        writer.write_all(&len.to_le_bytes())?;
        for item in &self.0 {
            item.serialize(writer)?;
        }
        Ok(())
    }
}

impl<T: BorshSerialize> BorshSerialize for SmallVec<u16, T> {
    fn serialize<W: io::Write>(&self, writer: &mut W) -> io::Result<()> {
        let len = u16::try_from(self.0.len())
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "length exceeds u16::MAX"))?;
        writer.write_all(&len.to_le_bytes())?;
        for item in &self.0 {
            item.serialize(writer)?;
        }
        Ok(())
    }
}

impl<T: BorshDeserialize> BorshDeserialize for SmallVec<u8, T> {
    fn deserialize_reader<R: io::Read>(reader: &mut R) -> io::Result<Self> {
        let len = u8::deserialize_reader(reader)?;
        let mut out = Vec::with_capacity(usize::from(len));
        for _ in 0..len {
            out.push(T::deserialize_reader(reader)?);
        }
        Ok(Self(out, PhantomData))
    }
}

impl<T: BorshDeserialize> BorshDeserialize for SmallVec<u16, T> {
    fn deserialize_reader<R: io::Read>(reader: &mut R) -> io::Result<Self> {
        let len = u16::deserialize_reader(reader)?;
        let mut out = Vec::with_capacity(usize::from(len));
        for _ in 0..len {
            out.push(T::deserialize_reader(reader)?);
        }
        Ok(Self(out, PhantomData))
    }
}

/// Wire-format compiled instruction. Matches the program's expected layout
/// (u8-prefixed account_indexes, u16-prefixed data).
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, Eq, PartialEq)]
pub struct CompiledInstruction {
    pub program_id_index: u8,
    pub account_indexes: SmallVec<u8, u8>,
    pub data: SmallVec<u16, u8>,
}

/// Wire-format address-table lookup. Matches the program's expected layout.
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, Eq, PartialEq)]
pub struct MessageAddressTableLookup {
    pub account_key: Address,
    pub writable_indexes: SmallVec<u8, u8>,
    pub readonly_indexes: SmallVec<u8, u8>,
}

/// Wire-format transaction message — the payload of
/// `CreateTransactionArgs::transaction_message`.
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, Eq, PartialEq)]
pub struct TransactionMessage {
    pub num_signers: u8,
    pub num_writable_signers: u8,
    pub num_writable_non_signers: u8,
    pub account_keys: SmallVec<u8, Address>,
    pub instructions: SmallVec<u8, CompiledInstruction>,
    pub address_table_lookups: SmallVec<u8, MessageAddressTableLookup>,
}

/// Errors surfaced by [`TransactionMessageBuilder::build`] and
/// [`SmartAccountTransactionMessage::try_from_wire`].
#[derive(Debug, thiserror::Error, Eq, PartialEq)]
pub enum TransactionMessageError {
    #[error("num_signers > account_keys.len()")]
    SignersExceedKeys,
    #[error("num_writable_signers > num_signers")]
    WritableSignersExceedSigners,
    #[error("num_writable_non_signers > non-signer key count")]
    WritableNonSignersExceedNonSigners,
    #[error("instruction account index {index} out of bounds (have {len})")]
    InstructionIndexOutOfBounds { index: u8, len: usize },
    #[error("too many account keys: {count} (max {})", u8::MAX)]
    TooManyAccountKeys { count: usize },
    #[error("too many instructions: {count} (max {})", u8::MAX)]
    TooManyInstructions { count: usize },
    #[error("instruction data exceeds u16::MAX bytes: {len}")]
    InstructionDataTooLarge { len: usize },
}

/// Builder that turns a list of `solana_instruction::Instruction`s into a
/// validated [`TransactionMessage`] ready for borsh encoding and submission
/// via the codama-generated `create_transaction` / `add_transaction_to_batch`
/// instructions.
///
/// Account keys are sorted in the order: writable-signers,
/// readonly-signers, writable-non-signers, readonly-non-signers.
#[derive(Default, Debug, Clone)]
pub struct TransactionMessageBuilder {
    instructions: Vec<Instruction>,
    address_table_lookups: Vec<MessageAddressTableLookup>,
}

impl TransactionMessageBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_instruction(mut self, ix: Instruction) -> Self {
        self.instructions.push(ix);
        self
    }

    pub fn add_instructions<I: IntoIterator<Item = Instruction>>(mut self, ixs: I) -> Self {
        self.instructions.extend(ixs);
        self
    }

    pub fn add_address_table_lookup(mut self, lookup: MessageAddressTableLookup) -> Self {
        self.address_table_lookups.push(lookup);
        self
    }

    /// Build the wire-format [`TransactionMessage`].
    ///
    /// Signer status is inferred from each instruction's `AccountMeta::is_signer`
    /// flag; writable status is the union of every occurrence of the key.
    pub fn build(self) -> Result<TransactionMessage, TransactionMessageError> {
        // Collect unique account keys with their union of writable/signer flags.
        // We use a BTreeMap keyed by insertion order via an explicit counter so
        // the resulting message is deterministic.
        #[derive(Default)]
        struct KeyState {
            order: usize,
            is_signer: bool,
            is_writable: bool,
        }
        let mut keys: BTreeMap<Address, KeyState> = BTreeMap::new();
        let mut next_order = 0usize;

        // Each ix contributes its program_id (readonly non-signer) and its accounts.
        let upsert = |keys: &mut BTreeMap<Address, KeyState>,
                      next_order: &mut usize,
                      addr: Address,
                      is_signer: bool,
                      is_writable: bool| {
            let entry = keys.entry(addr).or_insert_with(|| {
                let s = KeyState {
                    order: *next_order,
                    ..Default::default()
                };
                *next_order += 1;
                s
            });
            entry.is_signer |= is_signer;
            entry.is_writable |= is_writable;
        };

        for ix in &self.instructions {
            upsert(&mut keys, &mut next_order, ix.program_id, false, false);
            for meta in &ix.accounts {
                upsert(
                    &mut keys,
                    &mut next_order,
                    meta.pubkey,
                    meta.is_signer,
                    meta.is_writable,
                );
            }
        }

        if keys.len() > usize::from(u8::MAX) {
            return Err(TransactionMessageError::TooManyAccountKeys { count: keys.len() });
        }

        // Sort into the canonical bucket order: ws, rs, wns, rns
        let mut ordered: Vec<(Address, KeyState)> = keys.into_iter().collect();
        ordered.sort_by_key(|(_, s)| {
            let bucket = match (s.is_signer, s.is_writable) {
                (true, true) => 0,
                (true, false) => 1,
                (false, true) => 2,
                (false, false) => 3,
            };
            (bucket, s.order)
        });

        let mut num_signers = 0u8;
        let mut num_writable_signers = 0u8;
        let mut num_writable_non_signers = 0u8;
        let mut account_keys: Vec<Address> = Vec::with_capacity(ordered.len());
        let mut key_index: BTreeMap<Address, u8> = BTreeMap::new();
        for (addr, state) in ordered {
            let idx = u8::try_from(account_keys.len()).expect("bounded above");
            key_index.insert(addr, idx);
            account_keys.push(addr);
            if state.is_signer {
                num_signers = num_signers.saturating_add(1);
                if state.is_writable {
                    num_writable_signers = num_writable_signers.saturating_add(1);
                }
            } else if state.is_writable {
                num_writable_non_signers = num_writable_non_signers.saturating_add(1);
            }
        }

        if self.instructions.len() > usize::from(u8::MAX) {
            return Err(TransactionMessageError::TooManyInstructions {
                count: self.instructions.len(),
            });
        }

        let mut compiled: Vec<CompiledInstruction> = Vec::with_capacity(self.instructions.len());
        for ix in &self.instructions {
            if ix.data.len() > usize::from(u16::MAX) {
                return Err(TransactionMessageError::InstructionDataTooLarge {
                    len: ix.data.len(),
                });
            }
            let program_id_index = *key_index.get(&ix.program_id).ok_or(
                TransactionMessageError::InstructionIndexOutOfBounds {
                    index: 0,
                    len: account_keys.len(),
                },
            )?;
            let account_indexes: Vec<u8> = ix
                .accounts
                .iter()
                .map(|m| {
                    key_index.get(&m.pubkey).copied().ok_or(
                        TransactionMessageError::InstructionIndexOutOfBounds {
                            index: 0,
                            len: account_keys.len(),
                        },
                    )
                })
                .collect::<Result<_, _>>()?;
            compiled.push(CompiledInstruction {
                program_id_index,
                account_indexes: account_indexes.into(),
                data: ix.data.clone().into(),
            });
        }

        Ok(TransactionMessage {
            num_signers,
            num_writable_signers,
            num_writable_non_signers,
            account_keys: account_keys.into(),
            instructions: compiled.into(),
            address_table_lookups: self.address_table_lookups.into(),
        })
    }
}

/// Extension trait that adds validating-conversion and computed helpers to the
/// codama-generated [`SmartAccountTransactionMessage`].
pub trait SmartAccountTransactionMessageExt: Sized {
    /// Validate a wire-format [`TransactionMessage`] and convert it to a
    /// [`SmartAccountTransactionMessage`]. Performs the same checks the
    /// on-chain program would (signer/writable index ranges).
    fn try_from_wire(msg: TransactionMessage) -> Result<Self, TransactionMessageError>;

    /// Total number of account keys available to instructions, including
    /// those loaded via address-lookup tables.
    fn num_all_account_keys(&self) -> usize;
    fn is_static_writable_index(&self, key_index: usize) -> bool;
    fn is_signer_index(&self, key_index: usize) -> bool;
}

impl SmartAccountTransactionMessageExt for SmartAccountTransactionMessage {
    fn try_from_wire(msg: TransactionMessage) -> Result<Self, TransactionMessageError> {
        let account_keys: Vec<Address> = msg.account_keys.into();
        let instructions: Vec<CompiledInstruction> = msg.instructions.into();
        let lookups: Vec<MessageAddressTableLookup> = msg.address_table_lookups.into();

        if usize::from(msg.num_signers) > account_keys.len() {
            return Err(TransactionMessageError::SignersExceedKeys);
        }
        if msg.num_writable_signers > msg.num_signers {
            return Err(TransactionMessageError::WritableSignersExceedSigners);
        }
        let non_signer_count = account_keys
            .len()
            .saturating_sub(usize::from(msg.num_signers));
        if usize::from(msg.num_writable_non_signers) > non_signer_count {
            return Err(TransactionMessageError::WritableNonSignersExceedNonSigners);
        }

        let num_all_keys = account_keys.len()
            + lookups
                .iter()
                .map(|l| l.writable_indexes.len() + l.readonly_indexes.len())
                .sum::<usize>();

        let compiled: Vec<SmartAccountCompiledInstruction> = instructions
            .into_iter()
            .map(|ci| {
                let program_id_index = ci.program_id_index;
                if usize::from(program_id_index) >= num_all_keys {
                    return Err(TransactionMessageError::InstructionIndexOutOfBounds {
                        index: program_id_index,
                        len: num_all_keys,
                    });
                }
                let account_indexes: Vec<u8> = ci.account_indexes.into();
                for &idx in &account_indexes {
                    if usize::from(idx) >= num_all_keys {
                        return Err(TransactionMessageError::InstructionIndexOutOfBounds {
                            index: idx,
                            len: num_all_keys,
                        });
                    }
                }
                let data: Vec<u8> = ci.data.into();
                Ok(SmartAccountCompiledInstruction {
                    program_id_index,
                    account_indexes,
                    data,
                })
            })
            .collect::<Result<_, _>>()?;

        let address_table_lookups: Vec<SmartAccountMessageAddressTableLookup> = lookups
            .into_iter()
            .map(|l| SmartAccountMessageAddressTableLookup {
                account_key: l.account_key,
                writable_indexes: l.writable_indexes.into(),
                readonly_indexes: l.readonly_indexes.into(),
            })
            .collect();

        Ok(SmartAccountTransactionMessage {
            num_signers: msg.num_signers,
            num_writable_signers: msg.num_writable_signers,
            num_writable_non_signers: msg.num_writable_non_signers,
            account_keys,
            instructions: compiled,
            address_table_lookups,
        })
    }

    fn num_all_account_keys(&self) -> usize {
        let lookup_count: usize = self
            .address_table_lookups
            .iter()
            .map(|l| l.writable_indexes.len() + l.readonly_indexes.len())
            .sum();
        self.account_keys.len() + lookup_count
    }

    fn is_static_writable_index(&self, key_index: usize) -> bool {
        let num_keys = self.account_keys.len();
        let signers = usize::from(self.num_signers);
        let writable_signers = usize::from(self.num_writable_signers);
        let writable_non_signers = usize::from(self.num_writable_non_signers);

        if key_index >= num_keys {
            return false;
        }
        if key_index < writable_signers {
            return true;
        }
        if key_index >= signers {
            let into_non_signers = key_index.saturating_sub(signers);
            return into_non_signers < writable_non_signers;
        }
        false
    }

    fn is_signer_index(&self, key_index: usize) -> bool {
        key_index < usize::from(self.num_signers)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy(addr: u8, signer: bool, writable: bool) -> solana_instruction::AccountMeta {
        solana_instruction::AccountMeta {
            pubkey: Address::new_from_array([addr; 32]),
            is_signer: signer,
            is_writable: writable,
        }
    }

    #[test]
    fn builder_classifies_signers_and_writability() {
        let prog = Address::new_from_array([0xAA; 32]);
        let ix = Instruction {
            program_id: prog,
            accounts: vec![
                dummy(1, true, true),
                dummy(2, true, false),
                dummy(3, false, true),
            ],
            data: vec![0xDE, 0xAD],
        };
        let msg = TransactionMessageBuilder::new()
            .add_instruction(ix)
            .build()
            .unwrap();

        assert_eq!(msg.num_signers, 2);
        assert_eq!(msg.num_writable_signers, 1);
        assert_eq!(msg.num_writable_non_signers, 1);
        assert_eq!(msg.account_keys.len(), 4); // 3 accounts + program_id
        assert_eq!(msg.instructions.len(), 1);
    }

    #[test]
    fn try_from_wire_round_trip() {
        let prog = Address::new_from_array([0xAA; 32]);
        let ix = Instruction {
            program_id: prog,
            accounts: vec![dummy(1, true, true)],
            data: vec![],
        };
        let wire = TransactionMessageBuilder::new()
            .add_instruction(ix)
            .build()
            .unwrap();
        let sm = SmartAccountTransactionMessage::try_from_wire(wire).unwrap();
        assert_eq!(sm.num_signers, 1);
        assert_eq!(sm.num_writable_signers, 1);
    }
}
