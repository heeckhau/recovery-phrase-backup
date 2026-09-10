use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Parser)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Convert a private key between hex/WIF/mnemonic and derive Bitcoin + Ethereum addresses.
    Convert(ConvertArgs),
    /// SLIP-39 Shamir secret sharing: split a key into backup shares, or recombine them.
    Slip39 {
        #[command(subcommand)]
        action: Slip39Command,
    },
}

#[derive(Args)]
struct ConvertArgs {
    /// Private key as raw hex (optionally 0x-prefixed), a Bitcoin WIF, or a
    /// 24-word BIP39 mnemonic. Omit this to be prompted instead (hidden
    /// input) — passing it here exposes it via shell history and process
    /// listings.
    input: Option<String>,

    /// Network to assume when `input` is raw hex or a mnemonic. Ignored for
    /// WIF input, which already encodes its own network.
    #[arg(long, value_enum, default_value_t = NetworkArg::Mainnet)]
    network: NetworkArg,

    /// Use an uncompressed key/address. Ignored for WIF input.
    #[arg(long)]
    uncompressed: bool,
}

#[derive(Clone, Copy, ValueEnum)]
enum NetworkArg {
    Mainnet,
    Testnet,
}

impl std::fmt::Display for NetworkArg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            NetworkArg::Mainnet => "mainnet",
            NetworkArg::Testnet => "testnet",
        })
    }
}

#[derive(Subcommand)]
enum Slip39Command {
    /// Split a private key into SLIP-39 mnemonic shares.
    Split(Slip39SplitArgs),
    /// Recombine SLIP-39 mnemonic shares into the original private key.
    Combine(Slip39CombineArgs),
    /// Show which group/member a single share is, without needing enough shares to recover anything.
    Inspect(Slip39InspectArgs),
}

#[derive(Args)]
struct Slip39SplitArgs {
    /// Private key as raw hex, WIF, or a 24-word BIP39 mnemonic. Omit this
    /// to be prompted instead (hidden input) — passing it here exposes it
    /// via shell history and process listings.
    private_key: Option<String>,

    /// Number of groups required to reconstruct the key.
    #[arg(long)]
    group_threshold: u8,

    /// One group as "threshold:count" (e.g. "2:3"). Repeat for multiple groups.
    #[arg(long = "group", value_name = "THRESHOLD:COUNT", required = true)]
    groups: Vec<String>,

    /// Passphrase, given inline (printable ASCII only). SLIP-39 always
    /// encrypts, even with no passphrase — omitting this flag means "no
    /// passphrase", not "prompt for one". Exposes it via shell history and
    /// process listings; use --passphrase-prompt for a real passphrase.
    #[arg(long, conflicts_with = "passphrase_prompt")]
    passphrase: Option<String>,

    /// Prompt for the passphrase interactively (hidden input) instead of
    /// passing it inline with --passphrase.
    #[arg(long)]
    passphrase_prompt: bool,

    /// PBKDF2 cost is 2500 << this value (0..=15). Higher is slower but
    /// more resistant to brute-forcing a guessed passphrase.
    #[arg(long, default_value_t = 0)]
    iteration_exponent: u8,
}

#[derive(Args)]
struct Slip39CombineArgs {
    /// Mnemonic shares, enough to meet the threshold (quote each as one
    /// shell argument). Omit these to be prompted for them one at a time
    /// instead (hidden input) — passing them here exposes them via shell
    /// history and process listings.
    mnemonics: Vec<String>,

    /// Passphrase, given inline. A wrong passphrase is NOT reported as an
    /// error: SLIP-39 decryption is intentionally unauthenticated, so it
    /// silently produces a different, wrong key — check the resulting
    /// addresses look right before trusting the result. Omitting this flag
    /// means "no passphrase"; exposes it via shell history and process
    /// listings if given inline — use --passphrase-prompt instead.
    #[arg(long, conflicts_with = "passphrase_prompt")]
    passphrase: Option<String>,

    /// Prompt for the passphrase interactively (hidden input) instead of
    /// passing it inline with --passphrase.
    #[arg(long)]
    passphrase_prompt: bool,
}

#[derive(Args)]
struct Slip39InspectArgs {
    /// A single mnemonic share (quote it as one shell argument). Omit this
    /// to be prompted instead (hidden input) — passing it here exposes it
    /// via shell history and process listings.
    mnemonic: Option<String>,
}

fn parse_group(spec: &str) -> Result<pkc_slip39::GroupSpec, String> {
    let (threshold, count) = spec
        .split_once(':')
        .ok_or_else(|| format!("group \"{spec}\" must be in THRESHOLD:COUNT form, e.g. \"2:3\""))?;
    let member_threshold: u8 = threshold.parse().map_err(|_| format!("invalid threshold in group \"{spec}\""))?;
    let member_count: u8 = count.parse().map_err(|_| format!("invalid count in group \"{spec}\""))?;
    Ok(pkc_slip39::GroupSpec { member_threshold, member_count })
}

fn warn_argv_secret(what: &str) {
    eprintln!(
        "warning: passing {what} as a command-line argument exposes it via shell history and \
         process listings; omit it to be prompted instead"
    );
}

