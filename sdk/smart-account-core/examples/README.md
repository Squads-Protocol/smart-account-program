# Check v1 signer compatibility

This optional example checks whether a wallet or key provider can sign the Solana v1 transactions this SDK sends.
The SDK is v1 only. If a provider cannot sign v1 messages, use a compatible signing path before migrating an account.

The example builds a v1 transaction (SIMD-0385) containing a single Memo instruction. The signer's address pays the transaction fee, so fund it on the chosen cluster first. `selftest` is offline; `submit` sends a real transaction.

```sh
cd sdk/smart-account-core && npm ci
node examples/check-v1-signer.mjs selftest
node examples/check-v1-signer.mjs build --payer <signer address> --rpc https://api.devnet.solana.com
#   → wireTransactionBase64 (unsigned), messageHex and lastValidBlockHeight
#   sign it (per signer, below), before that blockhash expires
node examples/check-v1-signer.mjs submit --tx <signed wire tx base64> --last-valid-block-height <height from build> --rpc https://api.devnet.solana.com
#   → verifies the signature locally, sends, prints PASS and the signature
```

Use the same cluster for `build` and `submit`. `--rpc` overrides `SOLANA_RPC_URL`; without either, the example uses public mainnet RPC.
The expiry height is not encoded in the wire transaction, so retain `lastValidBlockHeight` from `build` and pass it to `submit`.

## How each signer signs

| Signer | How to sign | Then |
|---|---|---|
| Privy server wallet | `POST https://api.privy.io/v1/wallets/{wallet_id}/rpc` with `{"method":"signTransaction","params":{"transaction":"<wireTransactionBase64>","encoding":"base64"}}` (app id/secret, plus authorization signature if the wallet has an owner) | `submit --tx <returned signed transaction> --last-valid-block-height <height from build>` |
| Privy embedded wallet | In a test app: the Privy React SDK's Solana `signTransaction` with the wire bytes | `submit` |
| Turnkey | `sign_raw_payload` with `payload = messageHex`, `encoding = PAYLOAD_ENCODING_HEXADECIMAL`, `hashFunction = HASH_FUNCTION_NOT_APPLICABLE` (the same shape Grid uses); signature = `r` + `s` | `attach --tx <wireTransactionBase64> --sig <r+s hex>`, then `submit` |
| Phantom, Solflare, Backpack | Wallet Standard `solana:signTransaction` with the wire bytes. First check that the wallet's `supportedTransactionVersions` includes `1` | `submit` |
| Ledger (via a wallet) | As the wallet above; note whether the device shows the message or blind-signs | `submit` |

Privy and the wallets parse the transaction before signing, so they can reject v1 even where the cryptography would work. Turnkey's raw-payload signing signs bytes and should be format-independent, but still test it end to end.

Record the signer provider, client/SDK version, cluster, result and transaction signature when validating a customer integration.
