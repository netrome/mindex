use clap::{Args, Parser, Subcommand};
use std::{net::SocketAddr, path::PathBuf};

#[tokio::main]
async fn main() {
    let Cli {
        command,
        root,
        app_name,
        icon_192,
        icon_512,
        vapid_private_key,
        vapid_public_key,
        vapid_subject,
    } = Cli::parse();

    if let Some(Command::Init(args)) = command {
        run_init(args);
        return;
    }

    let root = root.unwrap_or_else(|| {
        eprintln!("error: --root is required unless using a subcommand");
        std::process::exit(2);
    });
    let root = std::fs::canonicalize(&root)
        .unwrap_or_else(|err| panic!("failed to resolve root directory: {err}"));
    if !root.is_dir() {
        panic!("root path is not a directory: {}", root.display());
    }
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));

    println!("listening on http://{addr}");

    let config = mindex::config::AppConfig {
        root,
        app_name,
        icon_192,
        icon_512,
        vapid_private_key,
        vapid_public_key,
        vapid_subject,
    };
    mindex::serve(addr, config).await;
}

#[derive(Parser, Debug)]
#[command(
    name = "mindex",
    version,
    about = "Small markdown knowledge base server"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
    #[arg(long)]
    root: Option<PathBuf>,
    #[arg(long, default_value = "Mindex")]
    app_name: String,
    #[arg(long)]
    icon_192: Option<PathBuf>,
    #[arg(long)]
    icon_512: Option<PathBuf>,
    #[arg(long, env = "MINDEX_VAPID_PRIVATE_KEY")]
    vapid_private_key: Option<String>,
    #[arg(long, env = "MINDEX_VAPID_PUBLIC_KEY")]
    vapid_public_key: Option<String>,
    #[arg(long, env = "MINDEX_VAPID_SUBJECT")]
    vapid_subject: Option<String>,
}

#[derive(Subcommand, Debug)]
enum Command {
    Init(InitArgs),
}

#[derive(Args, Debug)]
struct InitArgs {
    #[arg(long)]
    subject: Option<String>,
}

fn run_init(args: InitArgs) {
    let credentials = match mindex::generate_vapid_credentials() {
        Ok(credentials) => credentials,
        Err(err) => {
            eprintln!("failed to generate VAPID credentials: {err}");
            std::process::exit(1);
        }
    };
    let (subject, show_subject_note) = match args.subject {
        Some(subject) => (subject, false),
        None => ("mailto:you@example.com".to_string(), true),
    };

    println!("VAPID credentials generated.");
    println!();
    println!("MINDEX_VAPID_PRIVATE_KEY=\"{}\"", credentials.private_key);
    println!("MINDEX_VAPID_PUBLIC_KEY=\"{}\"", credentials.public_key);
    println!("MINDEX_VAPID_SUBJECT=\"{subject}\"");
    if show_subject_note {
        println!();
        println!("Note: replace MINDEX_VAPID_SUBJECT with a contact URI you control.");
    }
    println!();
    println!(
        "--vapid-private-key \"{}\" --vapid-public-key \"{}\" --vapid-subject \"{subject}\"",
        credentials.private_key, credentials.public_key
    );
}
