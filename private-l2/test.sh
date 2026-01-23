#!/bin/bash
set -e

CONTRACT_PATH=~/confidential-bond-poc/private-l2/private-bonds-contract/target/private_bonds-PrivateBonds.json

# ==============================================================================
# PRIVATE BONDS ON AZTEC - INVESTOR/ISSUER STORY
# ==============================================================================
#
# This script demonstrates a complete bond lifecycle with maximum privacy.
#
# ACTORS:
#   - Issuer (test0): The bond issuer - a financial institution
#   - Investor A (test1): An institutional investor
#   - Investor B (test2): Another institutional investor
#
# STORY OVERVIEW:
#   1. Issuer creates bond with FIXED $1M supply (all minted to issuer privately)
#   2. Investor A and Investor B complete KYC and get whitelisted
#   3. Issuer distributes $500k to Investor A (private transfer from pool)
#   4. Issuer distributes $300k to Investor B (private transfer from pool)
#   5. Investor A sells $100k to Investor B on secondary market (private transfer)
#   6. At maturity, both redeem their bonds (private transfer back to issuer)
#
# PRIVACY GUARANTEES:
#   - WHO holds bonds: Public (via whitelist - required for KYC/AML)
#   - HOW MUCH each holds: PRIVATE (encrypted in notes)
#   - WHO trades with whom: PRIVATE (whitelist check reveals recipient only)
#   - Total supply: Public but FIXED (never changes - no information leaked)
#
# KEY INSIGHT: Total supply is set once at initialization and NEVER changes.
# This prevents observers from deducing transaction amounts.
#
# ==============================================================================

echo "=============================================================="
echo "  PRIVATE BONDS ON AZTEC L2 - FULL LIFECYCLE DEMO"
echo "=============================================================="
echo ""
echo "Actors:"
echo "  - Issuer (test0): Bond Issuer"
echo "  - Investor A (test1): Institutional Investor"
echo "  - Investor B (test2): Institutional Investor"
echo ""

# ==============================================================================
# PHASE 1: CONTRACT DEPLOYMENT & ISSUANCE
# ==============================================================================
#
# Issuer deploys the PrivateBonds contract with:
#   - total_supply: 1,000,000 (FIXED forever)
#   - maturity: 0 (for demo - production would be future timestamp)
#
# CRITICAL: The ENTIRE supply is minted to issuer's PRIVATE balance at deploy.
# This is the ONLY minting that ever happens. Total supply NEVER changes.
#
# PRIVACY:
#   - Total supply: Public but FIXED (reveals nothing about individual trades)
#   - Maturity date: Public
#   - Issuer address: Public
#   - Issuer's balance: PRIVATE (nobody knows how much issuer still holds)
#
# ==============================================================================

echo "=============================================================="
echo "PHASE 1: CONTRACT DEPLOYMENT & ISSUANCE"
echo "=============================================================="
echo ""
echo "Issuer deploys PrivateBonds contract..."
echo "  - Total supply: 1,000,000 (FIXED forever)"
echo "  - Maturity: 0 (immediate for demo)"
echo ""

aztec-wallet deploy $CONTRACT_PATH --from accounts:test0 -a privatebonds --init initialize --args 1000000 0

echo ""
echo "Contract deployed. Issuer holds 1,000,000 bonds privately."
echo ""

echo "Verifying issuer's private balance:"
aztec-wallet simulate private_balance_of --from accounts:test0 --contract-address contracts:privatebonds --args accounts:test0
echo ""

# ==============================================================================
# PHASE 2: KYC & WHITELISTING
# ==============================================================================
#
# Before investors can receive bonds, they must complete KYC off-chain
# and be added to the on-chain whitelist by the issuer.
#
# PRIVACY: Public function - everyone sees:
#   - Which addresses are whitelisted
#   - When they were added
#
# This is a regulatory requirement for securities.
# ==============================================================================

echo "=============================================================="
echo "PHASE 2: KYC & WHITELISTING"
echo "=============================================================="
echo ""
echo "Investor A and Investor B complete KYC verification off-chain..."
echo "(In production: ID verification, accreditation checks, AML screening)"
echo ""
echo "Issuer adds verified investors to whitelist..."
echo ""
echo "PRIVACY: Whitelist is PUBLIC - everyone can see who is authorized"
echo "         to hold bonds. This is required for regulatory compliance."
echo ""

echo "Adding Investor A to whitelist..."
aztec-wallet send add_to_whitelist --from accounts:test0 --contract-address contracts:privatebonds --args accounts:test1

echo ""
echo "Adding Investor B to whitelist..."
aztec-wallet send add_to_whitelist --from accounts:test0 --contract-address contracts:privatebonds --args accounts:test2

