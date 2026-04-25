pub mod account_events;

pub use account_events::*;

#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
pub enum SmartAccountEvent {
    CreateSmartAccountEvent(CreateSmartAccountEvent),
    SynchronousTransactionEvent(SynchronousTransactionEvent),
    SynchronousSettingsTransactionEvent(SynchronousSettingsTransactionEvent),
    AddSpendingLimitEvent(AddSpendingLimitEvent),
    RemoveSpendingLimitEvent(RemoveSpendingLimitEvent),
    UseSpendingLimitEvent(UseSpendingLimitEvent),
    AuthoritySettingsEvent(AuthoritySettingsEvent),
    AuthorityChangeEvent(AuthorityChangeEvent),
    TransactionEvent(TransactionEvent),
    ProposalEvent(ProposalEvent),
    SynchronousTransactionEventV2(SynchronousTransactionEventV2),
    SettingsChangePolicyEvent(SettingsChangePolicyEvent),
    PolicyEvent(PolicyEvent),
}
