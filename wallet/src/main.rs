use chrono::Utc;
use clap::{Parser, Subcommand};
use rand::Rng;
use std::error::Error;
use std::fs;

use alloy::{
    primitives::{address, Bytes},
    providers::ProviderBuilder,
    signers::local::PrivateKeySigner,
    sol,
};

mod config;
mod keys;
mod merkle;
mod notes;
mod prover;
mod utils;

use config::{PRIVATE_BOND_ADDRESS, RPC_URL};
use prover::{build_joinsplit_witness, generate_proof, CircuitNote};
use utils::{fr_to_bytes32, format_date, load_bond, load_wallet, TreeState, Bond, Wallet, ensure_data_dir, DATA_DIR};

use crate::keys::ShieldedKeys;

// Contract ABI - minimal interface for the functions we use
sol!(
    #[sol(rpc)]
    PrivateBond,
    "abi/PrivateBond.abi.json"
);

#[derive(Parser)]
#[command(name = "Bond Wallet")]
#[command(about = "CLI wallet for zero-coupon bond protocol", long_about = None)]
struct Cli {
    #[arg(long, default_value = "wallet")]
    wallet: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize issuer wallet: generate keys and create initial bond tranche
    Onboard,

    /// Register as a buyer: generate keys only (no bond creation)
    Register,

    /// Buy bond from issuer (splits issuer's note)
    Buy {
        /// Amount to buy
        #[arg(long)]
        value: u64,
        /// Path to issuer's source note (being split)
        #[arg(long)]
        source_note: String,
        /// Path to issuer's wallet (for signing)
        #[arg(long)]
        issuer_wallet: String,
    },

    /// Trade: swap two bonds P2P
    Trade {
        #[arg(long)]
        bond_a: String,
        #[arg(long)]
        bond_b: String,
    },

    /// Redeem: burn bond at maturity
    Redeem {
        #[arg(long)]
        bond: String,
    },

    /// Info: display bond details
    Info {
        #[arg(long)]
        bond: String,
    },
}

fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();

    // Run async commands
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        match cli.command {
            Commands::Onboard => onboard(&cli.wallet).await,
            Commands::Register => register(&cli.wallet),
            Commands::Buy { value, source_note, issuer_wallet } => {
                buy(&cli.wallet, value, &source_note, &issuer_wallet).await
            }
            Commands::Trade { bond_a, bond_b } => trade(&cli.wallet, &bond_a, &bond_b).await,
            Commands::Redeem { bond } => redeem(&cli.wallet, &bond).await,
            Commands::Info { bond } => info(&bond),
        }
    });

    Ok(())
}

