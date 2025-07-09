use anchor_lang::{prelude::*, AnchorDeserialize, AnchorSerialize};

pub mod spending_limit;

pub trait Policy {
    type SetupParams: AnchorSerialize + AnchorDeserialize;
    type UseParams: AnchorSerialize + AnchorDeserialize;
    fn invariant(&self) -> Result<()>;
    fn set_up(&self, params: Self::SetupParams) -> Result<()>;
    fn use_(&self, params: Self::UseParams) -> Result<()>;
}
