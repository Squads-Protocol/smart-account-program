//! Anchor-trait impls for the account types defined in this crate.
//!
//! Compiled only when the `anchor` feature is enabled, which is turned on by
//! the program crate and off for every other consumer. Providing these impls
//! from the types crate (rather than from the program crate via newtypes) keeps
//! all program callers free of anchor-specific wrapping while still satisfying
//! Rust's orphan rule — the trait is foreign, and the type is local to this
//! crate.

use anchor_lang::prelude::*;
use borsh::{BorshDeserialize, BorshSerialize};
use std::io::Write;

use crate::{
    Batch, BatchTransaction, LegacyTransaction, Policy, ProgramConfig, Proposal, Settings,
    SettingsTransaction, SpendingLimit, Transaction, TransactionBuffer, PROGRAM_ID,
};

macro_rules! impl_anchor_account {
    ($ty:ty) => {
        impl anchor_lang::Discriminator for $ty {
            const DISCRIMINATOR: [u8; 8] = <$ty>::DISCRIMINATOR;
        }

        impl anchor_lang::Owner for $ty {
            fn owner() -> Pubkey {
                PROGRAM_ID
            }
        }

        impl anchor_lang::AccountSerialize for $ty {
            fn try_serialize<W: Write>(&self, writer: &mut W) -> anchor_lang::Result<()> {
                writer
                    .write_all(&<Self as anchor_lang::Discriminator>::DISCRIMINATOR)
                    .map_err(|_| {
                        anchor_lang::error::Error::from(
                            anchor_lang::error::ErrorCode::AccountDidNotSerialize,
                        )
                    })?;
                BorshSerialize::serialize(self, writer).map_err(|_| {
                    anchor_lang::error::Error::from(
                        anchor_lang::error::ErrorCode::AccountDidNotSerialize,
                    )
                })?;
                Ok(())
            }
        }

        impl anchor_lang::AccountDeserialize for $ty {
            fn try_deserialize(buf: &mut &[u8]) -> anchor_lang::Result<Self> {
                if buf.len() < 8 {
                    return Err(anchor_lang::error::ErrorCode::AccountDiscriminatorNotFound.into());
                }
                let given: [u8; 8] = buf[..8].try_into().unwrap();
                if given != <Self as anchor_lang::Discriminator>::DISCRIMINATOR {
                    return Err(anchor_lang::error::ErrorCode::AccountDiscriminatorMismatch.into());
                }
                Self::try_deserialize_unchecked(buf)
            }

            fn try_deserialize_unchecked(buf: &mut &[u8]) -> anchor_lang::Result<Self> {
                let mut data = &buf[8..];
                BorshDeserialize::deserialize(&mut data).map_err(|_| {
                    anchor_lang::error::Error::from(
                        anchor_lang::error::ErrorCode::AccountDidNotDeserialize,
                    )
                })
            }
        }
    };
}

impl_anchor_account!(Settings);
impl_anchor_account!(Proposal);
impl_anchor_account!(Policy);
impl_anchor_account!(Transaction);
impl_anchor_account!(LegacyTransaction);
impl_anchor_account!(Batch);
impl_anchor_account!(BatchTransaction);
impl_anchor_account!(SettingsTransaction);
impl_anchor_account!(SpendingLimit);
impl_anchor_account!(TransactionBuffer);
impl_anchor_account!(ProgramConfig);

// Convenience From impl so callers can use `?` to propagate the types-crate
// error through anchor-flavored `Result<T>`. Orphan rule: the trait is
// foreign but `SmartAccountError` is local to this crate, so it's allowed.
impl From<crate::SmartAccountError> for anchor_lang::error::Error {
    fn from(err: crate::SmartAccountError) -> Self {
        anchor_lang::error::Error::AnchorError(Box::new(anchor_lang::error::AnchorError {
            error_name: format!("{:?}", err),
            error_code_number: err as u32,
            error_msg: format!("{}", err),
            error_origin: None,
            compared_values: None,
        }))
    }
}
