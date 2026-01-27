import * as smartAccount from "@sqds/smart-account";
import { Connection, Keypair, PublicKey } from "@solana/web3.js";
import { createHash } from "crypto";

const { Permissions } = smartAccount.types;

type V2SignerResult = {
  signer: smartAccount.generated.SmartAccountSigner;
  keyId: PublicKey;
};

const toGeneratedPermissions = (
  permissions: smartAccount.types.Permissions
): smartAccount.generated.Permissions => ({
  mask: permissions.mask,
});

const deriveSignerKeyId = (
  signerType: smartAccount.generated.SignerType,
  canonicalKey: Uint8Array
) => {
  const data = Buffer.concat([
    Buffer.from([signerType]),
    Buffer.from(canonicalKey),
  ]);
  const hash = createHash("sha256").update(data).digest();
  return new PublicKey(hash);
};

const getFutureTimestamp = async (
  connection: Connection,
  offsetSeconds = 3600
) => {
  const slot = await connection.getSlot("processed");
  const blockTime = await connection.getBlockTime(slot);
  const now = blockTime ?? Math.floor(Date.now() / 1000);
  return BigInt(now + offsetSeconds) as unknown as
    smartAccount.generated.SessionKeyData["expiration"];
};

const buildSessionKeyData = async (
  connection: Connection,
  sessionKey: Keypair
): Promise<smartAccount.generated.SessionKeyData> => ({
  key: sessionKey.publicKey,
  expiration: await getFutureTimestamp(connection),
});

export const buildNativeSigner = (
  key: PublicKey,
  permissions = Permissions.all()
): V2SignerResult => ({
  signer: {
    __kind: "Native",
    key,
    permissions: toGeneratedPermissions(permissions),
  },
  keyId: key,
});

export const buildP256WebauthnSigner = async ({
  connection,
  sessionKey,
  permissions = Permissions.all(),
  rpId = "example.com",
}: {
  connection: Connection;
  sessionKey: Keypair;
  permissions?: smartAccount.types.Permissions;
  rpId?: string;
}): Promise<V2SignerResult> => {
  const compressedPubkey = new Uint8Array(33).fill(1);
  compressedPubkey[0] = 2;
  const rpIdBytes = Buffer.from(rpId);
  const rpIdLen = Math.min(rpIdBytes.length, 32);
  const rpIdPadded = new Uint8Array(32);
  rpIdPadded.set(rpIdBytes.subarray(0, rpIdLen));
  const rpIdHash = createHash("sha256")
    .update(rpIdBytes.subarray(0, rpIdLen))
    .digest();
  const sessionKeyData = await buildSessionKeyData(connection, sessionKey);
  const keyId = deriveSignerKeyId(
    smartAccount.generated.SignerType.P256Webauthn,
    compressedPubkey
  );

  return {
    signer: {
      __kind: "P256Webauthn",
      keyId,
      permissions: toGeneratedPermissions(permissions),
      data: {
        compressedPubkey: Array.from(compressedPubkey),
        rpIdLen,
        rpId: Array.from(rpIdPadded),
        rpIdHash: Array.from(rpIdHash),
        counter: 0n as unknown as smartAccount.generated.P256WebauthnData["counter"],
        sessionKeyData,
      },
    },
    keyId,
  };
};

export const buildSecp256k1Signer = async ({
  connection,
  sessionKey,
  permissions = Permissions.all(),
}: {
  connection: Connection;
  sessionKey: Keypair;
  permissions?: smartAccount.types.Permissions;
}): Promise<V2SignerResult> => {
  const uncompressedPubkey = new Uint8Array(64).fill(3);
  const ethAddress = new Uint8Array(20).fill(4);
  const sessionKeyData = await buildSessionKeyData(connection, sessionKey);
  const keyId = deriveSignerKeyId(
    smartAccount.generated.SignerType.Secp256k1,
    uncompressedPubkey
  );

  return {
    signer: {
      __kind: "Secp256k1",
      keyId,
      permissions: toGeneratedPermissions(permissions),
      data: {
        uncompressedPubkey: Array.from(uncompressedPubkey),
        ethAddress: Array.from(ethAddress),
        hasEthAddress: true,
        sessionKeyData,
      },
    },
    keyId,
  };
};

export const buildEd25519ExternalSigner = async ({
  connection,
  sessionKey,
  permissions = Permissions.all(),
}: {
  connection: Connection;
  sessionKey: Keypair;
  permissions?: smartAccount.types.Permissions;
}): Promise<V2SignerResult> => {
  const externalPubkey = new Uint8Array(32).fill(5);
  const sessionKeyData = await buildSessionKeyData(connection, sessionKey);
  const keyId = deriveSignerKeyId(
    smartAccount.generated.SignerType.Ed25519External,
    externalPubkey
  );

  return {
    signer: {
      __kind: "Ed25519External",
      keyId,
      permissions: toGeneratedPermissions(permissions),
      data: {
        externalPubkey: Array.from(externalPubkey),
        sessionKeyData,
      },
    },
    keyId,
  };
};

export const createSmartAccountV2WithSigners = async ({
  connection,
  programId,
  creator,
  signers,
  timeLock = 0,
  threshold = 1,
  rentCollector = null,
}: {
  connection: Connection;
  programId: PublicKey;
  creator: Keypair;
  signers: smartAccount.generated.SmartAccountSigner[];
  timeLock?: number;
  threshold?: number;
  rentCollector?: PublicKey | null;
}) => {
  const programConfig =
    await smartAccount.accounts.ProgramConfig.fromAccountAddress(
      connection,
      smartAccount.getProgramConfigPda({ programId })[0]
    );
  const accountIndex = BigInt(programConfig.smartAccountIndex.toString()) + 1n;
  const [settingsPda] = smartAccount.getSettingsPda({
    accountIndex,
    programId,
  });
  const signature = await smartAccount.rpc.createSmartAccountV2({
    connection,
    treasury: programConfig.treasury,
    creator,
    settings: settingsPda,
    settingsAuthority: null,
    threshold,
    signers,
    timeLock,
    rentCollector,
    sendOptions: { skipPreflight: true },
    programId,
  });
  const confirmation = await connection.confirmTransaction(signature);
  if (confirmation.value.err) {
    const tx = await connection.getTransaction(signature, {
      commitment: "confirmed",
      maxSupportedTransactionVersion: 0,
    });
    const logs = tx?.meta?.logMessages ?? [];
    throw new Error(
      `createSmartAccountV2 failed: ${JSON.stringify(
        confirmation.value.err
      )}\n${logs.join("\n")}`
    );
  }

  return settingsPda;
};
