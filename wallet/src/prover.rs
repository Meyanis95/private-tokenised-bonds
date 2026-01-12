use std::path::Path;
use std::process::Command;

/// Generate a proof for a bond using nargo and bb
pub async fn generate_proof(circuit_dir: &str, witness_name: &str) -> Result<String, String> {
    println!("   🔄 Generating witness...");

    // Step 1: nargo execute to generate witness
    let output = Command::new("nargo")
        .arg("execute")
        .arg(witness_name)
        .current_dir(circuit_dir)
        .output()
        .map_err(|e| format!("Failed to run nargo: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "nargo execute failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    println!("   ✅ Witness generated");
    println!("   🔄 Generating proof with bb...");

    // Step 2: bb prove to generate actual proof
    let bb_output = Command::new("bb")
        .arg("prove")
        .arg("-b")
        .arg(&format!("./target/{}.json", witness_name))
        .arg("-w")
        .arg(&format!("./target/{}", witness_name))
        .arg("-o")
        .arg("./target")
        .arg("--oracle_hash")
        .arg("keccak")
        .current_dir(circuit_dir)
        .output()
        .map_err(|e| format!("Failed to run bb prove: {}", e))?;

    if !bb_output.status.success() {
        return Err(format!(
            "bb prove failed: {}",
            String::from_utf8_lossy(&bb_output.stderr)
        ));
    }

    println!("   ✅ Proof generated!");

    // Return path to proof
    Ok(format!("{}/target/proof", circuit_dir))
}
