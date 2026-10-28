//! zkcert: host CLI for the EulerPhi verified-expert proof.
//!
//!   zkcert check   --input fixtures/input.test.json      # native dry run, no zkVM
//!   zkcert execute --input ...                           # run in SP1 without proving
//!   zkcert prove   --input ... --system compressed       # produce proof.bin
//!   zkcert verify  --proof proof.bin                     # verify + print public values
//!   zkcert vkey                                          # program verification key
//!
//! Instead of --input you can pass --proofs-dir proofs/ --policy policy.json --wallet 0x...

use std::path::{Path, PathBuf};

use clap::{Parser, Subcommand, ValueEnum};
use eulerphi_core::{decode_public, evaluate, ProgramInput, PublicValuesStruct, ReclaimProof, Verdict};
use sp1_sdk::{
    include_elf, Elf, HashableKey, ProveRequest, Prover, ProverClient, ProvingKey,
    SP1ProofWithPublicValues, SP1Stdin,
};

const ELF: Elf = include_elf!("eulerphi-zkcert-program");

#[derive(Parser)]
#[command(name = "zkcert", about = "EulerPhi verified-expert proofs from zkTLS certificates")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(clap::Args, Clone)]
struct InputArgs {
    /// A complete ProgramInput JSON (e.g. fixtures/input.test.json).
    #[arg(long, conflicts_with_all = ["proofs_dir", "policy", "wallet"])]
    input: Option<PathBuf>,
    /// Directory of Reclaim proof JSON files (as written by `npm run request`).
    #[arg(long)]
    proofs_dir: Option<PathBuf>,
    #[arg(long)]
    policy: Option<PathBuf>,
    /// Wallet the proofs must be bound to.
    #[arg(long)]
    wallet: Option<String>,
    /// Evaluation time (unix seconds). Defaults to now.
    #[arg(long)]
    at: Option<u64>,
    /// Private 32-byte hex blinding for the commitment. Defaults to a fresh random value
    /// (printed to stderr; keep it to open the commitment later).
    #[arg(long)]
    blinding: Option<String>,
}

#[derive(Copy, Clone, ValueEnum)]
enum System {
    /// Fast, large proof. Good for local demos.
    Core,
    /// Constant-size STARK. Default.
    Compressed,
    /// ~260-byte SNARK verifiable on Ethereum (needs Docker or lots of RAM).
    Groth16,
    Plonk,
}

#[derive(Subcommand)]
enum Cmd {
    Check(InputArgs),
    Execute(InputArgs),
    Prove {
        #[command(flatten)]
        input: InputArgs,
        #[arg(long, value_enum, default_value = "compressed")]
        system: System,
        #[arg(long, default_value = "proof.bin")]
        out: PathBuf,
    },
    Verify {
        #[arg(long, default_value = "proof.bin")]
        proof: PathBuf,
    },
    Vkey,
}

fn load_input(a: &InputArgs) -> ProgramInput {
    let mut input = if let Some(p) = &a.input {
        serde_json::from_str(&read(p)).expect("invalid ProgramInput JSON")
    } else {
        let dir = a.proofs_dir.as_ref().expect("--input or --proofs-dir required");
        let policy = serde_json::from_str(&read(a.policy.as_ref().expect("--policy required")))
            .expect("invalid policy JSON");
        let mut files: Vec<_> = std::fs::read_dir(dir)
            .expect("cannot read proofs dir")
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|x| x == "json"))
            .collect();
        files.sort();
        let proofs = files
            .iter()
            .flat_map(|f| {
                // A file may hold one proof or an array of proofs.
                let raw = read(f);
                serde_json::from_str::<Vec<ReclaimProof>>(&raw)
                    .or_else(|_| serde_json::from_str::<ReclaimProof>(&raw).map(|p| vec![p]))
                    .unwrap_or_else(|e| panic!("{}: not a Reclaim proof: {e}", f.display()))
            })
            .collect();
        ProgramInput {
            proofs,
            policy,
            wallet: a.wallet.clone().expect("--wallet required"),
            evaluated_at: 0,
            blinding: String::new(),
        }
    };
    if let Some(b) = &a.blinding {
        input.blinding = b.clone();
    } else if input.blinding.is_empty() {
        input.blinding = random_blinding();
        eprintln!("blinding (private, keep to open the commitment): {}", input.blinding);
    }
    if let Some(t) = a.at {
        input.evaluated_at = t;
    } else if input.evaluated_at == 0 {
        input.evaluated_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
    }
    input
}