fn prompt_hidden(prompt: &str) -> Result<String, String> {
    rpassword::prompt_password(prompt).map_err(|e| format!("failed to read input: {e}"))
}

/// Resolves a required secret positional argument: uses it (with a warning)
/// if given inline, otherwise prompts for it with hidden input.
fn resolve_secret(provided: Option<String>, what: &str, prompt: &str) -> Result<String, String> {
    match provided {
        Some(value) => {
            warn_argv_secret(what);
            Ok(value)
        }
        None => prompt_hidden(prompt),
    }
}

/// Resolves a passphrase: `--passphrase-prompt` prompts with hidden input,
/// `--passphrase <value>` uses it inline (with a warning), and neither
/// means "no passphrase" — the common case, not treated as "prompt me".
fn resolve_passphrase(inline: Option<String>, use_prompt: bool) -> Result<String, String> {
    if use_prompt {
        prompt_hidden("Passphrase: ")
    } else if let Some(value) = inline {
        warn_argv_secret("the passphrase");
        Ok(value)
    } else {
        Ok(String::new())
    }
}

/// Resolves the list of mnemonic shares for `combine`: uses them (with one
/// warning) if given inline, otherwise prompts for them one at a time
/// (hidden input, blank line to finish).
fn resolve_mnemonics(provided: Vec<String>) -> Result<Vec<String>, String> {
    if !provided.is_empty() {
        warn_argv_secret("mnemonic shares");
        return Ok(provided);
    }

    println!("Enter mnemonic shares one at a time (blank line when done):");
    let mut shares = Vec::new();
    loop {
        let share = prompt_hidden(&format!("Share {}: ", shares.len() + 1))?;
        if share.trim().is_empty() {
            break;
        }
        shares.push(share);
    }
    Ok(shares)
}

fn run_convert(args: ConvertArgs) -> Result<(), String> {
    let input = resolve_secret(args.input, "the private key", "Private key (hex, WIF, or mnemonic): ")?;

    let network = match args.network {
        NetworkArg::Mainnet => pkc_core::Network::Mainnet,
        NetworkArg::Testnet => pkc_core::Network::Testnet,
    };
    let format = if args.uncompressed { pkc_core::KeyFormat::Uncompressed } else { pkc_core::KeyFormat::Compressed };

    let result = pkc_core::convert(&input, network, format).map_err(|e| e.to_string())?;
    println!("Network:      {:?}", result.network);
    println!("Format:       {:?}", result.format);
    println!("Private key:  {}", result.private_key_hex);
    println!("WIF:          {}", result.wif);
    println!("BTC address:  {}", result.btc_address);
    println!("ETH address:  {}", result.eth_address);
    println!("Mnemonic:     {}", result.mnemonic);
    Ok(())
}

fn run_slip39_split(args: Slip39SplitArgs) -> Result<(), String> {
    let private_key = resolve_secret(args.private_key, "the private key", "Private key (hex, WIF, or mnemonic): ")?;
    let passphrase = resolve_passphrase(args.passphrase, args.passphrase_prompt)?;

    let key = pkc_core::parse_private_key(&private_key).map_err(|e| e.to_string())?.key;
    let groups: Vec<pkc_slip39::GroupSpec> = args.groups.iter().map(|s| parse_group(s)).collect::<Result<_, _>>()?;

    let shares = pkc_slip39::split(&key.to_bytes(), args.group_threshold, &groups, &passphrase, args.iteration_exponent)
        .map_err(|e| e.to_string())?;

    for (i, group) in shares.iter().enumerate() {
        println!("Group {i}:");
        for (j, mnemonic) in group.iter().enumerate() {
            println!("  Member {j}: {mnemonic}");
        }
    }
    Ok(())
}

fn run_slip39_combine(args: Slip39CombineArgs) -> Result<(), String> {
    let mnemonics = resolve_mnemonics(args.mnemonics)?;
    let passphrase = resolve_passphrase(args.passphrase, args.passphrase_prompt)?;

    let refs: Vec<&str> = mnemonics.iter().map(String::as_str).collect();
    let secret = pkc_slip39::combine(&refs, &passphrase).map_err(|e| e.to_string())?;
    println!("Private key: {}", hex_encode(&secret));
    Ok(())
}

fn run_slip39_inspect(args: Slip39InspectArgs) -> Result<(), String> {
    let mnemonic = resolve_secret(args.mnemonic, "the mnemonic share", "Mnemonic share: ")?;
    let info = pkc_slip39::inspect(&mnemonic).map_err(|e| e.to_string())?;
    println!("Group:           {} of {}", info.group_index + 1, info.group_count);
    println!("Member:          {} (this group needs {})", info.member_index + 1, info.member_threshold);
    println!("Groups required: {} of {}", info.group_threshold, info.group_count);
    Ok(())
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Command::Convert(args) => run_convert(args),
        Command::Slip39 { action: Slip39Command::Split(args) } => run_slip39_split(args),
        Command::Slip39 { action: Slip39Command::Combine(args) } => run_slip39_combine(args),
        Command::Slip39 { action: Slip39Command::Inspect(args) } => run_slip39_inspect(args),
    };

    if let Err(err) = result {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}