async fn onboard(wallet_name: &str) {
    println!("\n🔐 Issuer Onboarding: Creating initial bond tranche...");

    // Ensure data directory exists
    ensure_data_dir();

    // Generate keys for issuer
    let keys = ShieldedKeys::generate();

    let wallet = Wallet {
        keys: keys.clone(),
        created_at: Utc::now().to_rfc3339(),
    };

    // Save wallet
    let filename = format!("{}/{}.json", DATA_DIR, wallet_name);
    match fs::write(&filename, serde_json::to_string_pretty(&wallet).unwrap()) {
        Ok(_) => {
            println!("✅ Issuer wallet created!");
            println!("   Saved to: {}", filename);
        }
        Err(e) => {
            println!("❌ Error: {}", e);
            return;
        }
    }

    // Create initial Global Note commitment for the bond tranche
    // Example: $100M bond tranche maturing 2030-01-01
    let global_value = 100_000_000u64; // $100M in smallest units
    let maturity_date = 1893456000u64; // 2030-01-01

    // Generate random salt
    let mut rng = rand::thread_rng();
    let salt = rng.gen::<u64>();

    // Get owner as Fr (proper field element)
    let owner_fr = keys.public_spending_key();

    // Create CircuitNote for commitment computation (matches circuit exactly)
    let global_note = CircuitNote {
        value: global_value,
        salt,
        owner: owner_fr.clone(),
        asset_id: 1,
        maturity_date,
    };

    // Compute commitment using CircuitNote.commitment() - matches circuit's note_commit
    let commitment = global_note.commitment();
    println!("\n📊 Global Note (Bond Tranche):");
    println!("   Value:     {} (units)", global_value);
    println!(
        "   Maturity:  {} ({})",
        maturity_date,
        format_date(maturity_date)
    );
    println!("   Commitment: {}", commitment);

    // Initialize a signer with a private key
    let signer: PrivateKeySigner =
        "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"
            .parse()
            .expect("Failed to parse private key");

    // Instantiate a provider with the signer and a local anvil node
    let provider = ProviderBuilder::new()
        .wallet(signer)
        .connect("http://127.0.0.1:8545")
        .await
        .expect("Failed to configure provider");

    let private_bond_address = address!("0xdc64a140aa3e981100a9beca4e685f962f0cf6c9");
    let private_bond = PrivateBond::new(private_bond_address, provider.clone());

    let commitment_bytes_vec = commitment.to_string().into_bytes();
    // Pad or truncate to exactly 32 bytes
    let mut commitment_array = [0u8; 32];
    let len = commitment_bytes_vec.len().min(32);
    commitment_array[..len].copy_from_slice(&commitment_bytes_vec[..len]);

    let mint_batch_tx = private_bond
        .mintBatch(vec![alloy::primitives::FixedBytes::<32>::from(
            commitment_array,
        )])
        .send()
        .await
        .expect("Failed to call mintBatch");

    let mint_batch_receipt = mint_batch_tx
        .get_receipt()
        .await
        .expect("Failed to send note batch");

    println!("   Mint transaction sent:     {:#?}", mint_batch_receipt);

    // Add commitment to the global tree state
    let mut tree_state = TreeState::load();
    let leaf_index = tree_state.add_commitment(commitment);
    println!("   Added real note to merkle tree at index: {}", leaf_index);
    
    // Also add the dummy note (value=0, salt=0, same owner) to the tree
    // This is required because the circuit verifies merkle proofs for both inputs
    let dummy_note = CircuitNote {
        value: 0,
        salt: 0,
        owner: owner_fr.clone(),
        asset_id: 1,
        maturity_date,
    };
    let dummy_commitment = dummy_note.commitment();
    let dummy_index = tree_state.add_commitment(dummy_commitment);
    println!("   Added dummy note to merkle tree at index: {}", dummy_index);

    // Save the global note as initial bond
    let bond = Bond {
        commitment: format!("{}", commitment),
        nullifier: "N/A (Global Note)".to_string(),
        value: global_value,
        salt,
        owner: keys.public_spending_key_hex,
        asset_id: 1,
        maturity_date,
        created_at: Utc::now().to_rfc3339(),
    };

    let filename = format!("{}/global_note_tranche.json", DATA_DIR);
    match fs::write(&filename, serde_json::to_string_pretty(&bond).unwrap()) {
        Ok(_) => println!("\n✅ Global note saved to: {}", filename),
        Err(e) => println!("❌ Error saving: {}", e),
    }
}

fn register(wallet_name: &str) {
    println!("\n📋 Registering new wallet...");

    // Ensure data directory exists
    ensure_data_dir();

    // Check if wallet already exists
    if load_wallet(wallet_name).is_some() {
        println!("⚠️  Wallet '{}' already exists", wallet_name);
        return;
    }

    // Generate keys
    let keys = ShieldedKeys::generate();

    let wallet = Wallet {
        keys: keys.clone(),
        created_at: Utc::now().to_rfc3339(),
    };

    // Save wallet
    let filename = format!("{}/{}.json", DATA_DIR, wallet_name);
    match fs::write(&filename, serde_json::to_string_pretty(&wallet).unwrap()) {
        Ok(_) => {
            println!("✅ Wallet created!");
            println!("   Saved to: {}", filename);
            println!("   Public key: {}", keys.public_spending_key_hex);
        }
        Err(e) => {
            println!("❌ Error: {}", e);
        }
    }
}

