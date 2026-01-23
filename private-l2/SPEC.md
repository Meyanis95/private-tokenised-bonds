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

- Distribute bond notes from initial pool
- Add/remove addresses from whitelist
- Transfer ownership
- Settle redemption requests

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
private_balances: Map<AztecAddress, BalanceSet>
```

`BalanceSet` is Aztec's private balance primitive - a set of encrypted notes representing a user's holdings. Notes are created on receive and consumed on transfer/burn.

**Fixed Supply Model**

Total supply is set once at contract initialization and **never changes**. The entire supply is minted to the issuer's private balance at deployment. This prevents observers from deducing transaction amounts by watching supply changes.

## Primary Market: Issuance

The issuer deploys contract with `initialize(total_supply, maturity_date)`, this is going to mint the total supply of bonds to isseur's private balance.

**Distribution Flow:**

1. Investor completes KYC off-chain
2. Issuer adds investor address to whitelist
3. Investor sends fiat payment via traditional rails
4. Issuer confirms payment receipt
5. Issuer calls `distribute_private(investor, amount)`:
   - Checks caller is owner
   - Checks investor is whitelisted
   - Transfers from issuer's private balance to investor
   - Total supply unchanged

Observer can see that the investor received bonds (via whitelist check) but cannot determine how much. This could be mitigated by storing our whitelist as a merkle tree root, so the investor would only prove his address is part of the tree.

> **PoC Limitation**: On-chain payment requires a stablecoin contract on Aztec L2. For PoC, we use off-chain fiat settlement. In production, Aztec's native shielding enables bridging L1 stablecoins (USDC) to private L2 tokens for atomic DvP, and authwit as a way to settle atomicaly and privately.

## Secondary Market: Trading

Peer-to-peer trading via atomic DvP: bond tokens exchanged for payment tokens in a single transaction.

**Simple Transfer** (bond only):

```
transfer_private(to: AztecAddress, amount: u64)
```

- Checks both sender and recipient are whitelisted
- Consumes sender's notes, creates note for recipient

**Atomic DvP** (bond ↔ stablecoin):

Uses the [atomic swap pattern](#appendix-authentication-witness-authwit) with a DvP contract. Both parties pre-authorize the swap via authwit, then either party executes:

1. Seller creates authwit: "DvP can transfer X bonds to Buyer"
2. Buyer creates authwit: "DvP can transfer Y stablecoins to Seller"
3. Execute: DvP verifies both authwits, atomically swaps assets

Both parties must be whitelisted. Trade terms are locked in authwits - neither party can modify after signing.

## Redemption & Maturity

At maturity, bondholders redeem notes for par value. Redemption uses the same [atomic swap pattern](#appendix-authentication-witness-authwit) as secondary trading - bonds exchanged for stablecoins - except bonds are burned instead of transferred.

### Flow

```
┌──────────┐                                    ┌──────────┐
│ Investor │                                    │  Issuer  │
│(has bonds)                                    │(has USDC)│
└────┬─────┘                                    └────┬─────┘
     │                                               │
     │  1. Create authwit:                           │
     │     "Bond contract can burn X of my bonds     │
     │      if I receive Y USDC"                     │
     │───────────────────────────────────────────────┼──┐
     │                                               │  │
     │       (Investor can cancel anytime            │  │
     │        before step 4)                         │  │
     │                                               │  │
     │                        2. Issuer sources      │  │
     │                           liquidity (off-chain)  │
     │                                               │  │
     │                        3. Create authwit:     │  │
     │                           "Bond contract can  │  │
     │                            transfer my Y USDC"│  │
     │                                               │──┼──┐
     │                                               │  │  │
     │                        4. settle_redemption() │  │  │
     │                                               │  │  │
     │              ┌────────────────────────────────┴──┴──┴──┐
     │              │  Bond Contract (atomic):                │
     │              │  • Check maturity date reached          │
     │              │  • Verify investor's authwit            │
     │              │  • Verify issuer's authwit              │
     │              │  • Burn investor's bonds                │
     │              │  • Transfer issuer's USDC → investor    │
     │              │  • Emit nullifiers (consume authwits)   │
     │              └─────────────────────────────────────────┘
     │                                               │
     ▼                                               ▼
 receives USDC                                 bonds redeemed
