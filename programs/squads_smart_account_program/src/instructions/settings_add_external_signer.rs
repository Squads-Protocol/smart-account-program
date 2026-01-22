use anchor_lang::prelude::*;

use crate::{
    errors::*,
    program::SquadsSmartAccountProgram,
    state::*,
};

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct AddExternalSignerArgs {
    /// The external signer type (1=P256Webauthn, 2=Secp256k1, 3=Ed25519External)
    pub signer_type: u8,
    /// Key ID (derived from signer type + public key)
    pub key_id: Pubkey,
    /// Permissions for the signer
    pub permissions: Permissions,
    /// Signer-specific data (73 bytes for P256, 85 for secp256k1, 32 for Ed25519)
    pub signer_data: Vec<u8>,
    /// Optional memo for indexing
    pub memo: Option<String>,
}

#[derive(Accounts)]
pub struct SettingsAddExternalSigner<'info> {
    #[account(
        mut,
        seeds = [SEED_PREFIX, SEED_SETTINGS, settings.seed.to_le_bytes().as_ref()],
        bump = settings.bump,
    )]
    pub settings: Account<'info, Settings>,

    /// Payer for any required account reallocation
    #[account(mut)]
    pub payer: Signer<'info>,

    /// System program for reallocation
    pub system_program: Program<'info, System>,

    #[allow(unused)]
    pub program: Program<'info, SquadsSmartAccountProgram>,

    // remaining_accounts:
    // - FIRST: instructions sysvar (sysvar::instructions::ID) - required for external sig verification
    // - Rest: native signer accounts for consensus
}

impl<'info> SettingsAddExternalSigner<'info> {
    /// Parse and validate external signer data from args
    #[allow(dead_code)]
    fn parse_signer_v2(args: &AddExternalSignerArgs) -> Result<SmartAccountSignerV2> {
        let signer_type = match args.signer_type {
            0 => return Err(SmartAccountError::UseAddSignerForNative.into()),
            1 => SignerTypeV2::P256Webauthn,
            2 => SignerTypeV2::Secp256k1,
            3 => SignerTypeV2::Ed25519External,
            _ => return Err(SmartAccountError::InvalidSignerType.into()),
        };

        match signer_type {
            SignerTypeV2::P256Webauthn => {
                if args.signer_data.len() != 73 {
                    return Err(SmartAccountError::InvalidPayload.into());
                }
                let mut compressed_pubkey = [0u8; 33];
                compressed_pubkey.copy_from_slice(&args.signer_data[0..33]);
                let mut rp_id_hash = [0u8; 32];
                rp_id_hash.copy_from_slice(&args.signer_data[33..65]);
                let counter = u64::from_le_bytes(
                    args.signer_data[65..73]
                        .try_into()
                        .map_err(|_| SmartAccountError::InvalidPayload)?,
                );

                Ok(SmartAccountSignerV2::P256Webauthn {
                    key_id: args.key_id,
                    permissions: args.permissions,
                    data: P256WebauthnDataV2 {
                        compressed_pubkey,
                        rp_id_hash,
                        counter,
                    },
                })
            }
            SignerTypeV2::Secp256k1 => {
                if args.signer_data.len() != 85 {
                    return Err(SmartAccountError::InvalidPayload.into());
                }
                let mut uncompressed_pubkey = [0u8; 64];
                uncompressed_pubkey.copy_from_slice(&args.signer_data[0..64]);
                let mut eth_address = [0u8; 20];
                eth_address.copy_from_slice(&args.signer_data[64..84]);
                let has_eth_address = args.signer_data[84] != 0;

                Ok(SmartAccountSignerV2::Secp256k1 {
                    key_id: args.key_id,
                    permissions: args.permissions,
                    data: Secp256k1DataV2 {
                        uncompressed_pubkey,
                        eth_address,
                        has_eth_address,
                    },
                })
            }
            SignerTypeV2::Ed25519External => {
                if args.signer_data.len() != 32 {
                    return Err(SmartAccountError::InvalidPayload.into());
                }
                let mut external_pubkey = [0u8; 32];
                external_pubkey.copy_from_slice(&args.signer_data[0..32]);

                Ok(SmartAccountSignerV2::Ed25519External {
                    key_id: args.key_id,
                    permissions: args.permissions,
                    data: Ed25519ExternalDataV2 { external_pubkey },
                })
            }
            SignerTypeV2::Native => Err(SmartAccountError::UseAddSignerForNative.into()),
        }
    }

    /// Add an external signer to the smart account
    /// 
    /// NOTE: This instruction is currently not implemented as it requires migrating
    /// the Settings account to use SmartAccountSignerWrapper instead of Vec<SmartAccountSigner>.
    /// 
    /// The scaffolding is in place for:
    /// - Parsing external signer data (P256/WebAuthn, secp256k1, Ed25519 external)
    /// - Deriving deterministic key_ids
    /// - Custom packed serialization format
    /// - Precompile introspection for signature verification
    pub fn settings_add_external_signer(
        _ctx: Context<SettingsAddExternalSigner<'info>>,
        _args: AddExternalSignerArgs,
    ) -> Result<()> {
        // Not yet implemented - requires Settings migration to SmartAccountSignerWrapper
        Err(SmartAccountError::NotImplemented.into())
    }
}

#[event]
pub struct ExternalSignerAddedEvent {
    pub settings: Pubkey,
    pub key_id: Pubkey,
    pub signer_type: u8,
}
