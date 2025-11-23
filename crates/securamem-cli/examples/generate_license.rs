//! License Generator for SecuraMem
//!
//! Usage:
//!   cargo run --example generate_license -- <machine_id> <company> <license_type> <days_valid> <private_key_path>

use std::env;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();

    if args.len() != 6 {
        eprintln!("Usage: {} <machine_id> <company> <license_type> <days_valid> <private_key_path>", args[0]);
        eprintln!("");
        eprintln!("Example:");
        eprintln!("  cargo run --example generate_license -- \\");
        eprintln!("    91f18d9691eea91d69f42a5bd474a26b1ca24b2747ba42fa3f99717caad79bfb \\");
        eprintln!("    \"Jeremy Corp\" developer 3649 \\");
        eprintln!("    C:\\Users\\scorp\\projects\\SecuraMem_Rust_VK\\vendor_private_key.pem");
        std::process::exit(1);
    }

    let machine_id = &args[1];
    let company = &args[2];
    let license_type = &args[3];
    let days_valid: i64 = args[4].parse()?;
    let private_key_path = &args[5];

    // Read vendor private key
    let private_key_pem = std::fs::read_to_string(private_key_path)?;

    // Generate license using securamem_core function
    let license_jwt = securamem_core::license::generate_license(
        machine_id,
        company,
        license_type,
        days_valid,
        &private_key_pem,
    )?;

    println!("\n=== LICENSE GENERATED ===");
    println!("");
    println!("Machine ID: {}", machine_id);
    println!("Company: {}", company);
    println!("License Type: {}", license_type);
    println!("Valid for: {} days ({} years)", days_valid, days_valid / 365);
    println!("");
    println!("LICENSE JWT (save this to license.key):");
    println!("{}", license_jwt);
    println!("");
    println!("=== INSTRUCTIONS ===");
    println!("1. Save the above JWT to a file named 'license.key'");
    println!("2. Place license.key in the same directory as smrust.exe");
    println!("3. Run: smrust init");
    println!("");

    // Also save to file
    std::fs::write("license.key", &license_jwt)?;
    println!("✅ License saved to: license.key");

    Ok(())
}
