//! SecuraMem CLI - AI Black Box Recorder

use clap::Parser;
use std::path::PathBuf;
use securamem_storage::Database;

#[derive(Parser)]
#[command(name = "smrust")]
#[command(version = "2.0.0")]
#[command(about = "SecuraMem - AI Black Box Recorder (Audit-Only)", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(clap::Subcommand)]
enum Commands {
    /// Initialize SecuraMem audit log
    Init,

    /// Log a test event to the audit chain
    Log {
        /// The message to log
        #[arg(short, long)]
        message: String,
    },

    /// Verify audit chain integrity
    Verify {
        /// Verify all receipts
        #[arg(long)]
        all: bool,
    },

    /// Start the L3 API and Monitoring Server (Daemon Mode)
    Serve {
        #[arg(long, default_value = "3050")]
        port: u16,
    },

    /// Show status
    Status,

    /// Generate vendor keypair (development only)
    #[command(hide = true)]
    GenVendorKeys,

    /// Generate license for a client (vendor only)
    #[command(hide = true)]
    GenLicense {
        /// Target machine ID
        #[arg(long)]
        machine_id: String,
        /// Company/organization name
        #[arg(long)]
        company: String,
        /// License type (trial, standard, enterprise)
        #[arg(long, default_value = "trial")]
        license_type: String,
        /// Number of days the license is valid
        #[arg(long, default_value = "14")]
        days: i64,
        /// Path to vendor private key PEM file
        #[arg(long)]
        vendor_key: String,
    },

    /// Show machine ID (for license generation)
    MachineId,

    /// Start the semantic firewall proxy (NeuroWall)
    Firewall {
        #[arg(long, default_value = "3051")]
        port: u16,
        /// OpenAI API key for proxying requests
        #[arg(long, env = "OPENAI_API_KEY")]
        openai_api_key: String,
    },

    /// Test embedding generation (debug tool)
    TestEmbedding {
        /// Text to embed
        #[arg(short, long)]
        text: String,
    },
}