```

### Why 2-Step?

Real bond economics: issuer receives cash at issuance and uses it (working capital, investment). At maturity, issuer pays from future cash flows - they don't have redemption capital locked upfront.

The investor's authwit is a **signed redemption request** that:

- Proves investor intent to redeem
- Locks exact settlement terms (amounts, stablecoin contract)
- Gives issuer time to source liquidity
- Remains cancellable until issuer settles

### Security Properties

- **Investor protected**: Authwit commits to exact terms. Issuer can only settle with matching parameters.
- **No overcollateralization**: Issuer funds not locked until settlement execution.
- **Atomic**: Either both legs execute (burn + payment) or neither does.
- **Cancellable**: Investor can revoke pending authwit before issuer settles.
- **Replay-safe**: Nonce prevents reuse of authwit.

### Privacy

- Settlement amount hidden (private function)
- Only issuer and investor know redemption details
- Total supply unchanged (bonds burned, not transferred)

> **PoC Limitation**: Full authwit redemption requires a stablecoin contract integration. For PoC, we implement a simple `redeem(amount)` where investor burns their own bonds directly and stablecoin settlement happens off-chain via fiat.

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

**Authwit Security**:

- Authwit hash includes: caller, contract, function selector, all arguments, chain ID
- Investor commits to exact settlement terms - issuer cannot modify
- Nonce prevents replay attacks
- Cancellation possible before execution

## Terminology

Privacy primitives (notes, nullifiers, commitments, Merkle trees) are defined and implemented by Aztec. See [Aztec documentation](https://docs.aztec.network/) for protocol-level details.

**Bond-specific terms:**

- **Total Supply**: Fixed amount of bonds issued at initialization. Never changes.
- **Distribution**: Transfer of bonds from issuer's pool to investor (primary market).
- **Authwit**: Authentication witness - cryptographic authorization for a specific action. See [Appendix](#appendix-authentication-witness-authwit).
- **Settlement**: Atomic exchange of bonds for stablecoins at redemption.
- **Burn**: Destruction of bond notes (nullifiers published, no new notes created).

---

## Appendix: Authentication Witness (Authwit)

Authwit is Aztec's pattern for authorizing contracts to act on behalf of users. It replaces EVM's `approve` pattern with action-specific authorization that works with private state. See [Aztec authwit documentation](https://docs.aztec.network/developers/docs/foundational-topics/advanced/authwit).

### Why Not ERC20 Approve?

Traditional `approve` doesn't work in Aztec's private model:

| Aspect | ERC20 Approve | Authwit |
|--------|---------------|---------|
| Scope | Blanket allowance (any amount up to limit) | Specific action (exact parameters) |
| Private state | Cannot access notes (only owner knows secrets) | Works via owner-initiated tx |
| Revocation | Requires on-chain tx to set allowance to 0 | Emit nullifier |
| Replay | Allowance persists until changed | Single-use (nullified after use) |

Even with authorization, spending private notes requires the owner's participation - they must initiate the transaction so their PXE can provide note secrets to the proving system.

### When Authwit is Required

| Scenario | `msg_sender()` | Authwit? |
|----------|----------------|----------|
| Alice calls `bond.transfer(Bob, 100)` | Alice | No |
| Alice calls `dvp.execute()` → DvP calls `bond.transfer_from(Alice, Bob, 100)` | DvP contract | **Yes** |

When a contract (not the user) calls `transfer_from`, it needs proof the user authorized that specific action.

### Authwit Hash Structure

An authwit commits to exact parameters via a two-level hash:

```
Inner hash:   H(caller, function_selector, args_hash)
Message hash: H(consumer_contract, chain_id, version, inner_hash)
```

This binds authorization to the specific contract, function, arguments, and chain.

### Private Authwit Verification Flow

When a DvP contract calls `bond.transfer_from(Alice, Bob, 100)`:

```
┌───────────┐         ┌───────────┐         ┌───────────┐         ┌───────────┐
│   Alice   │         │    DvP    │         │   Bond    │         │  Alice's  │
│   (PXE)   │         │ Contract  │         │ Contract  │         │  Account  │
└─────┬─────┘         └─────┬─────┘         └─────┬─────┘         └─────┬─────┘
      │                     │                     │                     │
      │ 1. Alice initiates  │                     │                     │
      │    dvp.execute()    │                     │                     │
      │────────────────────>│                     │                     │
      │                     │                     │                     │
      │                     │ 2. transfer_from    │                     │
      │                     │    (Alice,Bob,100)  │                     │
      │                     │────────────────────>│                     │
      │                     │                     │                     │
      │                     │                     │ 3. Static call:     │
      │                     │                     │    verify_authwit   │
      │                     │                     │────────────────────>│
      │                     │                     │                     │
      │                     │                     │         4. Oracle fetches witness
      │                     │                     │            from Alice's PXE
      │<────────────────────┼─────────────────────┼─────────────────────│
      │                     │                     │                     │
      │ 5. Return witness   │                     │                     │
      │────────────────────>│─────────────────────┼────────────────────>│
      │                     │                     │                     │
      │                     │                     │         6. Validate witness
      │                     │                     │            matches action hash
      │                     │                     │                     │
      │                     │                     │ 7. Return: valid    │
      │                     │                     │<────────────────────│
      │                     │                     │                     │
      │                     │                     │ 8. Execute transfer │
      │                     │                     │    + emit nullifier │
      │                     │                     │    (prevents replay)│
      └─────────────────────┴─────────────────────┴─────────────────────┘
