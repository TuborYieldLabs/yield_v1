# Tubor Yield Protocol

<p align="center">
  <img src="https://github.com/TuborYieldLabs/yield_v1/blob/main/kit/logo.png?raw=true" width="120" alt="Tubor Yield Logo"/>
</p>

<h1 align="center">Tubor Yield Protocol</h1>

<p align="center">
  <a href="LICENSE"><img src="https://img.shields.io/badge/License-ISC-blue.svg" alt="License"></a>
  <a href="https://solana.com/"><img src="https://img.shields.io/badge/Solana-Anchor-blueviolet" alt="Solana"></a>
  <a href="#"><img src="https://img.shields.io/badge/build-anchor-green" alt="Build"></a>
</p>

---

## Overview

Tubor Yield is a robust, security-focused DeFi protocol built on Solana, leveraging the Anchor framework. It features a modular architecture for yield generation, agent-based trading, and comprehensive on-chain risk management.

---

## 🚀 Features

- **Global Protocol State:** Centralized management of protocol parameters, balances, and security controls.
- **Agent System:** Mint, buy, sell, and transfer protocol agents and master agents (NFTs) with fine-grained permissions.
- **Yield Management:** Secure, rate-limited yield updates with multisig protection and event emission.
- **Trade Engine:** On-chain trade lifecycle management with risk validation, oracle consensus, and circuit breaker protection.
- **Security Controls:** Emergency pause, circuit breaker, rate limiting, and parameter bounds for protocol safety.
- **Referral & User System:** On-chain user accounts, referral tracking, and status management.
- **Comprehensive Error Handling:** Custom error codes and robust result types for all operations.

---

## 📚 Technical Documentation

### Architecture

- **Solana/Anchor:** All core logic is implemented as a Solana program using Anchor.
- **Instructions:** Each protocol action (e.g., open trade, mint agent, update yield) is an Anchor instruction with strict account validation and parameter checks.
- **State:** On-chain state is managed via Anchor accounts for protocol, agents, users, trades, and more.
- **Security:** Multisig, circuit breaker, and rate limiting are enforced at the protocol and agent level.

### Main Instructions (API)

<details>
<summary><strong>Click to expand API details</strong></summary>

#### 1. `init`

Initialize the protocol, multisig, and authority accounts.

- **Params:** `InitParams { min_signatures, allow_agent_deploy, allow_agent_buy, allow_agent_sell, allow_withdraw_yield, buy_tax, sell_tax, max_tax_percentage, ref_earn_percentage, supported_mint, ... }`
- **Accounts:** Upgrade authority, multisig PDA, protocol state PDA, transfer authority PDA, supported mint, etc.

#### 2. `register_user`

Register a new user, optionally with a referrer.

- **Params:** `RegisterUserParams { name: [u8; 15], referrer: Option<Pubkey> }`
- **Accounts:** payer, authority, user PDA, (optional) referrer, referral registry, referral link, t_yield, event authority, system program.

#### 3. `mint_master_agent`

Mint a new master agent NFT (requires multisig).

- **Params:** `MintMasterAgentParams { name, symbol, uri, seller_fee_basis_points, price, w_yield, max_supply, trading_status, auto_relist }`
- **Accounts:** payer, multisig, master agent PDA, mint, t_yield, transfer authority, Metaplex metadata, etc.

#### 4. `mint_agent`

Mint a new agent NFT under a master agent (requires multisig).

- **Params:** `MintAgentParams { name, symbol, uri, seller_fee_basis_points }`
- **Accounts:** payer, multisig, t_yield, transfer authority, mint, metadata, master agent, agent, token account, etc.

#### 5. `buy_agent`

Buy an agent NFT from a master agent.

- **Params:** None (all info from accounts)
- **Accounts:** authority, user, agent, master agent, t_yield, transfer authority, y_mint, token accounts, event authority, etc.

#### 6. `open_trade`

Open a new trade.

- **Params:** `OpenTradeParams { entry_price, take_profit, size, stop_loss, trade_type, feed_id, trade_pair }`
- **Accounts:** authority, t_yield, multisig, oracle, twap, master agent, master agent mint, trade, system program.

#### 7. `update_yield`

Update a master agent’s yield rate (requires multisig).

- **Params:** `UpdateYieldParams { new_yield_rate }`
- **Accounts:** authority, multisig, t_yield, master agent, master agent mint, system program, event authority.

#### 8. `update_trade`

Update a trade’s status based on current price (can be called by anyone).

- **Params:** None
- **Accounts:** authority, t_yield, oracle, twap, trade, master agent, event authority, system program.

#### 9. `update_protocol_config`

Update protocol configuration (requires multisig).

- **Params:** `UpdateProtocolConfigParams { buy_tax, sell_tax, max_tax_percentage, allow_agent_deploy, ... }`
- **Accounts:** admin, multisig, t_yield, system program, event authority.

