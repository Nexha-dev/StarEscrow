use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use serde_json::{json, Value};

/// StarEscrow CLI — interact with the escrow contract on Stellar Testnet.
///
/// Prerequisites:
///   - Stellar CLI installed: https://developers.stellar.org/docs/tools/developer-tools/cli/install-cli
///   - Contract deployed and ESCROW_CONTRACT_ID set in env
///   - PAYER_SECRET and FREELANCER_SECRET set in env
#[derive(Parser)]
#[command(name = "star-escrow", version, about)]
struct Cli {
    /// Soroban RPC endpoint (default: Testnet)
    #[arg(long, default_value = "https://soroban-testnet.stellar.org")]
    rpc_url: String,

    /// Network passphrase
    #[arg(long, default_value = "Test SDF Network ; September 2015")]
    network_passphrase: String,

    /// Output results as JSON
    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialise protocol config (admin, fee)
    Init {
        #[arg(long, env = "ESCROW_CONTRACT_ID")]
        contract_id: String,

        #[arg(long, env = "ADMIN_SECRET")]
        admin_secret: String,

        /// Fee in basis points (e.g. 100 = 1%)
        #[arg(long, default_value = "0")]
        fee_bps: u32,

        /// Fee collector Stellar address
        #[arg(long)]
        fee_collector: String,
    },

    /// Pause all state-changing operations (admin only)
    Pause {
        #[arg(long, env = "ESCROW_CONTRACT_ID")]
        contract_id: String,

        #[arg(long, env = "ADMIN_SECRET")]
        admin_secret: String,
    },

    /// Unpause the contract (admin only)
    Unpause {
        #[arg(long, env = "ESCROW_CONTRACT_ID")]
        contract_id: String,

        #[arg(long, env = "ADMIN_SECRET")]
        admin_secret: String,
    },

    /// Create a new escrow and lock funds
    Create {
        #[arg(long, env = "ESCROW_CONTRACT_ID")]
        contract_id: String,

        #[arg(long, env = "PAYER_SECRET")]
        payer_secret: String,

        /// Freelancer Stellar address
        #[arg(long)]
        freelancer: String,

        /// Token contract ID (use native XLM wrapper or a SAC address)
        #[arg(long)]
        token: String,

        /// Amount in stroops (1 XLM = 10_000_000)
        #[arg(long)]
        amount: i128,

        /// Milestone description
        #[arg(long)]
        milestone: String,

        /// Optional deadline as a ledger timestamp (Unix seconds).
        #[arg(long)]
        deadline: Option<u64>,
    },

    /// Freelancer submits work
    SubmitWork {
        #[arg(long, env = "ESCROW_CONTRACT_ID")]
        contract_id: String,

        #[arg(long, env = "FREELANCER_SECRET")]
        freelancer_secret: String,
    },

    /// Transfer freelancer role to a new address
    TransferFreelancer {
        #[arg(long, env = "ESCROW_CONTRACT_ID")]
        contract_id: String,

        #[arg(long, env = "FREELANCER_SECRET")]
        freelancer_secret: String,

        /// New freelancer Stellar address
        #[arg(long)]
        new_freelancer: String,
    },

    /// Payer approves milestone and releases payment
    Approve {
        #[arg(long, env = "ESCROW_CONTRACT_ID")]
        contract_id: String,

        #[arg(long, env = "PAYER_SECRET")]
        payer_secret: String,
    },

    /// Payer cancels escrow and gets refund (only before work submitted)
    Cancel {
        #[arg(long, env = "ESCROW_CONTRACT_ID")]
        contract_id: String,

        #[arg(long, env = "PAYER_SECRET")]
        payer_secret: String,
    },

    /// Payer reclaims funds after the deadline has passed
    Expire {
        #[arg(long, env = "ESCROW_CONTRACT_ID")]
        contract_id: String,

        #[arg(long, env = "PAYER_SECRET")]
        payer_secret: String,
    },

    /// Read current escrow status and full data
    Status {
        #[arg(long, env = "ESCROW_CONTRACT_ID")]
        contract_id: String,
    },