fn random_blinding() -> String {
    use std::io::Read;
    let mut b = [0u8; 32];
    std::fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut b))
        .expect("read /dev/urandom");
    format!("0x{}", hex::encode(b))
}

fn read(p: &Path) -> String {
    std::fs::read_to_string(p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

fn stdin_for(input: &ProgramInput) -> SP1Stdin {
    let mut s = SP1Stdin::new();
    s.write(input);
    s
}

fn print_public(pv: &PublicValuesStruct) {
    println!("── public values (all a verifier learns) ──");
    println!("wallet            {}", pv.wallet);
    println!("industry          {}", pv.industry);
    println!("qualified / min   {} / {}", pv.qualifiedCount, pv.minCount);
    println!("passed            {}", pv.passed);
    println!("policy hash       {}", pv.policyHash);
    println!("cert set commit   {}", pv.certSetCommitment);
    println!("evaluated at      {}", pv.evaluatedAt);
}

#[tokio::main]
async fn main() {
    sp1_sdk::utils::setup_logger();
    match Cli::parse().cmd {
        Cmd::Check(a) => {
            let input = load_input(&a);
            match evaluate(&input) {
                Ok(e) => {
                    println!("── private credentials (never leave this machine) ──");
                    for (c, v) in &e.credentials {
                        let tag = match v {
                            Verdict::Counted => "COUNTED ",
                            Verdict::Duplicate => "dup     ",
                            Verdict::OtherIndustry => "other   ",
                            Verdict::Incomplete => "partial ",
                        };
                        println!("{tag} [{}] {} ({}%)", c.platform, c.title, c.completion_pct);
                    }
                    print_public(&e.public);
                }
                Err((i, err)) => {
                    eprintln!("proof #{i} rejected: {err}");
                    std::process::exit(1);
                }
            }
        }
        Cmd::Execute(a) => {
            let input = load_input(&a);
            let client = ProverClient::from_env().await;
            let (public, report) = client
                .execute(ELF, stdin_for(&input))
                .await
                .expect("program rejected the input");
            println!("executed in {} cycles", report.total_instruction_count());
            print_public(&decode_public(public.as_slice()).expect("decode public values"));
        }
        Cmd::Prove { input, system, out } => {
            let input = load_input(&input);
            let client = ProverClient::from_env().await;
            let pk = client.setup(ELF).await.expect("setup");
            let req = client.prove(&pk, stdin_for(&input));
            let proof = match system {
                System::Core => req.core().await,
                System::Compressed => req.compressed().await,
                System::Groth16 => req.groth16().await,
                System::Plonk => req.plonk().await,
            }
            .expect("proving failed");
            client.verify(&proof, pk.verifying_key(), None).expect("self-verify failed");
            proof.save(&out).expect("save proof");
            println!("proof written to {}", out.display());
            println!("program vkey      {}", pk.verifying_key().bytes32());
            if matches!(system, System::Groth16 | System::Plonk) {
                println!("onchain proof     0x{}", hex::encode(proof.bytes()));
            }
            print_public(&decode_public(proof.public_values.as_slice()).unwrap());
        }
        Cmd::Verify { proof } => {
            let proof = SP1ProofWithPublicValues::load(&proof).expect("load proof");
            let client = ProverClient::from_env().await;
            let pk = client.setup(ELF).await.expect("setup");
            client
                .verify(&proof, pk.verifying_key(), None)
                .expect("INVALID PROOF");
            println!("proof is VALID for program {}", pk.verifying_key().bytes32());
            print_public(&decode_public(proof.public_values.as_slice()).unwrap());
        }
        Cmd::Vkey => {
            let client = ProverClient::from_env().await;
            let pk = client.setup(ELF).await.expect("setup");
            println!("{}", pk.verifying_key().bytes32());
        }
    }
}
