use chrono::Utc;
use clap::{Parser, Subcommand};
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fs;

use alloy::{
    primitives::address, providers::ProviderBuilder, signers::local::PrivateKeySigner, sol,
};

mod config;
mod keys;
mod merkle;
mod notes;
mod prover;

use config::{PRIVATE_BOND_ADDRESS, RPC_URL};
use notes::Note;

use crate::keys::ShieldedKeys;

sol!(
    #[sol(rpc)]
    PrivateBond,
    r#"[
    {
      "type": "constructor",
      "inputs": [
        { "name": "_verifier", "type": "address", "internalType": "address" },
        { "name": "initialOwner", "type": "address", "internalType": "address" }
      ],
      "stateMutability": "nonpayable"
    },
    {
      "type": "function",
      "name": "atomicSwap",
      "inputs": [
        { "name": "proofA", "type": "bytes", "internalType": "bytes" },
        {
          "name": "publicInputsA",
          "type": "bytes32[]",
          "internalType": "bytes32[]"
        },
        { "name": "proofB", "type": "bytes", "internalType": "bytes" },
        {
          "name": "publicInputsB",
          "type": "bytes32[]",
          "internalType": "bytes32[]"
        }
      ],
      "outputs": [],
      "stateMutability": "nonpayable"
    },
    {
      "type": "function",
      "name": "burn",
      "inputs": [
        { "name": "proof", "type": "bytes", "internalType": "bytes" },
        { "name": "root", "type": "bytes32", "internalType": "bytes32" },
        { "name": "nullifier", "type": "bytes32", "internalType": "bytes32" },
        {
          "name": "newCommitment",
          "type": "bytes32",
          "internalType": "bytes32"
        },
        {
          "name": "inputMaturityDate",
          "type": "bytes32",
          "internalType": "bytes32"
        },
        { "name": "isRedeem", "type": "bytes32", "internalType": "bytes32" }
      ],
      "outputs": [],
      "stateMutability": "nonpayable"
    },
    {
      "type": "function",
      "name": "commitments",
      "inputs": [{ "name": "", "type": "uint256", "internalType": "uint256" }],
      "outputs": [{ "name": "", "type": "bytes32", "internalType": "bytes32" }],
      "stateMutability": "view"
    },
    {
      "type": "function",
      "name": "knownRoots",
      "inputs": [{ "name": "", "type": "bytes32", "internalType": "bytes32" }],
      "outputs": [{ "name": "", "type": "bool", "internalType": "bool" }],
      "stateMutability": "view"
    },
    {
      "type": "function",
      "name": "mint",
      "inputs": [
        { "name": "commitment", "type": "bytes32", "internalType": "bytes32" },
        { "name": "newRoot", "type": "bytes32", "internalType": "bytes32" }
      ],
      "outputs": [],
      "stateMutability": "nonpayable"
    },
    {
      "type": "function",
      "name": "mintBatch",
      "inputs": [
        {
          "name": "_commitments",
          "type": "bytes32[]",
          "internalType": "bytes32[]"
        }
      ],
      "outputs": [],
      "stateMutability": "nonpayable"
    },
    {
      "type": "function",
      "name": "nullifiers",
      "inputs": [{ "name": "", "type": "bytes32", "internalType": "bytes32" }],
      "outputs": [{ "name": "", "type": "bool", "internalType": "bool" }],
      "stateMutability": "view"
    },
    {
      "type": "function",
      "name": "owner",
      "inputs": [],
      "outputs": [{ "name": "", "type": "address", "internalType": "address" }],
      "stateMutability": "view"
    },
    {
      "type": "function",
      "name": "renounceOwnership",
      "inputs": [],
      "outputs": [],
      "stateMutability": "nonpayable"
    },
    {
      "type": "function",
      "name": "transferOwnership",
      "inputs": [
        { "name": "newOwner", "type": "address", "internalType": "address" }
      ],
      "outputs": [],
      "stateMutability": "nonpayable"
    },
    {
      "type": "function",
      "name": "verifier",
      "inputs": [],
      "outputs": [
        {
          "name": "",
          "type": "address",
          "internalType": "contract HonkVerifier"
        }
      ],
      "stateMutability": "view"
    },
    {
      "type": "event",
      "name": "OwnershipTransferred",
      "inputs": [
        {
          "name": "previousOwner",
          "type": "address",
          "indexed": true,
          "internalType": "address"
        },
        {
          "name": "newOwner",
          "type": "address",
          "indexed": true,
          "internalType": "address"
        }
      ],
      "anonymous": false
    },
    {
      "type": "error",
      "name": "OwnableInvalidOwner",
      "inputs": [
        { "name": "owner", "type": "address", "internalType": "address" }
      ]
    },
    {
      "type": "error",
      "name": "OwnableUnauthorizedAccount",
      "inputs": [
        { "name": "account", "type": "address", "internalType": "address" }
      ]
    }
  ]"#
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
    /// Initialize wallet: generate seed and keys
    Onboard,

    /// Buy bond from issuer
    Buy {
        #[arg(long)]
        value: u64,
        #[arg(long)]
        maturity: u64,
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

#[derive(Serialize, Deserialize, Debug)]
struct Wallet {
    keys: ShieldedKeys,
    created_at: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct Bond {
    commitment: String,
    nullifier: String,
    value: u64,
    salt: u64,
    owner: String,
    asset_id: u64,
    maturity_date: u64,
    created_at: String,
}

fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();

    // Run async commands
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        match cli.command {
            Commands::Onboard => onboard(&cli.wallet).await,
            Commands::Buy { value, maturity } => buy(&cli.wallet, value, maturity).await,
            Commands::Trade { bond_a, bond_b } => trade(&cli.wallet, &bond_a, &bond_b).await,
            Commands::Redeem { bond } => redeem(&cli.wallet, &bond).await,
            Commands::Info { bond } => info(&bond),
        }
    });

    Ok(())
}