    /// List all escrows created by a payer address
    List {
        #[arg(long, env = "ESCROW_CONTRACT_ID")]
        contract_id: String,

        /// Payer Stellar address to filter by
        #[arg(long)]
        payer: String,
    },

    /// Build (optional) and deploy the escrow contract WASM to the network
    Deploy {
        /// Path to pre-built WASM file. If omitted, runs `stellar contract build` first.
        #[arg(long)]
        wasm: Option<std::path::PathBuf>,

        /// Deployer secret key (pays the deployment fee)
        #[arg(long, env = "DEPLOYER_SECRET")]
        deployer_secret: String,

        /// Write the resulting contract ID to a local .env file
        #[arg(long, default_value = ".env")]
        env_file: std::path::PathBuf,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let as_json = cli.json;

    match cli.command {
        Commands::Init { contract_id, admin_secret, fee_bps, fee_collector } => {
            let admin_addr = stellar_address_from_secret(&admin_secret)?;
            invoke_stellar_cli(
                &cli.rpc_url, &cli.network_passphrase, &contract_id, &admin_secret,
                "init",
                &["--admin", &admin_addr, "--fee-bps", &fee_bps.to_string(), "--fee-collector", &fee_collector],
            )?;
            output(as_json, json!({"status": "ok", "action": "init"}), "Protocol initialised.");
        }

        Commands::Pause { contract_id, admin_secret } => {
            invoke_stellar_cli(
                &cli.rpc_url, &cli.network_passphrase, &contract_id, &admin_secret,
                "pause", &[],
            )?;
            output(as_json, json!({"status": "ok", "action": "pause"}), "Contract paused.");
        }

        Commands::Unpause { contract_id, admin_secret } => {
            invoke_stellar_cli(
                &cli.rpc_url, &cli.network_passphrase, &contract_id, &admin_secret,
                "unpause", &[],
            )?;
            output(as_json, json!({"status": "ok", "action": "unpause"}), "Contract unpaused.");
        }

        Commands::Create { contract_id, payer_secret, freelancer, token, amount, milestone, deadline } => {
            let payer_addr = stellar_address_from_secret(&payer_secret)?;
            let deadline_str = deadline.map(|d| d.to_string()).unwrap_or_else(|| "null".into());
            invoke_stellar_cli(
                &cli.rpc_url, &cli.network_passphrase, &contract_id, &payer_secret,
                "create",
                &["--payer", &payer_addr, "--freelancer", &freelancer, "--token", &token,
                  "--amount", &amount.to_string(), "--milestone", &milestone,
                  "--deadline", &deadline_str],
            )?;
            output(
                as_json,
                json!({"status": "ok", "action": "create", "contract_id": contract_id,
                       "payer": payer_addr, "freelancer": freelancer, "amount": amount,
                       "milestone": milestone, "deadline": deadline}),
                "Escrow created. Funds locked.",
            );
        }

        Commands::SubmitWork { contract_id, freelancer_secret } => {
            invoke_stellar_cli(
                &cli.rpc_url, &cli.network_passphrase, &contract_id, &freelancer_secret,
                "submit_work", &[],
            )?;
            output(as_json, json!({"status": "ok", "action": "submit_work"}), "Work submitted. Waiting for payer approval.");
        }

        Commands::TransferFreelancer { contract_id, freelancer_secret, new_freelancer } => {
            invoke_stellar_cli(
                &cli.rpc_url, &cli.network_passphrase, &contract_id, &freelancer_secret,
                "transfer_freelancer",
                &["--new-freelancer", &new_freelancer],
            )?;
            output(
                as_json,
                json!({"status": "ok", "action": "transfer_freelancer", "new_freelancer": new_freelancer}),
                &format!("Freelancer role transferred to {new_freelancer}."),
            );
        }

        Commands::Approve { contract_id, payer_secret } => {
            invoke_stellar_cli(
                &cli.rpc_url, &cli.network_passphrase, &contract_id, &payer_secret,
                "approve", &[],
            )?;
            output(as_json, json!({"status": "ok", "action": "approve"}), "Payment released to freelancer.");
        }

        Commands::Cancel { contract_id, payer_secret } => {
            invoke_stellar_cli(
                &cli.rpc_url, &cli.network_passphrase, &contract_id, &payer_secret,
                "cancel", &[],
            )?;
            output(as_json, json!({"status": "ok", "action": "cancel"}), "Escrow cancelled. Funds refunded to payer.");
        }

        Commands::Expire { contract_id, payer_secret } => {
            invoke_stellar_cli(
                &cli.rpc_url, &cli.network_passphrase, &contract_id, &payer_secret,
                "expire", &[],
            )?;
            output(as_json, json!({"status": "ok", "action": "expire"}), "Escrow expired. Funds returned to payer.");
        }

        Commands::Status { contract_id } => {
            let raw = query_contract(&cli.rpc_url, &cli.network_passphrase, &contract_id, "get_escrow")?;
            if as_json {
                // Parse the XDR/JSON output from stellar CLI and re-emit as JSON
                let parsed: Value = serde_json::from_str(raw.trim()).unwrap_or(Value::String(raw.trim().to_string()));
                println!("{}", serde_json::to_string_pretty(&json!({"status": "ok", "escrow": parsed}))?);
            } else {
                println!("{}", raw.trim());
            }
        }

        Commands::List { contract_id, payer } => {
            list_escrows(&cli.rpc_url, &cli.network_passphrase, &contract_id, &payer, as_json)?;
        }

        Commands::Deploy { wasm, deployer_secret, env_file } => {
            deploy_contract(&cli.rpc_url, &cli.network_passphrase, wasm.as_deref(), &deployer_secret, &env_file, as_json)?;
        }
    }

    Ok(())
}

/// Build (if needed) and deploy the contract; print and optionally persist the contract ID.
fn deploy_contract(
    rpc_url: &str,
    network_passphrase: &str,
    wasm: Option<&std::path::Path>,
    deployer_secret: &str,
    env_file: &std::path::Path,
    as_json: bool,
) -> Result<()> {
    // Step 1: resolve WASM path, building if not provided.
    let wasm_path = match wasm {
        Some(p) => p.to_path_buf(),
        None => {
            eprintln!("No --wasm provided; running `stellar contract build`…");
            let status = std::process::Command::new("stellar")
                .args(["contract", "build"])
                .status()
                .context("stellar CLI not found — install from https://developers.stellar.org/docs/tools/developer-tools/cli/install-cli")?;
            if !status.success() {
                anyhow::bail!("`stellar contract build` failed");
            }
            std::path::PathBuf::from("target/wasm32-unknown-unknown/release/escrow.wasm")
        }
    };

    if !wasm_path.exists() {
        anyhow::bail!("WASM file not found: {}", wasm_path.display());
    }

    // Step 2: deploy.
    let out = std::process::Command::new("stellar")
        .args([
            "contract", "deploy",
            "--wasm", wasm_path.to_str().context("invalid wasm path")?,
            "--source", deployer_secret,
            "--rpc-url", rpc_url,
            "--network-passphrase", network_passphrase,
        ])
        .output()
        .context("stellar CLI not found")?;

    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        anyhow::bail!("Deployment failed: {stderr}");
    }