async fn buy(buyer_wallet_name: &str, buy_value: u64, source_note_path: &str, issuer_wallet_path: &str) {
    println!("\n💳 Buying bond from issuer...");
    println!("   Buy amount: {}", buy_value);

    // 1. Load buyer's wallet (to get buyer's public key)
    let buyer_wallet = match load_wallet(buyer_wallet_name) {
        Some(w) => w,
        None => {
            println!("❌ Buyer wallet '{}' not found. Run 'onboard' first.", buyer_wallet_name);
            return;
        }
    };

    // 2. Load issuer's wallet (for private key to sign nullifier)
    let issuer_wallet = match load_wallet(issuer_wallet_path) {
        Some(w) => w,
        None => {
            println!("❌ Issuer wallet '{}' not found.", issuer_wallet_path);
            return;
        }
    };

    // 3. Load source note (issuer's note being split)
    let source_bond = match load_bond(source_note_path) {
        Some(b) => b,
        None => {
            println!("❌ Source note '{}' not found.", source_note_path);
            return;
        }
    };

    // Validate: buy value must be less than source note value
    if buy_value >= source_bond.value {
        println!("❌ Buy value ({}) must be less than source note value ({}).", buy_value, source_bond.value);
        return;
    }

    let change_value = source_bond.value - buy_value;
    println!("   Source note: {} (value={})", source_note_path, source_bond.value);
    println!("   Change to issuer: {}", change_value);
    println!("   Maturity: {} ({})", source_bond.maturity_date, format_date(source_bond.maturity_date));

    // 4. Create INPUT note (issuer's note being consumed)
    let issuer_owner_fr = issuer_wallet.keys.public_spending_key();

    let input_note = CircuitNote {
        value: source_bond.value,
        salt: source_bond.salt,
        owner: issuer_owner_fr.clone(),
        asset_id: source_bond.asset_id,
        maturity_date: source_bond.maturity_date,
    };

    // 5. Compute nullifiers for input notes (issuer signs)
    let input_nullifier_fr = issuer_wallet.keys.sign_nullifier(source_bond.salt);
    let dummy_nullifier_fr = issuer_wallet.keys.sign_nullifier(0); // Dummy note has salt=0
    
    // Debug: verify nullifier computation uses the same private key
    let private_key_debug = issuer_wallet.keys.get_private_spending_key();
    println!("   DEBUG: salt={}, private_key={}", source_bond.salt, private_key_debug);
    println!("   DEBUG: computed nullifier={}", input_nullifier_fr);

    // 6. Create OUTPUT notes
    let mut rng = rand::thread_rng();
    
    // Output 1: Buyer's note
    let buyer_salt = rng.gen::<u64>();
    let buyer_owner_fr = buyer_wallet.keys.public_spending_key();

    let buyer_note = CircuitNote {
        value: buy_value,
        salt: buyer_salt,
        owner: buyer_owner_fr.clone(),
        asset_id: source_bond.asset_id,
        maturity_date: source_bond.maturity_date,
    };

    // Output 2: Issuer's change note
    let change_salt = rng.gen::<u64>();
    let change_note = CircuitNote {
        value: change_value,
        salt: change_salt,
        owner: issuer_owner_fr.clone(),
        asset_id: source_bond.asset_id,
        maturity_date: source_bond.maturity_date,
    };

    // 7. Compute output commitments using CircuitNote.commitment() - matches circuit
    let buyer_commitment_fr = buyer_note.commitment();
    let change_commitment_fr = change_note.commitment();

    println!("\n📊 JoinSplit Summary:");
    println!("   INPUT:  value={}, nullifier={}", source_bond.value, input_nullifier_fr);
    println!("   OUTPUT1 (buyer):  value={}, commitment={}", buy_value, buyer_commitment_fr);
    println!("   OUTPUT2 (change): value={}, commitment={}", change_value, change_commitment_fr);

    // 8. Build merkle tree and generate proofs for both input notes
    let mut tree_state = TreeState::load();
    
    // Find the source note's commitment in the tree (should be at index 0)
    let source_commitment_str = &source_bond.commitment;
    let real_note_index = match tree_state.find_commitment(source_commitment_str) {
        Some(idx) => idx,
        None => {
            println!("❌ Source note commitment not found in tree state!");
            println!("   Commitment: {}", source_commitment_str);
            println!("   ℹ️  Make sure the issuer ran 'onboard' to register the initial note.");
            return;
        }
    };
    
    // Create dummy note (value=0, salt=0) and find its commitment (should be at index 1)
    let dummy_note = CircuitNote {
        value: 0,
        salt: 0,
        owner: issuer_owner_fr.clone(),
        asset_id: source_bond.asset_id,
        maturity_date: source_bond.maturity_date,
    };
    let dummy_commitment = dummy_note.commitment();
    let dummy_commitment_str = format!("{}", dummy_commitment);
    
    let dummy_note_index = match tree_state.find_commitment(&dummy_commitment_str) {
        Some(idx) => idx,
        None => {
            println!("❌ Dummy note commitment not found in tree state!");
            println!("   Commitment: {}", dummy_commitment_str);
            println!("   ℹ️  The issuer's onboard should have added both real and dummy notes.");
            return;
        }
    };
    
    println!("   Real note at tree index: {}", real_note_index);
    println!("   Dummy note at tree index: {}", dummy_note_index);
    
    // Build the merkle tree and generate proofs for BOTH notes
    let tree = tree_state.build_tree();
    let merkle_root = tree.root();
    let real_note_path = tree.generate_proof(real_note_index);
    let dummy_note_path = tree.generate_proof(dummy_note_index);
    
    println!("   Merkle root: {}", merkle_root);
    println!("   Real note path_indices: {:?}", real_note_path.indices);
    println!("   Dummy note path_indices: {:?}", dummy_note_path.indices);

    // Get issuer's private spending key
    let private_key_fr = issuer_wallet.keys.get_private_spending_key();

    // Build JoinSplit witness: 2 inputs (real + dummy) -> 2 outputs (buyer + change)
    let witness = build_joinsplit_witness(
        merkle_root,
        input_note,
        real_note_path,
        input_nullifier_fr,
        dummy_note,
        dummy_note_path,
        [buyer_note.clone(), change_note.clone()],
        [buyer_commitment_fr.clone(), change_commitment_fr.clone()],
        private_key_fr,
    );

    // 9. Write Prover.toml
    let circuit_dir = "../circuits";
    match witness.write_prover_toml(circuit_dir) {
        Ok(_) => println!("\n✅ Witness written to {}/Prover.toml", circuit_dir),
        Err(e) => {
            println!("❌ Failed to write witness: {}", e);
            return;
        }
    }

    // 10. Generate proof
    println!("\n🔐 Generating ZK proof...");
    let proof_result = generate_proof(circuit_dir, "circuits").await;
    let proof_path = match &proof_result {
        Ok(path) => {
            println!("   ✅ Proof saved to: {}", path);
            Some(path.clone())
        }
        Err(e) => {
            println!("   ⚠️  Proof generation failed: {}", e);
            println!("   ℹ️  You can run manually:");
            println!("      cd {} && nargo execute circuits && bb prove -b ./target/circuits.json -w ./target/circuits -o ./target", circuit_dir);
            None
        }
    };

    // 11. Call contract transfer() with proof
    if let Some(ref proof_file) = proof_path {
        println!("\n📡 Calling contract transfer()...");
        
        // Read proof bytes
        let proof_bytes = match fs::read(proof_file) {
            Ok(bytes) => bytes,
            Err(e) => {
                println!("   ❌ Failed to read proof file: {}", e);
                return;
            }
        };

        // Convert Fr values to bytes32
        let root_bytes = fr_to_bytes32(&merkle_root);
        let nullifier0_bytes = fr_to_bytes32(&input_nullifier_fr);
        let nullifier1_bytes = fr_to_bytes32(&dummy_nullifier_fr);
        let commitment0_bytes = fr_to_bytes32(&buyer_commitment_fr);
        let commitment1_bytes = fr_to_bytes32(&change_commitment_fr);

        // Setup provider with signer (use anvil's first account for now)
        let signer: PrivateKeySigner = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"
            .parse()
            .expect("valid private key");
        let provider = ProviderBuilder::new()
            .wallet(signer)
            .connect(RPC_URL).await.expect("Failed to configure provider");
        

        let contract_address = PRIVATE_BOND_ADDRESS.parse().expect("valid contract address");
        let contract = PrivateBond::new(contract_address, provider);

        // Call transfer()
        match contract
            .transfer(
                Bytes::from(proof_bytes),
                root_bytes,
                [nullifier0_bytes, nullifier1_bytes],
                [commitment0_bytes, commitment1_bytes],
            )
            .send()
            .await
        {
            Ok(pending) => {
                match pending.watch().await {
                    Ok(tx_hash) => {
                        println!("   ✅ Transaction confirmed: {:?}", tx_hash);
                    }
                    Err(e) => {
                        println!("   ⚠️  Transaction pending but watch failed: {}", e);
                    }
                }
            }
            Err(e) => {
                println!("   ❌ Contract call failed: {}", e);
                println!("   ℹ️  Make sure anvil is running and contract is deployed");
            }
        }
    }

    // 12. Save buyer's bond
    let buyer_bond = Bond {
        commitment: format!("{}", buyer_commitment_fr),
        nullifier: format!("{}", buyer_wallet.keys.sign_nullifier(buyer_salt)),
        value: buy_value,
        salt: buyer_salt,
        owner: buyer_wallet.keys.public_spending_key_hex.clone(),
        asset_id: source_bond.asset_id,
        maturity_date: source_bond.maturity_date,
        created_at: Utc::now().to_rfc3339(),
    };

    let buyer_filename = format!("{}/bond_{}_{}.json", DATA_DIR, buyer_wallet_name, &format!("{:016x}", buyer_salt)[..8]);
    match fs::write(&buyer_filename, serde_json::to_string_pretty(&buyer_bond).unwrap()) {
        Ok(_) => println!("\n✅ Buyer bond saved to: {}", buyer_filename),
        Err(e) => println!("❌ Error saving buyer bond: {}", e),
    }

    // 12. Save issuer's change note (update source)
    let change_bond = Bond {
        commitment: format!("{}", change_commitment_fr),
        nullifier: format!("{}", issuer_wallet.keys.sign_nullifier(change_salt)),
        value: change_value,
        salt: change_salt,
        owner: issuer_wallet.keys.public_spending_key_hex.clone(),
        asset_id: source_bond.asset_id,
        maturity_date: source_bond.maturity_date,
        created_at: Utc::now().to_rfc3339(),
    };

    let change_filename = format!("{}/issuer_change_{}.json", DATA_DIR, &format!("{:016x}", change_salt)[..8]);
    match fs::write(&change_filename, serde_json::to_string_pretty(&change_bond).unwrap()) {
        Ok(_) => println!("✅ Issuer change note saved to: {}", change_filename),
        Err(e) => println!("❌ Error saving change note: {}", e),
    }

    // 14. Add new commitments to tree state (for future transactions)
    tree_state.add_commitment(buyer_commitment_fr.clone());
    tree_state.add_commitment(change_commitment_fr.clone());
    println!("   📝 Added 2 new commitments to merkle tree");
}

