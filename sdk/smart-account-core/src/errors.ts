export type SmartAccountErrorCode =
  | 'ACCOUNT_NOT_FOUND'
  | 'NOT_A_SMART_ACCOUNT'
  | 'INSUFFICIENT_SIGNERS'
  | 'SYNC_NOT_ALLOWED'
  | 'INVALID_SIGNER_SET'
  | 'SIMULATION_FAILED'
  | 'UNSUPPORTED_TRANSACTION'
  | 'TX_TOO_LARGE'
  | 'INVALID_ARGUMENT'
  | 'SIGNER_MODIFIED_TRANSACTION';

/**
 * Every error this package raises itself. `code` is stable; `message` is for humans. Errors from `@solana/kit` (signing,
 * address parsing, RPC transport) pass through unchanged.
 */
export class SmartAccountError extends Error {
  readonly code: SmartAccountErrorCode;
  readonly details: unknown;

  constructor(code: SmartAccountErrorCode, message: string, details?: unknown) {
    super(`${code}: ${message}`);
    this.name = 'SmartAccountError';
    this.code = code;
    this.details = details;
  }
}
