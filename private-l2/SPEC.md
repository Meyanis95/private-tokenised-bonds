# Private Institutional Bond on Aztec L2

## Approach

This approach builds on the custom-private-utxo design by deploying on Aztec, an Ethereum L2 with native privacy.

Aztec enshrines the privacy primitives we built manually (notes, nullifiers, Merkle trees, ZK proofs) into a distributed network:

- **Privacy**: Encrypted notes, nullifiers, private execution handled by protocol
- **Ethereum security**: ZK rollup settling to L1
- **Programmability**: Contracts can mix public and private state/functions
- **Decentralization**: Sequencer network (removes trusted relayer)

**Contract implements:**

- Bond lifecycle (issuance, transfer, redemption)
- Whitelist enforcement
- Issuer admin controls
- Regulatory viewing keys

## Identity & Access Model

**Aztec Address**: Each user has an Aztec account (contract wallet) identified by `AztecAddress`. This address is public and used for:

- Whitelist registry entries
- Note recipient identification
- Transaction authorization

**Whitelist**: Public mapping of approved addresses. Maintained by issuer.

```
whitelist: Map<AztecAddress, bool>
```

**Enforcement**: Transfer functions check whitelist status before execution. For private transfers, the check reads public state (whitelist) from within a private function context.

**Privacy Model**: Sender and recipient addresses are linkable per transaction (visible via whitelist reads). Amounts and balances remain hidden. This aligns with institutional requirements where participant identities are public but positions are confidential.

> **Mitigation (not in PoC)**: To unlink participants, the whitelist could be stored as a Merkle tree root with users proving membership via private inclusion proofs.

**Issuer Role**: Single privileged address stored in public state. Can:

- Mint new bond notes
- Add/remove addresses from whitelist
- Transfer ownership

## Storage Structure

**Public State:**

```
owner: PublicMutable<AztecAddress>
whitelist: Map<AztecAddress, bool>
total_supply: PublicMutable<u64>
maturity_date: PublicMutable<u64>
```

**Private State:**

```
balances: Map<AztecAddress, BalanceSet>
```

`BalanceSet` is Aztec's private balance primitive - a set of encrypted notes representing a user's holdings. Notes are created on mint/receive and consumed on transfer/burn.

## Primary Market: Issuance

1. Investor completes KYC off-chain
2. Issuer adds investor address to whitelist
3. Investor sends fiat payment via traditional rails
4. Issuer confirms payment receipt
5. Issuer calls `mint_private(investor, amount)`:
   - Checks caller is owner
   - Checks investor is whitelisted
   - Creates private note for investor's balance
   - Increments total_supply (public)

The minted amount is hidden. Only total_supply change is visible (reveals aggregate issuance, not individual positions).

**Alternative - On-chain payment**: Investor can pay with tokens already on Aztec (e.g., private stablecoins). This enables atomic DvP for primary market: bond notes minted in exchange for private payment tokens in a single transaction.

**L1 Bridge flow**: Investor with L1 assets (e.g., USDC) can:

1. Bridge to Aztec L2 via portal (public on L2)
2. Shield tokens (public → private)
3. Use private tokens for bond purchase

The shield step reveals that _some_ amount was shielded, but subsequent trades remain private. This improves on previous custom private utxo approach only approaches where every shield/unshield is fully visible on mainnet.

## Secondary Market: Trading

Peer-to-peer trading via atomic DvP: bond tokens exchanged for payment tokens in a single transaction.

**Simple Transfer** (bond only):

```
transfer_private(to: AztecAddress, amount: u64)
```

- Checks both sender and recipient are whitelisted
- Consumes sender's notes, creates note for recipient

**Atomic DvP** (bond ↔ payment token):

Requires interaction with a separate payment token contract (e.g., private stablecoin). Two approaches:

1. **[Authwit pattern](https://docs.aztec.network/developers/docs/foundational-topics/advanced/authwit)**: Alice authorizes bond contract to spend her payment tokens. Bob authorizes payment contract to spend his bonds. Single transaction executes both legs atomically.

2. **Swap contract**: Dedicated escrow contract that holds both legs and releases atomically when both parties have deposited.

For PoC, approach 1 (authwit) is simpler and uses Aztec's native authorization pattern.

## Redemption & Maturity

At maturity, bondholders redeem notes for par value.

```
redeem(amount: u64)
```

1. Check `block.timestamp >= maturity_date`
2. Check caller is whitelisted
3. Burn caller's bond notes (consume without creating new notes)
4. Decrement total_supply (public)

**Settlement**: Two options:

- **Off-chain**: Issuer observes redemption event, initiates fiat transfer via traditional rails
- **On-chain**: Atomic exchange of bond notes for payment tokens (same as DvP)

## Regulatory Viewing Keys

Regulators need read-only access to transaction details without participating in trades.

Aztec accounts have separate key pairs:

- **Spending key**: authorizes transactions
- **Viewing key**: decrypts notes (read-only)

**Approach**: Users share their viewing key with regulators. Regulator can then decrypt and inspect that user's note history (balances, transaction amounts) without ability to spend.

**Scope options**:

- **Per-user**: Regulator receives viewing keys from individual users on request
- **Issuer-mediated**: Issuer collects viewing keys at onboarding, provides to regulator as needed

**Limitations**:

- Viewing key reveals all notes for that user, not selective per-transaction disclosure
- No native support for "auditor key" that sees all contract activity

> **Future (not in PoC)**: Selective disclosure via note-level encryption to multiple parties, or ZK proofs of compliance without revealing underlying data.

## Security Model

**Trust Assumptions**:

- Aztec protocol is secure (ZK proofs, sequencer, L1 settlement)
- Issuer is honest for whitelist management
- Users secure their own keys

**What Aztec handles** (vs custom-utxo):

- Double-spend prevention via nullifiers ✅
- Note privacy via encryption ✅
- Balance integrity via protocol constraints ✅
- Sequencer censorship resistance (decentralized sequencer) ✅
- Frontrunning mitigation (encrypted mempool) ✅

**Remaining application-level concerns**:

- Issuer censorship: Issuer can refuse to whitelist addresses (acceptable for regulated context)
- Viewing key scope: All-or-nothing per user (no selective disclosure)

## Terminology

Privacy primitives (notes, nullifiers, commitments, Merkle trees) are defined and implemented by Aztec. See [Aztec documentation](https://docs.aztec.network/) for protocol-level details.

Bond-specific terms follow the custom-private-utxo spec.