#### 10. `pause_protocol` / `unpause_protocol`

Pause or unpause the protocol (requires multisig).

- **Params:** None
- **Accounts:** admin, multisig, t_yield.

#### 11. `claim_referral_rewards`, `withdraw_yield`, `ban_user`, etc.

See the `instructions/` directory for full details.

</details>

---

## 🛠️ Usage Example

Below is a TypeScript example using the Anchor/TypeScript SDK to register a user and open a trade:

```typescript
import { Program, AnchorProvider, web3 } from "@coral-xyz/anchor";
import { TuborYield } from "../target/types/tubor_yield";

// Set up provider and program
const provider = AnchorProvider.env();
const program = new Program<TuborYield>(IDL, programId, provider);

// Register a new user
await program.methods
  .registerUser({ name: Buffer.from("alice"), referrer: null })
  .accounts({
    payer: provider.wallet.publicKey,
    authority: provider.wallet.publicKey,
    user: userPda,
    tYield: tYieldPda,
    eventAuthority: eventAuthorityPda,
    systemProgram: web3.SystemProgram.programId,
  })
  .rpc();

// Open a trade
await program.methods
  .openTrade({
    entryPrice: 100_000_000, // 100.0 (6 decimals)
    takeProfit: 120_000_000,
    size: 1_000_000,
    stopLoss: 90_000_000,
    tradeType: { buy: {} },
    feedId: new Array(32).fill(0),
    tradePair: new Array(8).fill(0),
  })
  .accounts({
    authority: provider.wallet.publicKey,
    tYield: tYieldPda,
    multisig: multisigPda,
    pairOracleAccount: oraclePda,
    masterAgent: masterAgentPda,
    masterAgentMint: masterAgentMintPda,
    trade: tradePda,
    systemProgram: web3.SystemProgram.programId,
  })
  .rpc();
```

---

## 🤝 Contribution Guidelines

We welcome contributions from the community! To contribute:

1. **Fork the repository** and create your branch from `main`.
2. **Write clear, concise code** and document your changes.
3. **Add tests** for any new features or bug fixes.
4. **Run `yarn lint` and `yarn test`** to ensure code quality and correctness.
5. **Open a pull request** with a clear description of your changes and reference any related issues.

**Code Style:**

- Use Prettier for formatting (`yarn lint:fix`).
- Follow Rust and TypeScript best practices.
- Keep PRs focused and minimal.

**Reporting Issues:**

- Please use GitHub Issues for bug reports and feature requests.
- Include as much detail as possible (logs, steps to reproduce, etc.).

---

## 🏗️ Building Locally

### Prerequisites

- [Rust](https://www.rust-lang.org/tools/install)
- [Solana CLI](https://docs.solana.com/cli/install-solana-cli-tools)
- [Anchor](https://book.anchor-lang.com/getting_started/installation.html)
- [Node.js](https://nodejs.org/) and [Yarn](https://yarnpkg.com/)

### Build the Solana Program

```sh
anchor build
```

### Run Tests

```sh
yarn test
```

### Lint

```sh
yarn lint
```

---

## 🗂️ Project Structure

```
programs/tubor_yield/         # Main Solana program (Rust, Anchor)
  └── src/
      ├── instructions/       # All protocol instructions (init, trade, yield, agent, etc.)
      ├── state/              # On-chain state: protocol, agents, users, trades, etc.
      ├── math/               # Math utilities and primitives
      ├── error.rs            # Custom error codes and result types
      └── lib.rs              # Program entrypoint
migrations/                   # Anchor migration and deployment scripts
tests/                        # (Empty) Add integration tests here
package.json                  # Node.js scripts and dependencies
tsconfig.json                 # TypeScript configuration
Anchor.toml                   # Anchor/cluster configuration
```

---

## 🔒 Security

- **Multisig Protection:** All critical updates require multisig approval.
- **Emergency Controls:** Circuit breaker and pause functionality for rapid response.
- **Rate Limiting:** Prevents excessive or rapid parameter changes.
- **Comprehensive Validation:** All operations are guarded by strict validation and access control.

---

## 📄 License

This project is licensed under the ISC License.

---

## 📝 Licenses

- **Tubor Yield Protocol:** Apache-2.0 License (see LICENSE file)
- **Anchor Framework:** Apache-2.0 License ([github.com/coral-xyz/anchor](https://github.com/coral-xyz/anchor))
- **Solana:** Apache-2.0 License ([github.com/solana-labs/solana](https://github.com/solana-labs/solana))
- **Metaplex:** Apache-2.0 License ([github.com/metaplex-foundation/metaplex-program-library](https://github.com/metaplex-foundation/metaplex-program-library))

This project may use other open source dependencies. Please review their respective repositories for detailed license information.
