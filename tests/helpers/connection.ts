import {
  Connection,
  Keypair,
  PublicKey,
} from "@solana/web3.js";
import * as smartAccount from "@sqds/smart-account";
import { readFileSync } from "fs";
import path from "path";

export function getTestProgramId() {
  const programKeypair = Keypair.fromSecretKey(
    Buffer.from(
      JSON.parse(
        readFileSync(
          path.join(
            __dirname,
            "../../target/deploy/squads_smart_account_program-keypair.json"
          ),
          "utf-8"
        )
      )
    )
  );

  return programKeypair.publicKey;
}

export function getTestProgramConfigInitializer() {
  return Keypair.fromSecretKey(
    Buffer.from(
      JSON.parse(
        readFileSync(
          path.join(
            __dirname,
            "../../test-program-config-initializer-keypair.json"
          ),
          "utf-8"
        )
      )
    )
  );
}
export function getProgramConfigInitializer() {
  return Keypair.fromSecretKey(
    Buffer.from(
      JSON.parse(
        readFileSync(
          "/Users/orion/Desktop/Squads/sqdcVVoTcKZjXU8yPUwKFbGx1Hig1rhbWJQtMRXp2E1.json",
          "utf-8"
        )
      )
    )
  );
}
export function getTestProgramConfigAuthority() {
  return Keypair.fromSecretKey(
    new Uint8Array([
      58, 1, 5, 229, 201, 214, 134, 29, 37, 52, 43, 109, 207, 214, 183, 48, 98,
      98, 141, 175, 249, 88, 126, 84, 69, 100, 223, 58, 255, 212, 102, 90, 107,
      20, 85, 127, 19, 55, 155, 38, 5, 66, 116, 148, 35, 139, 23, 147, 13, 179,
      188, 20, 37, 180, 156, 157, 85, 137, 29, 133, 29, 66, 224, 91,
    ])
  );
}

export function getTestProgramTreasury() {
  return Keypair.fromSecretKey(
    new Uint8Array([
      232, 179, 154, 90, 210, 236, 13, 219, 79, 25, 133, 75, 156, 226, 144, 171,
      193, 108, 104, 128, 11, 221, 29, 219, 139, 195, 211, 242, 231, 36, 196,
      31, 76, 110, 20, 42, 135, 60, 143, 79, 151, 67, 78, 132, 247, 97, 157, 8,
      86, 47, 10, 52, 72, 7, 88, 121, 175, 107, 108, 245, 215, 149, 242, 20,
    ])
  ).publicKey;
}
export function getTestAccountCreationAuthority() {
  return Keypair.fromSecretKey(
    Buffer.from(
      JSON.parse(
        readFileSync(
          path.join(__dirname, "../../test-account-creation-authority.json"),
          "utf-8"
        )
      )
    )
  );
}

export function createLocalhostConnection() {
  return new Connection("http://127.0.0.1:8899", "confirmed");
}

export const getLogs = async (
  connection: Connection,
  signature: string
): Promise<string[]> => {
  const tx = await connection.getTransaction(signature, {
    commitment: "confirmed",
  });
  return tx!.meta!.logMessages || [];
};

export async function getNextAccountIndex(
  connection: Connection,
  programId: PublicKey
): Promise<bigint> {
  const [programConfigPda] = smartAccount.getProgramConfigPda({ programId });
  const programConfig =
    await smartAccount.accounts.ProgramConfig.fromAccountAddress(
      connection,
      programConfigPda,
      "processed"
    );
  const accountIndex = BigInt(programConfig.smartAccountIndex.toString());
  const nextAccountIndex = accountIndex + 1n;
  return nextAccountIndex;
}
