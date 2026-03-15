import {
  Keypair,
  LAMPORTS_PER_SOL,
  PublicKey,
  SystemProgram,
  SYSVAR_INSTRUCTIONS_PUBKEY,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js";
import * as smartAccount from "@sqds/smart-account";
import assert from "assert";
import {
  buildSecp256r1MultiSigPrecompileInstruction,
  createLocalhostConnection,
  generateEd25519ExternalKeypair,
  generateFundedKeypair,
  generateP256Keypair,
  generateSecp256k1Keypair,
  getNextAccountIndex,
  getTestProgramId,
  getTestProgramTreasury,
  signEd25519External,
  signP256,
  signSecp256k1,
  base64urlEncode,
} from "../../../utils";
import {
  ExtraVerificationDataKind,
  serializeExtraVerificationDataVec,
} from "../../../helpers/extraVerificationData";
import { sha256 } from "@noble/hashes/sha256";
import { keccak_256 } from "@noble/hashes/sha3";

const { Settings } = smartAccount.accounts;
const { Permissions } = smartAccount.types;

const programId = getTestProgramId();
const connection = createLocalhostConnection();

function buildWebauthnSignature(
  rpIdHash: Uint8Array,
  hashedMessage: Uint8Array,
  rpId: string,
  counter: number,
  privateKey: Uint8Array
): { signature: Uint8Array; precompileMessage: Buffer } {
  const counterBuf = Buffer.alloc(4);
  counterBuf.writeUInt32BE(counter);
  const authenticatorData = Buffer.concat([
    Buffer.from(rpIdHash),
    Buffer.from([0x01]),
    counterBuf,
  ]);
  const challengeB64url = base64urlEncode(hashedMessage);
  const clientDataJSON = `{"type":"webauthn.get","challenge":"${challengeB64url}","origin":"https://${rpId}","crossOrigin":false}`;
  const clientDataHash = sha256(Buffer.from(clientDataJSON, "utf-8"));
  const precompileMessage = Buffer.concat([
    authenticatorData,
    Buffer.from(clientDataHash),
  ]);
  const signature = signP256(precompileMessage, privateKey);
  return { signature, precompileMessage };
}

// Offset compiled instruction account indices by +N to account for
// accounts prepended before txAccounts in remaining_accounts (e.g. sysvar).
// Format: [numIxs: u8] per ix: [programIdIndex: u8][numAccts: u8][acctIdx...][dataLen: u16 LE][data...]
function offsetCompiledInstructionIndices(
  raw: Uint8Array,
  offset: number
): Buffer {
  const buf = Buffer.from(raw);
  let cursor = 0;
  const numIxs = buf[cursor];
  cursor += 1;
  for (let i = 0; i < numIxs; i++) {
    buf[cursor] += offset;
    cursor += 1;
    const numAccts = buf[cursor];
    cursor += 1;
    for (let j = 0; j < numAccts; j++) {
      buf[cursor] += offset;
      cursor += 1;
    }
    const dataLen = buf[cursor] | (buf[cursor + 1] << 8);
    cursor += 2 + dataLen;
  }
  return buf;
}

describe("Instructions / mixed_signer_sync", () => {
  it("execute synchronous transfer with 1 native + 2 P256 precompile + 1 secp256k1 syscall + 1 ed25519 syscall", async () => {
    const creator = await generateFundedKeypair(connection);
    const nativeSigner1 = await generateFundedKeypair(connection);
    const p256Keypair1 = generateP256Keypair();
    const p256Keypair2 = generateP256Keypair();
    const secp256k1Keypair = generateSecp256k1Keypair();
    const ed25519Keypair = generateEd25519ExternalKeypair();
    const treasury = getTestProgramTreasury();

    const rpId = "example.com";
    const rpIdBytes = Buffer.from(rpId, "utf-8");
    const rpIdPadded = Buffer.alloc(32);
    rpIdBytes.copy(rpIdPadded);
    const rpIdHash = sha256(rpIdBytes);

    // Create smart account with 5 mixed signers, threshold 5, timeLock 0
    const accountIndex = await getNextAccountIndex(connection, programId);
    const [settingsPda] = smartAccount.getSettingsPda({
      accountIndex,
      programId,
    });

    const signers: smartAccount.generated.SmartAccountSigner[] = [
      {
        __kind: "Native",
        key: nativeSigner1.publicKey,
        permissions: { mask: Permissions.all().mask },
      },
      {
        __kind: "P256Webauthn",
        permissions: { mask: Permissions.all().mask },
        data: {
          compressedPubkey: Array.from(p256Keypair1.publicKeyCompressed),
          rpIdLen: rpIdBytes.length,
          rpId: Array.from(rpIdPadded),
          rpIdHash: Array.from(rpIdHash),
          counter: 0,
          sessionKeyData: { key: PublicKey.default, expiration: 0 },
        },
        nonce: 0,
      },
      {
        __kind: "P256Webauthn",
        permissions: { mask: Permissions.all().mask },
        data: {
          compressedPubkey: Array.from(p256Keypair2.publicKeyCompressed),
          rpIdLen: rpIdBytes.length,
          rpId: Array.from(rpIdPadded),
          rpIdHash: Array.from(rpIdHash),
          counter: 0,
          sessionKeyData: { key: PublicKey.default, expiration: 0 },
        },
        nonce: 0,
      },
      {
        __kind: "Secp256k1",
        permissions: { mask: Permissions.all().mask },
        data: {
          uncompressedPubkey: Array.from(
            secp256k1Keypair.publicKeyUncompressed
          ),
          ethAddress: Array.from(secp256k1Keypair.ethAddress),
          hasEthAddress: true,
          sessionKeyData: { key: PublicKey.default, expiration: 0 },
        },
        nonce: 0,
      },
      {
        __kind: "Ed25519External",
        permissions: { mask: Permissions.all().mask },
        data: {
          externalPubkey: Array.from(ed25519Keypair.publicKey),
          sessionKeyData: { key: PublicKey.default, expiration: 0 },
        },
        nonce: 0,
      },
    ];

    await smartAccount.rpc.createSmartAccount({
      connection,
      treasury,
      creator,
      settings: settingsPda,
      settingsAuthority: null,
      threshold: 5,
      signers,
      timeLock: 0,
      rentCollector: null,
      programId,
    });

    await new Promise((resolve) => setTimeout(resolve, 1000));

    const settings = await Settings.fromAccountAddress(
      connection,
      settingsPda
    );
    assert.strictEqual(settings.signers.length, 5);
    assert.strictEqual(settings.threshold, 5);

    // Fund the vault
    const [vaultPda] = smartAccount.getSmartAccountPda({
      settingsPda,
      accountIndex: 0,
      programId,
    });
    await connection.confirmTransaction(
      await connection.requestAirdrop(vaultPda, 2 * LAMPORTS_PER_SOL)
    );

    // Build the compiled instructions first so we can include the payload hash in the message
    const transferAmount = 1_000_000;
    const receiver = Keypair.generate();
    const transferIx = SystemProgram.transfer({
      fromPubkey: vaultPda,
      toPubkey: receiver.publicKey,
      lamports: transferAmount,
    });

    const { instructions: rawCompiledIxBytes, accounts: txAccounts } =
      smartAccount.utils.instructionsToSynchronousTransactionDetailsV2({
        vaultPda,
        members: [],
        transaction_instructions: [transferIx],
      });

    const compiledIxBytes = Buffer.from(rawCompiledIxBytes);

    // Hash the SyncPayload::Transaction(Vec<u8>) Borsh serialization:
    // [0x00 (enum variant), 4-byte LE length, ...compiled_ix_bytes]
    const payloadLenBytes = Buffer.alloc(4);
    payloadLenBytes.writeUInt32LE(compiledIxBytes.length);
    const payloadHash = sha256(
      Buffer.concat([Buffer.from([0x00]), payloadLenBytes, compiledIxBytes])
    );

    // Build sync consensus message:
    // sha256("squads-sync" || settings_key || transaction_index_le || payload_hash || next_nonce_le)
    const transactionIndex = BigInt(settings.transactionIndex.toString());
    const transactionIndexBytes = Buffer.alloc(8);
    transactionIndexBytes.writeBigUInt64LE(transactionIndex);
    const nonceBytes = Buffer.alloc(8);
    nonceBytes.writeBigUInt64LE(BigInt(1)); // next_nonce = current(0) + 1
    const hashedMessage = sha256(
      Buffer.concat([
        Buffer.from("squads-sync", "utf-8"),
        settingsPda.toBuffer(),
        transactionIndexBytes,
        Buffer.from(payloadHash),
        nonceBytes,
      ])
    );

    // Sign with P256 Webauthn signers (precompile path)
    const p256Sig1 = buildWebauthnSignature(
      rpIdHash,
      hashedMessage,
      rpId,
      1,
      p256Keypair1.privateKey
    );
    const p256Sig2 = buildWebauthnSignature(
      rpIdHash,
      hashedMessage,
      rpId,
      1,
      p256Keypair2.privateKey
    );

    // Sign with secp256k1 (syscall path — keccak256 of hashed message)
    const keccakHash = keccak_256(hashedMessage);
    const { signature: secp256k1Sig, recoveryId } = signSecp256k1(
      keccakHash,
      secp256k1Keypair.privateKey
    );

    // Sign with ed25519 external (syscall path — raw hashed message)
    const ed25519Sig = signEd25519External(
      hashedMessage,
      ed25519Keypair.privateKey
    );

    // Pack 2 P256 signatures into a single precompile instruction at index 0
    const precompileIx = buildSecp256r1MultiSigPrecompileInstruction([
      {
        signature: p256Sig1.signature,
        publicKeyCompressed: p256Keypair1.publicKeyCompressed,
        message: p256Sig1.precompileMessage,
      },
      {
        signature: p256Sig2.signature,
        publicKeyCompressed: p256Keypair2.publicKeyCompressed,
        message: p256Sig2.precompileMessage,
      },
    ]);

    // Build extra_verification_data as SmallVec<u8, ExtraVerificationData>
    // Order must match signer order: [native(skip), P256_1, P256_2, secp256k1, ed25519]
    // Native signers don't need EVD entries — only external signers do.
    const extraVerificationData = serializeExtraVerificationDataVec([
      {
        kind: ExtraVerificationDataKind.P256WebauthnPrecompile,
        typeAndFlags: 0x10, // TYPE_GET
        port: 0,
      },
      {
        kind: ExtraVerificationDataKind.P256WebauthnPrecompile,
        typeAndFlags: 0x10,
        port: 0,
      },
      {
        kind: ExtraVerificationDataKind.Secp256k1Syscall,
        signature: secp256k1Sig,
        recoveryId,
      },
      {
        kind: ExtraVerificationDataKind.Ed25519Syscall,
        signature: ed25519Sig,
      },
    ]);

    // Derive signer keys (must match on-chain key() method)
    const p256SignerKey1 = new PublicKey(
      p256Keypair1.publicKeyCompressed.slice(0, 32)
    );
    const p256SignerKey2 = new PublicKey(
      p256Keypair2.publicKeyCompressed.slice(0, 32)
    );
    const secp256k1SignerKey = new PublicKey(
      secp256k1Keypair.publicKeyUncompressed.slice(0, 32)
    );
    const ed25519SignerKey = new PublicKey(ed25519Keypair.publicKey);

    // Remaining accounts layout:
    // [0..4]  signers (native is_signer=true, external is_signer=false)
    // [5]     SYSVAR_INSTRUCTIONS
    // [6+]    transaction accounts (vault, receiver, SystemProgram)
    const allRemainingAccounts = [
      { pubkey: nativeSigner1.publicKey, isSigner: true, isWritable: false },
      { pubkey: p256SignerKey1, isSigner: false, isWritable: false },
      { pubkey: p256SignerKey2, isSigner: false, isWritable: false },
      { pubkey: secp256k1SignerKey, isSigner: false, isWritable: false },
      { pubkey: ed25519SignerKey, isSigner: false, isWritable: false },
      { pubkey: SYSVAR_INSTRUCTIONS_PUBKEY, isSigner: false, isWritable: false },
      ...txAccounts,
    ];

    const syncIx =
      smartAccount.generated.createExecuteTransactionSyncV2ExternalInstruction(
        {
          consensusAccount: settingsPda,
          program: programId,
          anchorRemainingAccounts: allRemainingAccounts,
        },
        {
          args: {
            accountIndex: 0,
            numSigners: 5,
            payload: {
              __kind: "Transaction",
              fields: [compiledIxBytes],
            },
          },
          extraVerificationData: null,
        },
        programId
      );
    // Patch EVD: beet.coption(beet.bytes) adds a 4-byte Vec length prefix
    // that doesn't exist in the on-chain Option<SmallVec<u8, ExtraVerificationData>>.
    // Replace the trailing null byte [0x00] with [0x01][correctly serialized bytes].
    const withoutNull = syncIx.data.subarray(0, syncIx.data.length - 1);
    syncIx.data = Buffer.concat([withoutNull, Buffer.from([0x01]), Buffer.from(extraVerificationData)]);

    // Build Solana transaction: [precompile @ ix 0, sync @ ix 1]
    const { blockhash } = await connection.getLatestBlockhash();
    const message = new TransactionMessage({
      payerKey: creator.publicKey,
      recentBlockhash: blockhash,
      instructions: [precompileIx, syncIx],
    }).compileToV0Message();

    const tx = new VersionedTransaction(message);
    tx.sign([creator, nativeSigner1]);

    // Skip simulation — simulateTransaction doesn't execute precompile
    // instructions correctly, always returning Custom(2) for secp256r1.
    const signature = await connection.sendRawTransaction(tx.serialize(), {
      skipPreflight: true,
    });
    await connection.confirmTransaction(signature);

    const txResult = await connection.getTransaction(signature, {
      commitment: "confirmed",
      maxSupportedTransactionVersion: 0,
    });
    if (txResult?.meta?.err) {
      throw new Error(
        `TX failed: ${JSON.stringify(txResult.meta.err)}\nLogs: ${txResult.meta.logMessages?.join("\n")}`
      );
    }

    const recipientBalance = await connection.getBalance(receiver.publicKey);
    assert.strictEqual(recipientBalance, transferAmount);
  });
});
