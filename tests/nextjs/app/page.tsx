import Image from "next/image";
import * as smartAccount from "@sqds/smart-account";
import { Keypair } from "@solana/web3.js";

export default function Home() {
  const accountIndex = 0n;
  const [settingsPda] = smartAccount.getSettingsPda({ accountIndex });
  const [smartAccountPda] = smartAccount.getSmartAccountPda({ settingsPda, accountIndex: 0 });
  return (
    <main className="flex min-h-screen flex-col items-center justify-between p-24">
      Hello world, {smartAccountPda.toBase58()}
    </main>
  );
}
