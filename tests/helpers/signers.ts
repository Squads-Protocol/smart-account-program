import { PublicKey } from "@solana/web3.js";
import * as smartAccount from "@sqds/smart-account";

// Signer format control via environment variable
const SIGNER_FORMAT = process.env.SIGNER_FORMAT || 'v2';

/**
 * Creates a signer object in V1 or V2 format based on SIGNER_FORMAT env var
 * V1 format: { key, permissions }
 * V2 format: { __kind: "Native", key, permissions }
 */
export function createSignerObject(
  key: PublicKey,
  permissions: smartAccount.types.Permissions | { mask: number }
): any {
  if (SIGNER_FORMAT === 'v1') {
    return { key, permissions };
  } else {
    return { __kind: "Native" as const, key, permissions };
  }
}

/**
 * Creates unwrapped signer array for SettingsAction.AddSigner
 * (processed by customSmartAccountSignerWrapperBeet)
 */
export function createSignerArray(
  key: PublicKey,
  permissions: smartAccount.types.Permissions | { mask: number }
): smartAccount.generated.LegacySmartAccountSigner[] | smartAccount.generated.SmartAccountSigner[] {
  if (SIGNER_FORMAT === 'v1') {
    return [{ key, permissions }];
  } else {
    return [{ __kind: "Native" as const, key, permissions }];
  }
}

/**
 * Creates SmartAccountSignerWrapper for LimitedSettingsAction.AddSigner
 * (used in PolicyPayload.SettingsChange - expects wrapper format)
 */
export function createSignerWrapper(
  key: PublicKey,
  permissions: smartAccount.types.Permissions | { mask: number }
): smartAccount.generated.SmartAccountSignerWrapper {
  if (SIGNER_FORMAT === 'v1') {
    return {
      __kind: "V1" as const,
      fields: [[{ key, permissions }]],
    };
  } else {
    return {
      __kind: "V2" as const,
      fields: [[{ __kind: "Native" as const, key, permissions }]],
    };
  }
}

/**
 * Extracts the public key from a signer object regardless of V1/V2 format
 * Used when comparing on-chain data that could be in either format
 */
export function getSignerKey(
  signer: smartAccount.generated.LegacySmartAccountSigner | smartAccount.generated.SmartAccountSigner
): PublicKey {
  // V1 format (LegacySmartAccountSigner) always has a key property
  if ('key' in signer && !('__kind' in signer)) {
    return signer.key;
  }
  // V2 format - check the __kind to determine how to get the key
  if ('__kind' in signer) {
    if (signer.__kind === 'Native') {
      return signer.key;
    }
    // For external signers (P256Webauthn, Secp256k1, Ed25519External),
    // the key is derived from the data, not stored directly
    // For now, we'll throw an error as these aren't supported in parameterized tests
    throw new Error(`External signer type ${signer.__kind} not supported in test utils`);
  }
  throw new Error('Invalid signer format');
}

// Helper functions for SmartAccountSignerWrapper
export function unwrapSigners(
  wrapper:
    | smartAccount.generated.SmartAccountSignerWrapper
    | smartAccount.generated.LegacySmartAccountSigner[]
    | smartAccount.generated.SmartAccountSigner[]
): smartAccount.generated.SmartAccountSigner[] {
  // Check if it's already an unwrapped array
  if (Array.isArray(wrapper)) {
    // If it's already SmartAccountSigner[], return as-is
    if (wrapper.length === 0 || '__kind' in wrapper[0]) {
      return wrapper as smartAccount.generated.SmartAccountSigner[];
    }
    // If it's LegacySmartAccountSigner[], convert to SmartAccountSigner[]
    return wrapper.map((legacy: any) => ({
      __kind: "Native" as const,
      key: legacy.key,
      permissions: legacy.permissions,
    }));
  }
  // It's a wrapper type
  if (wrapper.__kind === "V2") {
    return wrapper.fields[0];
  } else {
    // V1 case - convert LegacySmartAccountSigner[] to SmartAccountSigner[]
    return wrapper.fields[0].map((legacy: any) => ({
      __kind: "Native" as const,
      key: legacy.key,
      permissions: legacy.permissions,
    }));
  }
}

export function wrapSigners(
  signers: smartAccount.generated.SmartAccountSigner[]
): smartAccount.generated.SmartAccountSignerWrapper {
  return {
    __kind: "V2" as const,
    fields: [signers],
  };
}