echo ""
echo "Both investors are now authorized to hold bonds."
echo ""

# ==============================================================================
# PHASE 3: PRIMARY MARKET - PRIVATE DISTRIBUTION
# ==============================================================================
#
# Investors purchase bonds from the issuer. This is NOT minting - it's
# a private transfer from issuer's pool to investor.
#
# We use distribute_private() which transfers from issuer to investor.
#
# PRIVACY:
#   - WHO bought: Revealed via whitelist check (acceptable for KYC)
#   - HOW MUCH: PRIVATE - encrypted in notes
#   - Total supply: UNCHANGED - no information leaked!
#
# This is the key improvement: observers see that Investor A received
# SOME bonds, but cannot determine how much.
#
# ==============================================================================

echo "=============================================================="
echo "PHASE 3: PRIMARY MARKET - PRIVATE DISTRIBUTION"
echo "=============================================================="
echo ""
echo "Investors wire fiat to Issuer's bank account (off-chain)..."
echo ""
echo "Issuer distributes bonds PRIVATELY from their pool:"
echo "  - Using distribute_private() - a private transfer from issuer"
echo "  - Total supply remains UNCHANGED (1,000,000)"
echo "  - Individual allocations are PRIVATE"
echo ""
echo "PRIVACY: Observers see whitelist checks but NOT the amounts."
echo "         Total supply stays at 1M - no leakage!"
echo ""

echo "Investor A purchases bonds (amount HIDDEN)..."
echo "  [Off-chain: Investor A wired payment for their allocation]"
aztec-wallet send distribute_private --from accounts:test0 --contract-address contracts:privatebonds --args accounts:test1 500000

echo ""
echo "Investor B purchases bonds (amount HIDDEN)..."
echo "  [Off-chain: Investor B wired payment for their allocation]"
aztec-wallet send distribute_private --from accounts:test0 --contract-address contracts:privatebonds --args accounts:test2 300000

echo ""
echo "Primary distribution complete."
echo ""
echo "What the PUBLIC can see:"
echo "  - Investor A is a bondholder (whitelisted)"
echo "  - Investor B is a bondholder (whitelisted)"
echo "  - Total supply: 1,000,000 (UNCHANGED!)"
echo ""
echo "What the PUBLIC CANNOT see:"
echo "  - Investor A's allocation: ??? (encrypted)"
echo "  - Investor B's allocation: ??? (encrypted)"
echo "  - Issuer's remaining pool: ??? (encrypted)"
echo ""

echo "Verifying balances (private - only visible to account owner):"
echo ""
echo "Issuer's remaining private balance (should be 200,000):"
aztec-wallet simulate private_balance_of --from accounts:test0 --contract-address contracts:privatebonds --args accounts:test0
echo ""
echo "Investor A's private balance (should be 500,000):"
aztec-wallet simulate private_balance_of --from accounts:test1 --contract-address contracts:privatebonds --args accounts:test1
echo ""
echo "Investor B's private balance (should be 300,000):"
aztec-wallet simulate private_balance_of --from accounts:test2 --contract-address contracts:privatebonds --args accounts:test2
echo ""

# ==============================================================================
# PHASE 4: SECONDARY MARKET - PRIVATE TRADING
# ==============================================================================
#
# Investor A wants to sell some bonds to Investor B.
# This is a peer-to-peer private transfer.
#
# PRIVACY:
#   - Sender: PRIVATE (hidden in note consumption)
#   - Recipient: Revealed via whitelist check
#   - Amount: PRIVATE (encrypted)
#   - Total supply: UNCHANGED - no leakage!
#
# ==============================================================================

echo "=============================================================="
echo "PHASE 4: SECONDARY MARKET - PRIVATE TRADING"
echo "=============================================================="
echo ""
echo "Investor A wants to sell 100,000 bonds to Investor B."
echo ""
echo "Trade agreement (off-chain or via atomic DvP):"
echo "  - Investor A sells: 100,000 bonds"
echo "  - Investor B pays: (price agreed off-chain or via payment token)"
echo ""
echo "Executing private transfer..."
echo ""
echo "PRIVACY: This is the MOST PRIVATE operation:"
echo "  - Sender: HIDDEN"
echo "  - Recipient: Revealed via whitelist check"
echo "  - Amount: HIDDEN"
echo "  - Total supply: UNCHANGED (still 1,000,000)"
echo ""

aztec-wallet send transfer_private --from accounts:test1 --contract-address contracts:privatebonds --args accounts:test2 100000

