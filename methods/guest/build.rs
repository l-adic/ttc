use std::env;
use std::fs;
use std::path::Path;
use std::process::Command;

fn main() {
    // Get the workspace directory using a more reliable method
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");
    let workspace_dir = Path::new(&manifest_dir).ancestors().nth(2).expect("Workspace not found");
    // Check if the contract file exists, if not, we need to compile it
    let contract_json_path =
        workspace_dir.join("contract/out/ITopTradingCycle.sol/ITopTradingCycle.json");

    if !contract_json_path.exists() {
        println!(
            "cargo:warning=Contract ABI not found at {:?}, compiling contracts...",
            contract_json_path
        );

        // Compile the Solidity contract
        Command::new("make")
            .arg("compile-contract-deps") // Or whatever command you use
            .current_dir(workspace_dir)
            .status()
            .expect("Failed to compile Solidity contracts");

        // Verify the file was created
        if !contract_json_path.exists() {
            panic!(
                "Contract ABI file was not generated: {:?}",
                contract_json_path
            );
        }
    }

    // Print debug info about the ABI file
    match fs::metadata(&contract_json_path) {
        Ok(metadata) => println!(
            "cargo:warning=Found contract ABI: {:?}, size: {} bytes",
            contract_json_path,
            metadata.len()
        ),
        Err(e) => println!("cargo:warning=Error checking ABI file: {}", e),
    }
}