async fn trade(_wallet_name: &str, bond_a_path: &str, bond_b_path: &str) {
    println!("\n🔄 Trading bonds...");

    let bond_a = match load_bond(bond_a_path) {
        Some(b) => b,
        None => return,
    };

    let bond_b = match load_bond(bond_b_path) {
        Some(b) => b,
        None => return,
    };

    println!(
        "   Bond A: {} (value: {})",
        &bond_a.commitment[..12],
        bond_a.value
    );
    println!(
        "   Bond B: {} (value: {})",
        &bond_b.commitment[..12],
        bond_b.value
    );

    // Check maturity
    let now = Utc::now().timestamp() as u64;
    if now >= bond_a.maturity_date {
        println!("❌ Bond A at/past maturity - cannot trade");
        return;
    }
    if now >= bond_b.maturity_date {
        println!("❌ Bond B at/past maturity - cannot trade");
        return;
    }

    // Check different nullifiers
    if bond_a.nullifier == bond_b.nullifier {
        println!("❌ Cannot trade: identical nullifiers!");
        return;
    }

    println!("\n✅ Trade valid!");
    println!("   Nullifier A marked spent: {}", bond_a.nullifier);
    println!("   Nullifier B marked spent: {}", bond_b.nullifier);
    println!("   New commitments generated for outputs");

    println!("\n📝 Contract call info:");
    println!("   Function: atomicSwap(proofA, inputsA, proofB, inputsB)");
    println!("   Address:  {}", PRIVATE_BOND_ADDRESS);
    println!("   RPC:      {}", RPC_URL);
    println!("   Status:   ℹ️  Proof generation needed before on-chain call");
}

