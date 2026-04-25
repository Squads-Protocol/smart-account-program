//! Conversion helpers between the types-crate's `SmartAccountError`
//! (thiserror, pure data) and this program's anchor-flavored errors.
//!
//! The orphan rule forbids implementing `From<types::SmartAccountError> for
//! anchor_lang::error::Error` since both are foreign. Instead we expose two
//! ergonomic tools:
//!   * `types_err_to_anchor(e)` — plain function.
//!   * `ToAnchorResult::to_anchor()` — `Result<T, types::SmartAccountError>`
//!     → `anchor_lang::Result<T>`. Call sites pattern: `x.to_anchor()?`.

use anchor_lang::error::{AnchorError, Error as AnchorLangError};

pub fn types_err_to_anchor(
    err: squads_smart_account_program_types::SmartAccountError,
) -> AnchorLangError {
    AnchorLangError::AnchorError(Box::new(AnchorError {
        error_name: format!("{:?}", err),
        error_code_number: err as u32,
        error_msg: format!("{}", err),
        error_origin: None,
        compared_values: None,
    }))
}

pub trait ToAnchorResult<T> {
    fn to_anchor(self) -> anchor_lang::Result<T>;
}

impl<T> ToAnchorResult<T> for Result<T, squads_smart_account_program_types::SmartAccountError> {
    fn to_anchor(self) -> anchor_lang::Result<T> {
        self.map_err(types_err_to_anchor)
    }
}
