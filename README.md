# Squads Smart Account Program v0.1

<img width="2500" alt="Frame 13" src="./assets/title_image.png">

![license][license-image]
![version][version-image]

[version-image]: https://img.shields.io/badge/version-0.1.0-blue.svg?style=flat
[license-image]: https://img.shields.io/badge/license-AGPL_3.0-blue.svg?style=flat

We are developing the Smart Account Program to address programmability and cost limitations related to deploying smart contract wallets at scale on Solana.

In order to deliver on this promise it builds on the following innovations:

- **Rent free wallet creation**: deploy wallets for as little as 0,0000025 SOL without paying rent until the account needs to execute transactions. Allowing developers to generate addresses for users at scale without worrying about deployment costs.
- **Atomic policy enforcement and transaction execution**: this program empowers developers with optimized smart contract wallets fit for building fully onchain PSPs, stablecoin banks and programmable wallets alike.
- **Archivable accounts**: recoup rent costs from inactive accounts without compromising on security using state compression. Enabling developers to confidently cover fees for their user's accounts by removing the burden of costs associated with inactive users.
- **Policies**: set rules on which an account can execute transactions and extend your account's functionality by creating your own policy programs.

The Smart Account Program is in active development, with regular updates posted on [squads.so/blog](http://squads.so/blog) and [@SquadsProtocol](https://x.com/SquadsProtocol) on X.

## Content

This repository contains:

- The Squads Smart Account v0.1 program.
- The `@sqds/smart-account` Typescript SDK to interact with the smart account program.

## Program (Smart contract) Addresses

The Squads Smart Account Program v0.1 is deployed to:

- Solana Mainnet-beta: `SMRTzfY6DfH5ik3TKiyLFfXexV8uSG3d2UksSCYdunG`
- Solana Devnet: `SMRTzfY6DfH5ik3TKiyLFfXexV8uSG3d2UksSCYdunG`

Deployments can be checked using [Solana Verifiable Build](https://github.com/solana-foundation/solana-verifiable-build). The mainnet build parameters and explorer verification workflow are documented below; devnet must be checked separately.

## Responsibility

By interacting with this program, users acknowledge and accept full personal responsibility for any consequences, regardless of their nature. This includes both potential risks inherent to the smart contract, also referred to as program, as well as any losses resulting from user errors or misjudgment.

By using a smart account, it is important to acknowledge certain concepts. Here are some that could be misunderstood by users:

- Loss of Private Keys: If a participant loses their private key, the smart account may not be able to execute transactions if a threshold number of signatures is required.
- Single Point of Failure with Keys: If all keys are stored in the same location or device, a single breach can compromise the smart account.
- Forgetting the Threshold: Misremembering the number of signatures required can result in a deadlock, where funds cannot be accessed.
- No Succession Planning: If keyholders become unavailable (e.g., due to accident, death), without a plan for transition, funds may be locked forever.
- Transfer of funds to wrong address: Funds should always be sent to the smart account account, and not the smart account settingsaddress. Due to the design of the Squads Protocol program, funds deposited to the smart account may not be recoverable.
- If the settings_authority of a smart account is compromised, an attacker can change smart account settings, potentially reducing the required threshold for transaction execution or instantly being able to remove and add new members.
- If the underlying SVM compatible blockchain undergoes a fork and a user had sent funds to the orphaned chain, the state of the blockchain may not interpret the owner of funds to be original one.
- Users might inadvertently set long or permanent time-locks in their smart account, preventing access to their funds for that period of time.
- Smart account participants might not have enough of the native token of the underlying SVM blockchain to pay for transaction and state fees.

## Developers

You can interact with the Squads Smart Account Program via our SDKs.

List of SDKs:

- Typescript SDK: `@sqds/smart-account` (not yet published)

## Compiling and testing

You can compile the code with Anchor.

```bash
anchor build
```

To successfully execute `anchor build` for this project, you must have Solana CLI version 1.18.16 (`solana-cli 1.18.16`) installed.

### How to Check Your Solana Version

```bash
# Run the following command:
solana --version
```

If your version is not `1.18.16`, you need to switch to the required version.

### How to Switch Solana CLI Version

Use the Agave installer to initialize the correct version:

```bash
agave-install init 1.18.16
```

### Installing Anchor

If you do not have the Solana Anchor framework CLI installed, you can do so by following [this guide](https://www.anchor-lang.com/docs/installation).

To deploy the program on a local validator instance for testing or development purposes, you can create a local instance by running this command from the [Solana CLI](https://docs.solana.com/cli/install-solana-cli-tools).

```bash
solana-test-validator
```

To run the tests, first install the node modules for the repository.

```bash
yarn
```

And run these tests with this command:

```bash
yarn test
```

### Verifying the code

The mainnet deployment at slot `443245754` (August 31, 2026) can be reproduced from commit [`0e510821e00bf2a02716e3933b8659fa36ab2bee`](https://github.com/Squads-Protocol/smart-account-program/commit/0e510821e00bf2a02716e3933b8659fa36ab2bee), which includes the async ALT account-resolution fix. Its executable hash is:

```text
1cacfe2080faabf33a02ea285b6f8c8182358d3c6e40e3e7375ad0d7baa58cd4
```

Use Docker with `solana-verify` version [`0.5.2`](https://github.com/solana-foundation/solana-verifiable-build/releases/tag/v0.5.2) and the pinned Solana **1.18.16** image below. Although `Cargo.lock` resolves `solana-program` to 1.18.26, the deployed executable was reproduced with the 1.18.16 build image. Specify the image explicitly and retain the committed lockfile. Build with the default production features; `--features testing` selects a different program ID.

#### Verify the published source against mainnet

Install and start Docker, then install the tested CLI using a recent Rust toolchain:

```bash
cargo +stable install solana-verify --version 0.5.2 --locked

REPO_URL=https://github.com/Squads-Protocol/smart-account-program
VERIFIED_COMMIT=0e510821e00bf2a02716e3933b8659fa36ab2bee
PROGRAM_ID=SMRTzfY6DfH5ik3TKiyLFfXexV8uSG3d2UksSCYdunG
BUILD_IMAGE=solanafoundation/solana-verifiable-build@sha256:1388b6e423013b0a4a1b67b3481b1c35f5a13034af97d9059439d923c19f1c87
RPC_URL=https://api.mainnet-beta.solana.com

solana-verify verify-from-repo "$REPO_URL" \
  --commit-hash "$VERIFIED_COMMIT" \
  --program-id "$PROGRAM_ID" \
  --library-name squads_smart_account_program \
  --base-image "$BUILD_IMAGE" \
  --url "$RPC_URL"
```

This clones the pinned commit, builds it in Docker, and compares its executable hash with mainnet. Both hashes must match the value above. Answer **no** when prompted to upload verification data: the upgrade authority is a Squads vault, so maintainers must publish the metadata through Squads as described below. Use a dedicated mainnet RPC endpoint if the public endpoint is rate limited.

The repository root contains the workspace `Cargo.toml` and `Cargo.lock`, so leave `--mount-path` unset. The library name selects the program within that workspace.

To build a local artifact independently, use a clean checkout of the same commit and run from the repository root, with the variables above set:

```bash
solana-verify build \
  --library-name squads_smart_account_program \
  --base-image "$BUILD_IMAGE"
solana-verify get-executable-hash target/deploy/squads_smart_account_program.so
solana-verify get-program-hash --url "$RPC_URL" "$PROGRAM_ID"
```

`anchor build` is intended for development; use the pinned Docker build for this hash comparison. Devnet currently has a different executable and is not covered by this mainnet source/hash claim.

#### Publish verification metadata through Squads (maintainers)

A matching local build does not by itself register a verified build with explorers. The current upgrade authority is `HT3JknwuufXdtVJggz5Z9JcnYtanPpLzTCqLWsVX1Vu2`. Confirm it before exporting metadata:

```bash
solana program show --url "$RPC_URL" "$PROGRAM_ID"

solana-verify export-pda-tx "$REPO_URL" \
  --commit-hash "$VERIFIED_COMMIT" \
  --program-id "$PROGRAM_ID" \
  --library-name squads_smart_account_program \
  --base-image "$BUILD_IMAGE" \
  --uploader HT3JknwuufXdtVJggz5Z9JcnYtanPpLzTCqLWsVX1Vu2 \
  --url "$RPC_URL" \
  --encoding base58 \
  --compute-unit-price 0
```

Import the emitted base58 transaction into the controlling Squads multisig. Check that its top-level instructions call only the verification program (`verifycLy8mB96wd9wqq3WDXQwM4oU6r42Th37Db9fC`) and, optionally, the compute-budget program, then simulate, approve, and execute it through the multisig. This registers the build parameters and does not upgrade the Smart Account Program.

After execution, read back the metadata and submit the remote build:

```bash
solana-verify get-program-pda --url "$RPC_URL" \
  --program-id "$PROGRAM_ID" \
  --signer HT3JknwuufXdtVJggz5Z9JcnYtanPpLzTCqLWsVX1Vu2

solana-verify remote submit-job \
  --program-id "$PROGRAM_ID" \
  --uploader HT3JknwuufXdtVJggz5Z9JcnYtanPpLzTCqLWsVX1Vu2

solana-verify remote get-job --job-id REPLACE_WITH_RETURNED_JOB_ID
```

Confirm that the recorded commit, image, library, and deployment slot are correct before submitting the job. Once the remote job succeeds, check [OtterSec verification status](https://verify.osec.io/status/SMRTzfY6DfH5ik3TKiyLFfXexV8uSG3d2UksSCYdunG), [Solana Explorer](https://explorer.solana.com/address/SMRTzfY6DfH5ik3TKiyLFfXexV8uSG3d2UksSCYdunG/verified-build), and [Solscan](https://solscan.io/account/SMRTzfY6DfH5ik3TKiyLFfXexV8uSG3d2UksSCYdunG). Remote verification is supported on mainnet; see the [Solana verified-builds guide](https://solana.com/docs/programs/verified-builds) for the full workflow. Repeat the metadata and remote verification steps after future program upgrades, using the new source commit and build parameters.

## Security

The Squads Smart Account Program has been audited by Ottersec and Certora and additionally formally verified by Certora.

- Certora FV & Audit: [View Full Report](./audits/certora_smart_account_audit+FV.pdf)
- Ottersec Audit: [View Full Report](./audits/ottersec_smart_account_audit.pdf)

## License

The primary license for Squads Smart Account Program is the AGPL-3.0 license, see [LICENSE](./LICENSE). The following exceptions are licensed separately as follows:

- The file <https://github.com/Squads-Protocol/smart-account-program/blob/main/programs/squads_smart_account_program/src/utils/system.rs> is derived from code released under the [Apache 2.0 license](https://github.com/coral-xyz/anchor/blob/master/LICENSE) at <https://github.com/coral-xyz/anchor/blob/714d5248636493a3d1db1481f16052836ee59e94/lang/syn/src/codegen/accounts/constraints.rs#L1126-L1179>.
- The file <https://github.com/Squads-Protocol/smart-account-program/blob/main/programs/squads_smart_account_program/src/utils/small_vec.rs> is derived from code released under both the [Apache 2.0 license](https://github.com/near/borsh-rs/blob/master/LICENSE-APACHE) and the [MIT license](https://github.com/near/borsh-rs/blob/master/LICENSE-MIT) at <https://github.com/near/borsh-rs/blob/master/borsh/src/de/hint.rs> and <https://github.com/near/borsh-rs/blob/master/borsh/src/ser/mod.rs>.

To the extent that each such file incorporates code from another source, such code is licensed under its respective open source license as provided above, and the original open source code is copyrighted by its respective owner as provided above.