async fn redeem(_wallet_name: &str, bond_path: &str) {
    println!("\n💰 Redeeming bond...");

    let bond = match load_bond(bond_path) {
        Some(b) => b,
        None => return,
    };

    println!(
        "   Bond: {} (value: {})",
        &bond.commitment[..12],
        bond.value
    );

    // Check maturity
    let now = Utc::now().timestamp() as u64;
    if now < bond.maturity_date {
        let days_left = (bond.maturity_date - now) / 86400;
        println!("❌ Cannot redeem: {} days until maturity", days_left);
        return;
    }

    println!("\n✅ Bond at maturity - ready to redeem!");
    println!("   Nullifier: {}", bond.nullifier);
    println!("   Value: {}", bond.value);

    println!("\n📝 Contract call info:");
    println!("   Function: burn(proof, root, nullifier, outputCommitment, maturityDate, isRedeem)");
    println!("   Address:  {}", PRIVATE_BOND_ADDRESS);
    println!("   RPC:      {}", RPC_URL);
    println!("   Status:   ℹ️  Proof generation needed before on-chain call");
    println!("   Settlement: off-chain cash transfer");
}

fn info(bond_path: &str) {
    println!("\n📊 Bond Information:");

    let bond = match load_bond(bond_path) {
        Some(b) => b,
        None => return,
    };

    println!("   Commitment: {}", bond.commitment);
    println!("   Nullifier:  {}", bond.nullifier);
    println!("   Value:      {}", bond.value);
    println!("   Salt:       {}", bond.salt);
    println!("   Asset ID:   {}", bond.asset_id);
    println!("   Created:    {}", bond.created_at);
    println!("   Maturity:   {}", format_date(bond.maturity_date));

    let now = Utc::now().timestamp() as u64;
    if now >= bond.maturity_date {
        println!("   Status:     🔴 Matured");
    } else {
        let days = (bond.maturity_date - now) / 86400;
        println!("   Status:     🟢 {} days remaining", days);
    }
}