#[tokio::main]
async fn main() -> securamem_core::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_target(false)
        .with_thread_ids(false)
        .with_level(true)
        .init();

    let cli = Cli::parse();

    // LICENSE CHECK (Skip for vendor/development commands)
    let skip_license_check = matches!(cli.command, Commands::GenVendorKeys | Commands::GenLicense { .. } | Commands::MachineId);

    if !skip_license_check {
        let license_path = std::path::PathBuf::from("license.key");

        if !license_path.exists() {
            eprintln!("\n❌ LICENSE NOT FOUND");
            eprintln!("SecuraMem requires a valid license to operate.");
            eprintln!();

            let machine_id = securamem_core::license::get_machine_id()?;
            eprintln!("Your Machine ID: {}", machine_id);
            eprintln!();
            eprintln!("📧 Send this Machine ID to jeremy@securamem.com to request a license.");
            eprintln!("💾 Place the received license.key file in the current directory.");
            std::process::exit(1);
        }

        match securamem_core::license::verify_license(&license_path) {
            Ok(info) => {
                if info.days_remaining <= 7 {
                    eprintln!("\n⚠️  LICENSE EXPIRING SOON");
                    eprintln!("  {} days remaining", info.days_remaining);
                }
                tracing::info!("License verified: {} ({}) - {} days remaining",
                    info.company, info.license_type, info.days_remaining);
            }
            Err(e) => {
                eprintln!("\n❌ LICENSE INVALID");
                eprintln!("{}", e);
                eprintln!();

                let machine_id = securamem_core::license::get_machine_id()?;
                eprintln!("Your Machine ID: {}", machine_id);
                eprintln!();
                eprintln!("📧 Contact jeremy@securamem.com for a new license.");
                std::process::exit(1);
            }
        }
    }

    match cli.command {
        Commands::Init => {
            let db_path = PathBuf::from(".securamemrust/memory.db");

            tracing::info!("Initializing SecuraMem storage...");

            let db = Database::init(&db_path).await?;

            // Verify genesis entry exists
            db.ping().await?;

            tracing::info!("✓ Storage initialization complete.");
            println!("SecuraMem initialized successfully at {:?}", db_path);

            Ok(())
        }
        Commands::Log { message } => {
            let db_path = PathBuf::from(".securamemrust/memory.db");

            // Check if database exists
            if !db_path.exists() {
                tracing::error!("Database not initialized. Run 'smrust init' first.");
                return Ok(());
            }

            // Connect to database
            let db = Database::init(&db_path).await?;

            // Generate or load signing key (for MVP, generate a temp key)
            // TODO: In production, load from .securamemrust/keys/
            let key = securamem_crypto::SecuraMemSigningKey::generate();

            // Create orchestrator
            let orchestrator = securamem_l1::AuditOrchestrator::new(&db, key);

            // Log the event
            match orchestrator.log_event("cli-user", "manual-log", &message).await {
                Ok(receipt_id) => {
                    println!("✓ Event logged successfully");
                    println!("  Receipt ID: {}", receipt_id);
                }
                Err(e) => {
                    tracing::error!("Failed to log event: {}", e);
                    return Err(e);
                }
            }

            Ok(())
        }
        Commands::Verify { all: _ } => {
            let db_path = PathBuf::from(".securamemrust/memory.db");

            // Check if database exists
            if !db_path.exists() {
                tracing::error!("Database not initialized. Run 'smrust init' first.");
                return Ok(());
            }

            // Connect to database
            let db = Database::init(&db_path).await?;

            // Generate temp key for orchestrator (not used for verification)
            let key = securamem_crypto::SecuraMemSigningKey::generate();

            // Create orchestrator
            let orchestrator = securamem_l1::AuditOrchestrator::new(&db, key);

            // Verify chain
            tracing::info!("Verifying audit chain integrity...");
            match orchestrator.verify_integrity().await {
                Ok(true) => {
                    println!("✓ AUDIT CHAIN INTEGRITY CONFIRMED");
                    let count = orchestrator.count_entries().await?;
                    println!("  Total entries verified: {}", count + 1); // +1 for genesis
                }
                Ok(false) => {
                    println!("✗ CHAIN INTEGRITY COMPROMISED");
                    tracing::error!("Hash chain verification failed!");
                }
                Err(e) => {
                    tracing::error!("Verification error: {}", e);
                    return Err(e);
                }
            }

            Ok(())
        }
        Commands::Serve { port } => {
            let db_path = PathBuf::from(".securamemrust/memory.db");

            // Check if database exists
            if !db_path.exists() {
                tracing::error!("Database not initialized. Run 'smrust init' first.");
                return Ok(());
            }

            // Connect to database
            tracing::info!("Starting SecuraMem Daemon...");
            let db = Database::init(&db_path).await?;

            // Hand over control to the L3 crate (this blocks forever)
            if let Err(e) = securamem_l3::start_server(db, port).await {
                tracing::error!("Server crashed: {}", e);
                std::process::exit(1);
            }

            Ok(())
        }
        Commands::Status => {
            let db_path = PathBuf::from(".securamemrust/memory.db");

            // Check if database exists
            if !db_path.exists() {
                println!("SecuraMem Status: NOT INITIALIZED");
                println!("  Run 'smrust init' to initialize the audit log.");
                return Ok(());
            }

            // Connect to database
            let db = Database::init(&db_path).await?;

            // Query audit log count
            let count: (i64,) = securamem_storage::sqlx::query_as("SELECT COUNT(*) FROM audit_log")
                .fetch_one(&db.pool)
                .await
                .map_err(|e| securamem_core::SecuraMemError::Database(e.to_string()))?;

            // Get genesis entry
            let genesis: (String, String, String) = securamem_storage::sqlx::query_as(
                "SELECT receipt_id, actor_user_id, operation_type FROM audit_log WHERE id = 1"
            )
                .fetch_one(&db.pool)
                .await
                .map_err(|e| securamem_core::SecuraMemError::Database(e.to_string()))?;

            println!("SecuraMem Status:");
            println!("  Version: 2.0.0");
            println!("  Mode: Audit-Only (AI Black Box Recorder)");
            println!("  Database: {:?}", db_path);
            println!("  Total Entries: {}", count.0);
            println!("  Genesis Entry:");
            println!("    Receipt ID: {}", genesis.0);
            println!("    Actor: {}", genesis.1);
            println!("    Operation: {}", genesis.2);

            Ok(())
        }
        Commands::GenVendorKeys => {
            // Generate a fresh ED25519 keypair for vendor use
            use ed25519_dalek::SigningKey;
            use ed25519_dalek::pkcs8::{EncodePrivateKey, EncodePublicKey};
            use rand_core::OsRng;

            let signing_key = SigningKey::generate(&mut OsRng);
            let verifying_key = signing_key.verifying_key();

            let private_pem = signing_key
                .to_pkcs8_pem(pkcs8::LineEnding::LF)
                .map_err(|e| securamem_core::SecuraMemError::Internal(e.to_string()))?;

            let public_pem = verifying_key
                .to_public_key_pem(pkcs8::LineEnding::LF)
                .map_err(|e| securamem_core::SecuraMemError::Internal(e.to_string()))?;

            println!("=== VENDOR KEYPAIR GENERATED ===\n");
            println!("PRIVATE KEY (Keep this secure!):");
            println!("{}", private_pem.as_str());
            println!("\nPUBLIC KEY (Embed this in VENDOR_PUBLIC_KEY constant):");
            println!("{}", public_pem);
            println!("\n⚠️  Save the private key to a secure location!");
            println!("⚠️  Update securamem-core/src/license.rs with the public key!");

            Ok(())
        }
        Commands::GenLicense { machine_id, company, license_type, days, vendor_key } => {
            // Read vendor private key
            let vendor_key_pem = std::fs::read_to_string(&vendor_key)
                .map_err(|e| securamem_core::SecuraMemError::LicenseError(format!("Failed to read vendor key: {}", e)))?;

            // Generate license
            let license_jwt = securamem_core::license::generate_license(
                &machine_id,
                &company,
                &license_type,
                days,
                &vendor_key_pem,
            )?;

            // Save to file
            let license_path = std::path::PathBuf::from("license.key");
            std::fs::write(&license_path, &license_jwt)
                .map_err(|e| securamem_core::SecuraMemError::LicenseError(format!("Failed to write license: {}", e)))?;

            println!("✓ License generated successfully!");
            println!("  Company: {}", company);
            println!("  Type: {}", license_type);
            println!("  Valid for: {} days", days);
            println!("  Machine ID: {}", machine_id);
            println!("  Saved to: {:?}", license_path);
            println!("\n📧 Send this license.key file to the customer.");

            Ok(())
        }
        Commands::MachineId => {
            let machine_id = securamem_core::license::get_machine_id()?;
            println!("Machine ID: {}", machine_id);
            println!("\n📧 Send this Machine ID to jeremy@securamem.com to request a license.");
            Ok(())
        }
        Commands::Firewall { port, openai_api_key } => {
            tracing::info!("Starting SecuraMem Firewall (NeuroWall) with audit logging...");

            let root_dir = std::env::current_dir()?;
            let keys_dir = root_dir.join(".securamemrust/keys");
            let db_path = root_dir.join(".securamemrust/memory.db");

            // 1. Check database exists
            if !db_path.exists() {
                tracing::error!("Database not initialized. Run 'smrust init' first.");
                return Ok(());
            }

            let db = Database::init(&db_path).await?;
            tracing::info!("✓ Database connected");

            // 2. Load PERSISTENT identity (critical for chain-of-custody)
            let private_key_path = keys_dir.join("private.pem");
            let identity = if private_key_path.exists() {
                tracing::info!("Loading persistent firewall identity...");
                securamem_crypto::SecuraMemSigningKey::load_from_file(&private_key_path)?
            } else {
                tracing::info!("Generating new persistent firewall identity...");
                let identity = securamem_crypto::SecuraMemSigningKey::generate();
                identity.save_to_file(&private_key_path)?;
                identity
            };

            tracing::info!("✓ Firewall identity loaded (key_id: {}...)", &identity.key_id()[..32]);

            // 3. Start firewall with audit logging
            if let Err(e) = securamem_firewall::start_firewall_server(port, openai_api_key, db, identity).await {
                tracing::error!("Firewall server crashed: {}", e);
                std::process::exit(1);
            }

            Ok(())
        }
        Commands::TestEmbedding { text } => {
            tracing::info!("Initializing semantic engine for test...");

            // Initialize semantic engine
            let engine = securamem_firewall::SemanticEngine::new()
                .map_err(|e| securamem_core::SecuraMemError::Internal(e.to_string()))?;

            // Generate embedding
            let embedding = engine.embed(&text)
                .map_err(|e| securamem_core::SecuraMemError::Internal(e.to_string()))?;

            println!("Text: {}", text);
            println!("Embedding dimensions: {}", embedding.len());
            println!("First 10 values: {:?}", &embedding[..10.min(embedding.len())]);

            // Compute L2 norm (should be ~1.0 for normalized vectors)
            let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
            println!("L2 norm: {:.6} (should be ~1.0)", norm);

            Ok(())
        }
    }
}