```

**Key points:**
- Alice must initiate the transaction (her PXE holds the witness and note secrets)
- Static call to account contract prevents re-entrancy during verification
- Bond contract (consumer) emits nullifier, not the account contract
- Nullifier prevents the same authwit from being used twice

### Atomic Swap Pattern

Both secondary market DvP and redemption use the same pattern:

```
┌─────────────┐                              ┌─────────────┐
│   Party A   │                              │   Party B   │
│   (has X)   │                              │   (has Y)   │
└──────┬──────┘                              └──────┬──────┘
       │                                            │
       │  1. Create authwit:                        │
       │     "Swap can transfer my X to B"          │
       │───────────────────┐                        │
       │                   ▼                        │
       │          ┌────────────────┐                │
       │          │  Swap Contract │                │
       │          │                │<───────────────│
       │          │                │  2. Create authwit:
       │          └───────┬────────┘     "Swap can transfer my Y to A"
       │                  │                         │
       │                  │  3. Either party        │
       │                  │     calls execute()     │
       │                  ▼                         │
       │          ┌────────────────┐                │
       │          │  Atomically:   │                │
       │          │  • Verify A's authwit           │
       │          │  • Verify B's authwit           │
       │          │  • X: A → B (or burn)           │
       │          │  • Y: B → A                     │
       │          │  • Emit nullifiers              │
       │          └────────────────┘                │
       │                                            │
       ▼                                            ▼
   receives Y                                  receives X
```

| Use Case | Party A | Party B | Asset X | Asset Y | X Outcome |
|----------|---------|---------|---------|---------|-----------|
| Secondary Market | Seller | Buyer | Bonds | Stablecoins | Transfer |
| Redemption | Investor | Issuer | Bonds | Stablecoins | Burn |

### Cancellation

Users can cancel unused authwits by directly emitting the nullifier (without executing the authorized action). This invalidates the authwit permanently.
