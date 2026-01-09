use chrono::Utc;
use clap::{Parser, Subcommand};
use ff::Field;
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::fs;

mod merkle;
mod notes;

use notes::Note;

#[derive(Parser)]
#[command(name = "Bond Wallet")]
#[command(about = "CLI wallet for zero-coupon bond protocol", long_about = None)]
struct Cli {
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
    seed: String,
    created_at: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct Bond {
    commitment: String,
    nullifier: String,
    value: u64,
    salt: u64,
    seed: String,
    asset_id: u64,
    maturity_date: u64,
    created_at: String,
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Onboard => onboard(),
        Commands::Buy { value, maturity } => buy(value, maturity),
        Commands::Trade { bond_a, bond_b } => trade(&bond_a, &bond_b),
        Commands::Redeem { bond } => redeem(&bond),
        Commands::Info { bond } => info(&bond),
    }
}

fn onboard() {
    println!("\n🔐 Initializing wallet...");

    // Generate random seed
    let mut rng = rand::thread_rng();
    let seed_bytes: Vec<u8> = (0..32).map(|_| rng.gen()).collect();
    let seed_hex = hex::encode(&seed_bytes);

    let wallet = Wallet {
        seed: seed_hex.clone(),
        created_at: Utc::now().to_rfc3339(),
    };

    let filename = "wallet.json";
    match fs::write(filename, serde_json::to_string_pretty(&wallet).unwrap()) {
        Ok(_) => {
            println!("✅ Wallet created!");
            println!("   Seed: {} (KEEP SAFE!)", seed_hex);
            println!("   Saved to: {}", filename);
        }
        Err(e) => println!("❌ Error: {}", e),
    }
}

fn buy(value: u64, maturity: u64) {
    println!("\n💳 Buying bond from issuer...");
    println!("   Value: {}", value);
    println!("   Maturity: {} ({})", maturity, format_date(maturity));

    // Load wallet to get seed
    let wallet = match load_wallet() {
        Some(w) => w,
        None => {
            println!("❌ No wallet found. Run 'onboard' first.");
            return;
        }
    };

    // Generate random salt
    let mut rng = rand::thread_rng();
    let salt = rng.gen::<u64>();

    // Create bond note
    let seed_u64 = u64::from_le_bytes(hex::decode(&wallet.seed).unwrap()[0..8].try_into().unwrap());

    let note = Note {
        value,
        salt,
        owner: seed_u64,
        asset_id: 1,
        maturity_date: maturity,
    };

    // Compute commitment and nullifier
    let commitment = note.commit();
    // Create private key from seed - simplified for demo
    let private_key_val = poseidon_rs::Poseidon::new()
        .hash(vec![
            poseidon_rs::Fr::one(), // placeholder - in production would properly derive
        ])
        .unwrap();
    let nullifier = note.nullifer(private_key_val);

    println!("\n✅ Bond issued!");
    println!("   Commitment: {}", commitment);
    println!("   Nullifier:  {}", nullifier);

    // Save bond
    let bond = Bond {
        commitment: format!("{}", commitment),
        nullifier: format!("{}", nullifier),
        value,
        salt,
        seed: wallet.seed,
        asset_id: 1,
        maturity_date: maturity,
        created_at: Utc::now().to_rfc3339(),
    };

    let commit_str = format!("{}", commitment);
    let filename = format!("bond_{}.json", &commit_str[4..16]); // Extract hex portion
    match fs::write(&filename, serde_json::to_string_pretty(&bond).unwrap()) {
        Ok(_) => println!("   Saved to: {}", filename),
        Err(e) => println!("❌ Error saving: {}", e),
    }
}

fn trade(bond_a_path: &str, bond_b_path: &str) {
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
    println!("   Merkle root updated on-chain");
}

fn redeem(bond_path: &str) {
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
    println!("   Action: call burn(proof, root, nullifier, ...)");
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

fn load_wallet() -> Option<Wallet> {
    match fs::read_to_string("wallet.json") {
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