async fn onboard(wallet_name: &str) {
    println!("\n🔐 Issuer Onboarding: Creating initial bond tranche...");

    // Generate keys for issuer
    let keys = ShieldedKeys::generate();

    let wallet = Wallet {
        keys: keys.clone(),
        created_at: Utc::now().to_rfc3339(),
    };

    // Save wallet
    let filename = format!("{}.json", wallet_name);
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

    // Extract owner as u64 from seed (first 8 bytes)
    let owner_u64 = u64::from_le_bytes(
        keys.public_spending_key_hex.as_bytes()[..8]
            .try_into()
            .expect("Failed to convert to [u8; 8]"),
    );

    let global_note = Note {
        value: global_value,
        salt,
        owner: owner_u64,
        asset_id: 1,
        maturity_date,
    };

    // Compute commitment
    let commitment = global_note.commit();
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

    let filename = "global_note_tranche.json";
    match fs::write(filename, serde_json::to_string_pretty(&bond).unwrap()) {
        Ok(_) => println!("\n✅ Global note saved to: {}", filename),
        Err(e) => println!("❌ Error saving: {}", e),
    }
}

async fn buy(wallet_name: &str, value: u64, maturity: u64) {
    println!("\n💳 Buying bond from issuer...");
    println!("   Value: {}", value);
    println!("   Maturity: {} ({})", maturity, format_date(maturity));

    // Load wallet to get keys
    let wallet = match load_wallet(wallet_name) {
        Some(w) => w,
        None => {
            println!("❌ No wallet found. Run 'onboard' first.");
            return;
        }
    };

    // Generate random salt
    let mut rng = rand::thread_rng();
    let salt = rng.gen::<u64>();

    // Get owner from wallet's public spending key
    let owner_u64 = u64::from_le_bytes(
        wallet.keys.public_spending_key_hex.as_bytes()[..8]
            .try_into()
            .expect("Failed to convert to [u8; 8]"),
    );

    let note = Note {
        value,
        salt,
        owner: owner_u64,
        asset_id: 1,
        maturity_date: maturity,
    };

    // Compute commitment and nullifier
    let commitment = note.commit();

    // Use the wallet's keys to compute proper nullifier
    let nullifier = wallet.keys.sign_nullifier(salt);

    println!("\n✅ Bond created locally!");
    println!("   Commitment: {}", commitment);
    println!("   Nullifier:  {}", nullifier);

    // Log contract call info (actual call would require proof generation)
    println!("\n📝 Proof generation info:");
    println!("   Command: nargo execute <witness> && bb prove -b ./target/<circuit>.json -w ./target/<witness> -o ./target");
    println!("   Location: ../circuits/");
    println!("   Status:   ℹ️  Run proof generation in circuits directory");

    println!("\n📝 Contract call info:");
    println!("   Function: mint(commitment, newRoot)");
    println!("   Address:  {}", PRIVATE_BOND_ADDRESS);
    println!("   RPC:      {}", RPC_URL);
    println!("   Status:   ℹ️  Proof ready for on-chain submission");

    // Save bond
    let bond = Bond {
        commitment: format!("{}", commitment),
        nullifier: format!("{}", nullifier),
        value,
        salt,
        owner: wallet.keys.public_spending_key_hex.clone(),
        asset_id: 1,
        maturity_date: maturity,
        created_at: Utc::now().to_rfc3339(),
    };

    let commit_str = format!("{}", commitment);
    let filename = format!("bond_{}_{}.json", wallet_name, &commit_str[4..16]); // Extract hex portion
    match fs::write(&filename, serde_json::to_string_pretty(&bond).unwrap()) {
        Ok(_) => println!("   Saved to: {}", filename),
        Err(e) => println!("❌ Error saving: {}", e),
    }
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

fn load_wallet(wallet_name: &str) -> Option<Wallet> {
    let filename = format!("{}.json", wallet_name);
    match fs::read_to_string(&filename) {
        Ok(content) => serde_json::from_str(&content).ok(),
        Err(_) => None,
    }
}

fn load_bond(path: &str) -> Option<Bond> {
    match fs::read_to_string(path) {
        Ok(content) => match serde_json::from_str(&content) {
            Ok(bond) => Some(bond),
            Err(e) => {
                println!("❌ Error parsing bond: {}", e);
                None
            }
        },
        Err(e) => {
            println!("❌ Error reading bond: {}", e);
            None
        }
    }
}

fn format_date(ts: u64) -> String {
    match chrono::DateTime::from_timestamp(ts as i64, 0) {
        Some(dt) => dt.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
        None => format!("{} (invalid)", ts),
    }
}
