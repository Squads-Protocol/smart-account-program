import Image from "next/image";
import * as smartAccount from "@sqds/smart-account";
import { Keypair } from "@solana/web3.js";

export default function Home() {
  const createKey = Keypair.generate().publicKey;
  const multisigPda = smartAccount.getMultisigPda({ createKey })[0].toBase58();
  return (
    <main className="flex min-h-screen flex-col items-center justify-between p-24">
      Hello world, {multisigPda}
    </main>
  );
}
