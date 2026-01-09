# Private Institutional Bond Protocol

A zero-coupon bond protocol using zero-knowledge proofs to keep transaction amounts private while maintaining auditability on-chain.

## Overview

This is a proof-of-concept implementation of a privacy-preserving bond protocol. Bondholders can trade and redeem bonds without revealing transaction amounts to the blockchain—only nullifiers and commitments are visible.

### Key features:

- Zero-coupon bonds with maturity enforcement
- UTXO-based note model for privacy
- Atomic peer-to-peer trading
- Redemption at maturity
- All amounts private; only identities and opaque events visible on-chain

## Repository Structure

### `/circuits`

Noir ZK circuits implementing the core proving system.

- `src/main.nr` — JoinSplit circuit (2-input, 2-output)
- Proves: ownership, merkle membership, balance conservation, maturity constraints

### `/contracts`

Solidity smart contracts for on-chain settlement.

- `src/PrivateBond.sol` — Main contract with `mint`, `atomicSwap`, and `burn` functions
- `src/Verifier.sol` — HONK proof verifier (generated)

### `/wallet`

Rust CLI emulating the full user flow.

- Generate bonds (onboarding)
- Trade bonds (atomicSwap)
- Redeem at maturity (burn)
- Query bond state and merkle proofs

### `/SPEC.md`

Full specification covering cryptography, protocol flow, security assumptions, and privacy analysis.

## How It Works: UTXO Model

`Notes` are like coins. Each note contains:

- `value`: amount of the note
- `owner`: shielded public key derived from user seed
- `salt`: random number used for uniqueness
- `maturityDate`: maturity date of the bond
- `assetId`: ID of the asset of the note

### Note Commitment

`Commitment = Hash(value, salt, owner, assetId, maturityDate)`

### Note storage

All commitments are stored in a merkle tree on-chain. Proves membership without revealing which specific commitments you own.

### Nullify a note to avoid double spending

`Nullifier = Hash(salt, private_key)`
When you spend a note, you publish its nullifier to prevent double-spending. No one can link the nullifier back to the note.

### Transactions

- **Onboarding**: User creates a note and commits it to the merkle tree
- **Trading**: User combines 2 notes (100 + 60) into 2 new ones (60 + 40), proving balance conservation
- **Redemption**: At maturity, user burns a note to redeem cash, proving they own it via ZK

All values stay private. Only hashes, nullifiers, and transaction confirmations are visible on-chain.

## Getting Started

```bash
# Build contracts
cd contracts && forge build

# Run tests
forge test

# Build circuits
cd ../circuits && nargo build

# Test circuits
cd ../circuits && nargo test

# CLI (coming soon)
cd ../wallet && cargo run
```