    let contract_id = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if contract_id.is_empty() {
        anyhow::bail!("Deployment succeeded but no contract ID was returned");
    }

    // Step 3: write to .env file.
    upsert_env_var(env_file, "ESCROW_CONTRACT_ID", &contract_id)?;

    output(
        as_json,
        serde_json::json!({"status": "ok", "contract_id": contract_id, "env_file": env_file.display().to_string()}),
        &format!("Deployed! Contract ID: {contract_id}\nWritten to {}", env_file.display()),
    );
    Ok(())
}

/// Insert or update a KEY=VALUE line in a .env file.
fn upsert_env_var(path: &std::path::Path, key: &str, value: &str) -> Result<()> {
    use std::io::Write as _;

    let existing = if path.exists() {
        std::fs::read_to_string(path).context("reading .env file")?
    } else {
        String::new()
    };

    let prefix = format!("{key}=");
    let new_line = format!("{key}={value}");
    let mut found = false;
    let updated: String = existing
        .lines()
        .map(|line| {
            if line.starts_with(&prefix) {
                found = true;
                new_line.clone()
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");

    let mut content = if found { updated } else { format!("{existing}\n{new_line}") };
    if !content.ends_with('\n') {
        content.push('\n');
    }

    let mut file = std::fs::File::create(path).context("writing .env file")?;
    file.write_all(content.as_bytes()).context("writing .env file")?;
    Ok(())
}

/// Print human-readable or JSON output depending on the flag.
fn output(as_json: bool, data: Value, human: &str) {
    if as_json {
        println!("{}", serde_json::to_string_pretty(&data).unwrap());
    } else {
        println!("{human}");
    }
}

/// Query `escrow_created` events for a given payer and display results.
fn list_escrows(rpc_url: &str, network_passphrase: &str, contract_id: &str, payer: &str, as_json: bool) -> Result<()> {
    let out = std::process::Command::new("stellar")
        .args([
            "contract", "events",
            "--id", contract_id,
            "--rpc-url", rpc_url,
            "--network-passphrase", network_passphrase,
            "--output", "json",
        ])
        .output()
        .context("stellar CLI not found — install from https://developers.stellar.org/docs/tools/developer-tools/cli/install-cli")?;

    let raw = String::from_utf8_lossy(&out.stdout);
    let events: Vec<Value> = serde_json::from_str(&raw).unwrap_or_default();

    let escrows: Vec<Value> = events
        .into_iter()
        .filter(|e| {
            e["topic"][0].as_str().unwrap_or("") == "escrow_created"
                && e["value"][0].as_str().unwrap_or("") == payer
        })
        .map(|e| json!({
            "contract_id": contract_id,
            "payer": e["value"][0],
            "freelancer": e["value"][1],
            "amount": e["value"][2],
            "milestone": e["value"][3],
        }))
        .collect();

    if as_json {
        println!("{}", serde_json::to_string_pretty(&json!({"escrows": escrows}))?);
    } else if escrows.is_empty() {
        println!("No escrows found for payer {payer}");
    } else {
        println!("Escrows for payer {payer}:");
        for (i, e) in escrows.iter().enumerate() {
            println!(
                "  [{}] contract={} milestone={} amount={} freelancer={}",
                i + 1,
                e["contract_id"].as_str().unwrap_or("-"),
                e["milestone"].as_str().unwrap_or("-"),
                e["amount"],
                e["freelancer"].as_str().unwrap_or("-"),
            );
        }
    }

    Ok(())
}

fn query_contract(rpc_url: &str, network_passphrase: &str, contract_id: &str, function: &str) -> Result<String> {
    let out = std::process::Command::new("stellar")
        .args([
            "contract", "invoke",
            "--id", contract_id,
            "--rpc-url", rpc_url,
            "--network-passphrase", network_passphrase,
            "--",
            function,
        ])
        .output()
        .context("stellar CLI not found — install from https://developers.stellar.org/docs/tools/developer-tools/cli/install-cli")?;
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

fn invoke_stellar_cli(
    rpc_url: &str,
    network_passphrase: &str,
    contract_id: &str,
    secret: &str,
    function: &str,
    extra_args: &[&str],
) -> Result<()> {
    let mut args = vec![
        "contract", "invoke",
        "--id", contract_id,
        "--rpc-url", rpc_url,
        "--network-passphrase", network_passphrase,
        "--source", secret,
        "--",
        function,
    ];
    args.extend_from_slice(extra_args);

    let status = std::process::Command::new("stellar")
        .args(&args)
        .status()
        .context("stellar CLI not found — install from https://developers.stellar.org/docs/tools/developer-tools/cli/install-cli")?;

    if !status.success() {
        anyhow::bail!("stellar CLI exited with status {status}");
    }
    Ok(())
}

fn stellar_address_from_secret(secret: &str) -> Result<String> {
    let out = std::process::Command::new("stellar")
        .args(["keys", "address", secret])
        .output()
        .context("stellar CLI not found")?;
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}
