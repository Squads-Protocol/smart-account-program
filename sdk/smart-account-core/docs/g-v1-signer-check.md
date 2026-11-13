# G-v1: can each signer sign a v1 transaction?

**Decision date:** Nov 13, 2026. If a signer a customer depends on fails, the SDK falls back to the documented v0 recipe for that customer (spec v0.4, open decision 3).

The test: each signer signs one v1 transaction (SIMD-0385) containing a single Memo instruction, and the transaction lands on mainnet. The signer's address pays the fee (about 5,000 lamports), so fund it first.

```sh
cd sdk/smart-account-core && npm ci
node scripts/g-v1-signer-check.mjs selftest           # checks the script itself, offline
node scripts/g-v1-signer-check.mjs build --payer <signer address>
#   → wireTransactionBase64 (unsigned) and messageHex
#   sign it (per signer, below), within about 60 seconds
node scripts/g-v1-signer-check.mjs submit --tx <signed wire tx base64>
#   → verifies the signature locally, sends, prints PASS and the signature
```

`SOLANA_RPC_URL` overrides the public mainnet RPC. Use `--rpc https://api.devnet.solana.com` for a devnet dry run.

## How each signer signs

| Signer | How to sign | Then |
|---|---|---|
| Privy server wallet | `POST https://api.privy.io/v1/wallets/{wallet_id}/rpc` with `{"method":"signTransaction","params":{"transaction":"<wireTransactionBase64>","encoding":"base64"}}` (app id/secret, plus authorization signature if the wallet has an owner) | `submit --tx <returned signed transaction>` |
| Privy embedded wallet | In a test app: the Privy React SDK's Solana `signTransaction` with the wire bytes | `submit` |
| Turnkey | `sign_raw_payload` with `payload = messageHex`, `encoding = PAYLOAD_ENCODING_HEXADECIMAL`, `hashFunction = HASH_FUNCTION_NOT_APPLICABLE` (the same shape Grid uses); signature = `r` + `s` | `attach --tx <wireTransactionBase64> --sig <r+s hex>`, then `submit` |
| Phantom, Solflare, Backpack | Wallet Standard `solana:signTransaction` with the wire bytes. First check that the wallet's `supportedTransactionVersions` includes `1` | `submit` |
| Ledger (via a wallet) | As the wallet above; note whether the device shows the message or blind-signs | `submit` |

Privy and the wallets parse the transaction before signing, so they can reject v1 even where the cryptography would work. Turnkey's raw-payload signing signs bytes and should be format-independent, but still test it end to end.

## Results

| Signer | Version / SDK | Result | Signature | Notes |
|---|---|---|---|---|
| Privy server wallet | | | | |
| Privy embedded wallet | | | | |
| Turnkey (raw payload) | | | | |
| Phantom | | | | |
| Solflare | | | | |
| Backpack | | | | |