echo ""
echo "Trade executed."
echo ""
echo "What happened on-chain (visible to observers):"
echo "  - Some nullifiers published (old notes consumed)"
echo "  - Some note commitments added (new notes created)"
echo "  - Investor B's address was checked against whitelist"
echo "  - Total supply: STILL 1,000,000 (no change!)"
echo ""
echo "What observers CANNOT determine:"
echo "  - Who sent (could be anyone with notes)"
echo "  - How much was transferred"
echo "  - New balances"
echo ""

echo "Updated balances (private - only visible to owners):"
echo ""
echo "Investor A's private balance (should be 400,000):"
aztec-wallet simulate private_balance_of --from accounts:test1 --contract-address contracts:privatebonds --args accounts:test1
echo ""
echo "Investor B's private balance (should be 400,000):"
aztec-wallet simulate private_balance_of --from accounts:test2 --contract-address contracts:privatebonds --args accounts:test2
echo ""

# ==============================================================================
# PHASE 5: REDEMPTION AT MATURITY
# ==============================================================================
#
# At maturity, investors redeem bonds by transferring them back to the issuer.
# This is NOT burning - it's returning tokens to issuer's pool.
#
# PRIVACY:
#   - Redemption uses redeem_private()
#   - Investor's notes are consumed (private)
#   - Amount credited to issuer (for payment tracking)
#   - Total supply: UNCHANGED (tokens return to issuer, not destroyed)
#
# ==============================================================================

echo "=============================================================="
echo "PHASE 5: REDEMPTION AT MATURITY"
echo "=============================================================="
echo ""
echo "Bond has reached maturity. Investors redeem their holdings."
echo ""
echo "Using redeem_private():"
echo "  - Investor's notes are consumed (private)"
echo "  - Issuer receives the tokens back"
echo "  - Total supply: UNCHANGED (still 1,000,000)"
echo ""

echo "Investor A redeems their bonds..."
aztec-wallet send redeem_private --from accounts:test1 --contract-address contracts:privatebonds --args 400000

echo ""
echo "Investor B redeems their bonds..."
aztec-wallet send redeem_private --from accounts:test2 --contract-address contracts:privatebonds --args 400000

echo ""
echo "All bonds redeemed."
echo ""
echo "[Off-chain: Issuer initiates wire transfers to investors for par value]"
echo ""

# ==============================================================================
# FINAL STATE
# ==============================================================================

echo "=============================================================="
echo "FINAL STATE"
echo "=============================================================="
echo ""

echo "Investor A's final balances:"
echo "  Public: $(aztec-wallet simulate public_balance_of --from accounts:test0 --contract-address contracts:privatebonds --args accounts:test1)"
echo "  Private: $(aztec-wallet simulate private_balance_of --from accounts:test1 --contract-address contracts:privatebonds --args accounts:test1)"
echo ""

echo "Investor B's final balances:"
echo "  Public: $(aztec-wallet simulate public_balance_of --from accounts:test0 --contract-address contracts:privatebonds --args accounts:test2)"
echo "  Private: $(aztec-wallet simulate private_balance_of --from accounts:test2 --contract-address contracts:privatebonds --args accounts:test2)"
echo ""

echo "Issuer's balances (received redemptions):"
echo "  Public: $(aztec-wallet simulate public_balance_of --from accounts:test0 --contract-address contracts:privatebonds --args accounts:test0)"
echo "  Private: $(aztec-wallet simulate private_balance_of --from accounts:test0 --contract-address contracts:privatebonds --args accounts:test0)"
echo ""

echo "Total supply (should still be 1,000,000):"
aztec-wallet simulate get_total_supply --from accounts:test0 --contract-address contracts:privatebonds
echo ""

# ==============================================================================
# PRIVACY SUMMARY
# ==============================================================================

echo "=============================================================="
echo "PRIVACY SUMMARY"
echo "=============================================================="
echo ""
echo "Throughout the bond lifecycle, an outside observer could see:"
echo ""
echo "  [PUBLIC - By Design]"
echo "  - Bond terms (total supply: 1M fixed, maturity date)"
echo "  - Who is authorized to hold bonds (whitelist)"
echo "  - That transactions occurred (nullifiers + commitments)"
echo ""
echo "  [HIDDEN - Protected]"
echo "  - Individual investor allocations"
echo "  - Trade amounts between parties"
echo "  - Running balances"
echo ""
echo "KEY PRIVACY IMPROVEMENT:"
echo "  Total supply is FIXED at 1,000,000 and NEVER changes."
echo "  This prevents observers from deducing transaction amounts"
echo "  by watching supply changes."
echo ""
echo "  Old model: mint 500k -> supply 0->500k (leaks allocation)"
echo "  New model: distribute 500k -> supply stays 1M (no leak)"
echo ""
echo "=============================================================="
echo "DEMO COMPLETE"
echo "=============================================================="
