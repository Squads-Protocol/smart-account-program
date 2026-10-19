//! Bitflag API for the codama-generated `Permissions { mask: u8 }` struct.
//!
//! Codama's `codama/transforms/filter-types.ts` drops the source-of-truth
//! `Permission` enum because the IDL representation collapses it to a `u8`
//! alias. This module re-introduces the enum and an extension trait so
//! callers can write `permissions.has(Permission::Vote)` instead of doing
//! bitmask arithmetic by hand.

use crate::generated::types::Permissions;

/// Individual permissions a signer can hold on a smart account.
///
/// The discriminant values are the bit positions used in [`Permissions::mask`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Permission {
    Initiate = 1 << 0,
    Vote = 1 << 1,
    Execute = 1 << 2,
}

impl Permission {
    pub const fn all() -> [Permission; 3] {
        [Permission::Initiate, Permission::Vote, Permission::Execute]
    }
}

/// Bitflag API extension for [`Permissions`]. Mirrors the hand-written SDK.
pub trait PermissionsExt {
    /// Bytes consumed by a `Permissions` field on-chain (matches anchor `InitSpace`).
    const INIT_SPACE: usize = 1;

    fn from_vec(permissions: &[Permission]) -> Self;
    fn has(&self, permission: Permission) -> bool;
    fn all() -> Self;
}

impl PermissionsExt for Permissions {
    fn from_vec(permissions: &[Permission]) -> Self {
        let mut mask = 0u8;
        for p in permissions {
            mask |= *p as u8;
        }
        Self { mask }
    }

    fn has(&self, permission: Permission) -> bool {
        self.mask & (permission as u8) != 0
    }

    fn all() -> Self {
        Self { mask: 0b111 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_vec_round_trip() {
        let p = Permissions::from_vec(&[Permission::Initiate, Permission::Execute]);
        assert!(p.has(Permission::Initiate));
        assert!(!p.has(Permission::Vote));
        assert!(p.has(Permission::Execute));
    }

    #[test]
    fn all_sets_every_bit() {
        let p = <Permissions as PermissionsExt>::all();
        for variant in Permission::all() {
            assert!(p.has(variant), "missing {:?}", variant);
        }
    }
}
