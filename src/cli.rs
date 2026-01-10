use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;
use time::Duration;

const DEFAULT_AUTH_COOKIE_NAME: &str = "mindex_auth";

#[allow(clippy::large_enum_variant)]
pub(crate) enum RunOutcome {
    Serve(mindex::config::AppConfig),
    Exit(i32),
}

pub(crate) fn run() -> RunOutcome {
    let cli = Cli::parse();
    if let Some(Command::Init(args)) = cli.command {
        let code = run_init(args);
        return RunOutcome::Exit(code);
    }
    if let Some(Command::AuthKey) = cli.command {
        let code = run_auth_key();
        return RunOutcome::Exit(code);
    }

    let root = match cli.root.as_ref() {
        Some(root) => root.clone(),
        None => {
            eprintln!("error: --root is required unless using a subcommand");
            return RunOutcome::Exit(2);
        }
    };
    let root = std::fs::canonicalize(&root)
        .unwrap_or_else(|err| panic!("failed to resolve root directory: {err}"));
    if !root.is_dir() {
        panic!("root path is not a directory: {}", root.display());
    }

    let auth = match resolve_auth_config(&cli) {
        Ok(auth) => auth,
        Err(err) => {
            eprintln!("error: {err}");
            return RunOutcome::Exit(2);
        }
    };

    RunOutcome::Serve(mindex::config::AppConfig {
        root,
        app_name: cli.app_name,
        icon_192: cli.icon_192,
        icon_512: cli.icon_512,
        vapid_private_key: cli.vapid_private_key,
        vapid_public_key: cli.vapid_public_key,
        vapid_subject: cli.vapid_subject,
        auth,
    })
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
    #[arg(long, env = "MINDEX_AUTH_KEY")]
    auth_key: Option<String>,
    #[arg(long, env = "MINDEX_AUTH_TOKEN_TTL")]
    auth_token_ttl: Option<String>,
    #[arg(long, env = "MINDEX_AUTH_COOKIE_NAME")]
    auth_cookie_name: Option<String>,
    #[arg(long, env = "MINDEX_AUTH_COOKIE_SECURE")]
    auth_cookie_secure: bool,
}

#[derive(Subcommand, Debug)]
enum Command {
    Init(InitArgs),
    AuthKey,
}

#[derive(Args, Debug)]
struct InitArgs {
    #[arg(long)]
    subject: Option<String>,
}

fn run_init(args: InitArgs) -> i32 {
    let credentials = match mindex::generate_vapid_credentials() {
        Ok(credentials) => credentials,
        Err(err) => {
            eprintln!("failed to generate VAPID credentials: {err}");
            return 1;
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
    0
}

fn run_auth_key() -> i32 {
    let secret = match mindex::auth::generate_auth_key() {
        Ok(secret) => secret,
        Err(err) => {
            eprintln!("failed to generate auth key: {err}");
            return 1;
        }
    };
    println!("{secret}");
    0
}

fn resolve_auth_config(cli: &Cli) -> Result<Option<mindex::config::AuthConfig>, String> {
    let has_any = cli.auth_key.is_some()
        || cli.auth_token_ttl.is_some()
        || cli.auth_cookie_name.is_some()
        || cli.auth_cookie_secure;

    if !has_any {
        return Ok(None);
    }

    let auth_key = cli
        .auth_key
        .as_ref()
        .ok_or("auth is configured but --auth-key is missing")?
        .trim();
    if auth_key.is_empty() {
        return Err("auth key cannot be empty".to_string());
    }

    if let Some(name) = cli.auth_cookie_name.as_deref()
        && name.trim().is_empty()
    {
        return Err("auth cookie name cannot be empty".to_string());
    }

    let token_ttl = match cli.auth_token_ttl.as_deref() {
        Some(raw) => parse_auth_token_ttl(raw)?,
        None => default_auth_token_ttl(),
    };
    let cookie_name = cli
        .auth_cookie_name
        .as_deref()
        .map(|name| name.trim().to_string())
        .unwrap_or_else(|| DEFAULT_AUTH_COOKIE_NAME.to_string());

    Ok(Some(mindex::config::AuthConfig {
        key: auth_key.to_string(),
        token_ttl,
        cookie_name,
        cookie_secure: cli.auth_cookie_secure,
    }))
}

fn default_auth_token_ttl() -> Duration {
    Duration::days(14)
}

fn parse_auth_token_ttl(raw: &str) -> Result<Duration, String> {
    let value = raw.trim();
    if value.is_empty() {
        return Err("auth token ttl cannot be empty".to_string());
    }

    let (amount, unit) = match value.chars().last() {
        Some(ch) if ch.is_ascii_alphabetic() => {
            (&value[..value.len() - 1], ch.to_ascii_lowercase())
        }
        _ => (value, 's'),
    };

    let amount: i64 = amount
        .parse()
        .map_err(|_| format!("invalid auth token ttl '{value}'; expected <number>[s|m|h|d]"))?;

    if amount <= 0 {
        return Err("auth token ttl must be greater than 0".to_string());
    }

    match unit {
        's' => Ok(Duration::seconds(amount)),
        'm' => Ok(Duration::minutes(amount)),
        'h' => Ok(Duration::hours(amount)),
        'd' => Ok(Duration::days(amount)),
        _ => Err(format!(
            "invalid auth token ttl '{value}'; expected <number>[s|m|h|d]"
        )),
    }
}

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::*;

    fn base_cli() -> Cli {
        Cli {
            command: None,
            root: Some(PathBuf::from("/")),
            app_name: "Mindex".to_string(),
            icon_192: None,
            icon_512: None,
            vapid_private_key: None,
            vapid_public_key: None,
            vapid_subject: None,
            auth_key: None,
            auth_token_ttl: None,
            auth_cookie_name: None,
            auth_cookie_secure: false,
        }
    }

    #[test]
    fn parse_auth_token_ttl__should_parse_seconds_when_unit_missing() {
        // When
        let duration = parse_auth_token_ttl("30").expect("parse ttl");

        // Then
        assert_eq!(duration, Duration::seconds(30));
    }

    #[test]
    fn parse_auth_token_ttl__should_parse_units() {
        // When
        let duration = parse_auth_token_ttl("15m").expect("parse ttl");

        // Then
        assert_eq!(duration, Duration::minutes(15));
    }

    #[test]
    fn parse_auth_token_ttl__should_reject_invalid_values() {
        // Then
        assert!(parse_auth_token_ttl("").is_err());
        assert!(parse_auth_token_ttl("0").is_err());
        assert!(parse_auth_token_ttl("abc").is_err());
    }

    #[test]
    fn resolve_auth_config__should_require_auth_key_when_options_present() {
        // Given
        let mut cli = base_cli();
        cli.auth_token_ttl = Some("1h".to_string());

        // When
        let result = resolve_auth_config(&cli);

        // Then
        assert!(result.is_err());
    }

    #[test]
    fn resolve_auth_config__should_apply_defaults_when_auth_key_present() {
        // Given
        let mut cli = base_cli();
        cli.auth_key = Some("base64-key".to_string());

        // When
        let config = resolve_auth_config(&cli)
            .expect("resolve auth config")
            .expect("auth config");

        // Then
        assert_eq!(config.key, "base64-key");
        assert_eq!(config.token_ttl, default_auth_token_ttl());
        assert_eq!(config.cookie_name, DEFAULT_AUTH_COOKIE_NAME);
        assert!(!config.cookie_secure);
    }
}
