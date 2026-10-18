use clap::{Args, Parser, Subcommand, ValueEnum};
use zeroize::Zeroizing;

#[derive(Parser)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Convert a raw private key between hex and WIF and derive its Bitcoin and Ethereum addresses.
    Convert(ConvertArgs),
    /// SLIP-39 Shamir secret sharing: split a recovery phrase (or a raw key) into backup shares, or recombine them.
    Slip39 {
        #[command(subcommand)]
        action: Slip39Command,
    },
}

#[derive(Args)]
struct ConvertArgs {
    /// Private key as raw hex (optionally 0x-prefixed) or a Bitcoin WIF.
    /// Omit this to be prompted instead (hidden input): passing it here
    /// exposes it via shell history and process listings.
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
    /// Split a BIP39 recovery phrase, or a raw private key, into SLIP-39 shares.
    /// For a single checksummed-words backup with no sharing, use
    /// --group-threshold 1 --group 1:1.
    Split(Slip39SplitArgs),
    /// Recombine SLIP-39 shares into the original recovery phrase or key.
    Combine(Slip39CombineArgs),
    /// Show which group/member a single share is, without needing enough shares to recover anything.
    Inspect(Slip39InspectArgs),
}

#[derive(Args)]
struct Slip39SplitArgs {
    /// A BIP39 recovery phrase (12-24 words), or a raw private key as hex or
    /// WIF. Omit this to be prompted instead (hidden input): passing it
    /// here exposes it via shell history and process listings.
    secret: Option<String>,

    /// Number of groups required to recover.
    #[arg(long)]
    group_threshold: u8,

    /// One group as "threshold:count" (e.g. "2:3"). Repeat for multiple groups.
    #[arg(long = "group", value_name = "THRESHOLD:COUNT", required = true)]
    groups: Vec<String>,

    /// Passphrase, given inline (printable ASCII only). SLIP-39 always
    /// encrypts, even with no passphrase: omitting this flag means "no
    /// passphrase", not "prompt for one". Exposes it via shell history and
    /// process listings; use --passphrase-prompt for a real passphrase.
    #[arg(long, conflicts_with = "passphrase_prompt")]
    passphrase: Option<String>,

    /// Prompt for the passphrase interactively (hidden input, asked twice)
    /// instead of passing it inline with --passphrase.
    #[arg(long)]
    passphrase_prompt: bool,

    /// Prompt for your wallet's BIP39 passphrase (the "25th word"), used only
    /// to show its real first addresses. It is not part of the backup: if
    /// your wallet uses one, it needs its own backup.
    #[arg(long)]
    bip39_passphrase_prompt: bool,

    /// PBKDF2 cost is 2500 << this value per round (0..=15). Higher is
    /// slower but more resistant to brute-forcing a guessed passphrase. The
    /// default, 1, matches the reference implementation and other tools.
    #[arg(long, default_value_t = 1)]
    iteration_exponent: u8,
}

#[derive(Args)]
struct Slip39CombineArgs {
    /// Mnemonic shares, enough to meet the threshold (quote each as one
    /// shell argument). Omit these to be prompted for them one at a time
    /// instead (hidden input): passing them here exposes them via shell
    /// history and process listings.
    mnemonics: Vec<String>,

    /// Passphrase, given inline. A wrong passphrase is NOT reported as an
    /// error: SLIP-39 decryption is intentionally unauthenticated, so it
    /// silently produces a different, wrong key: check the resulting
    /// addresses look right before trusting the result. Omitting this flag
    /// means "no passphrase"; exposes it via shell history and process
    /// listings if given inline: use --passphrase-prompt instead.
    #[arg(long, conflicts_with = "passphrase_prompt")]
    passphrase: Option<String>,

    /// Prompt for the passphrase interactively (hidden input) instead of
    /// passing it inline with --passphrase.
    #[arg(long)]
    passphrase_prompt: bool,

    /// Prompt for your wallet's BIP39 passphrase (the "25th word"), used only
    /// to show its real first addresses. It is not part of the backup: if
    /// your wallet uses one, it needs its own backup.
    #[arg(long)]
    bip39_passphrase_prompt: bool,
}

#[derive(Args)]
struct Slip39InspectArgs {
    /// A single mnemonic share (quote it as one shell argument). Omit this
    /// to be prompted instead (hidden input): passing it here exposes it
    /// via shell history and process listings.
    mnemonic: Option<String>,
}

fn parse_group(spec: &str) -> Result<rpb_slip39::GroupSpec, String> {
    let (threshold, count) = spec
        .split_once(':')
        .ok_or_else(|| format!("group \"{spec}\" must be in THRESHOLD:COUNT form, e.g. \"2:3\""))?;
    let member_threshold: u8 = threshold.parse().map_err(|_| format!("invalid threshold in group \"{spec}\""))?;
    let member_count: u8 = count.parse().map_err(|_| format!("invalid count in group \"{spec}\""))?;
    Ok(rpb_slip39::GroupSpec { member_threshold, member_count })
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
/// means "no passphrase": the common case, not treated as "prompt me".
/// `confirm` asks twice: when splitting, a typo would be on every share,
/// and recovery never reports a wrong passphrase.
fn resolve_passphrase(inline: Option<String>, use_prompt: bool, confirm: bool) -> Result<String, String> {
    if use_prompt {
        let passphrase = prompt_hidden("Passphrase: ")?;
        if confirm && *Zeroizing::new(prompt_hidden("Passphrase again: ")?) != passphrase {
            return Err("the two passphrases differ; nothing was split".to_string());
        }
        Ok(passphrase)
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
    let input = resolve_secret(args.input, "the private key", "Private key (hex or WIF): ")?;

    let network = match args.network {
        NetworkArg::Mainnet => rpb_core::Network::Mainnet,
        NetworkArg::Testnet => rpb_core::Network::Testnet,
    };
    let format = if args.uncompressed { rpb_core::KeyFormat::Uncompressed } else { rpb_core::KeyFormat::Compressed };

    let result = rpb_core::convert(&input, network, format).map_err(|e| e.to_string())?;
    println!("Network:      {:?}", result.network);
    println!("Format:       {:?}", result.format);
    println!("Private key:  {}", result.private_key_hex);
    println!("WIF:          {}", result.wif);
    println!("BTC address:  {}", result.btc_address);
    println!("ETH address:  {}", result.eth_address);
    Ok(())
}

/// What `slip39 split` backs up. Words are always a recovery phrase; a
/// single token is a raw private key (hex or WIF). SLIP-39 itself doesn't
/// record which, so the output says it out loud.
enum BackupInput {
    Phrase(rpb_core::Phrase),
    Key(rpb_core::PrivateKey),
}

fn parse_backup_input(input: &str) -> Result<BackupInput, String> {
    if input.split_whitespace().count() > 1 {
        rpb_core::Phrase::parse(input).map(BackupInput::Phrase).map_err(|e| e.to_string())
    } else {
        rpb_core::parse_private_key(input).map(|p| BackupInput::Key(p.key)).map_err(|e| e.to_string())
    }
}

fn resolve_bip39_passphrase(use_prompt: bool) -> Result<Zeroizing<String>, String> {
    Ok(Zeroizing::new(if use_prompt { prompt_hidden("BIP39 passphrase (25th word): ")? } else { String::new() }))
}

/// The fingerprint, and the first addresses for recognising the phrase.
fn print_phrase_check(phrase: &rpb_core::Phrase, bip39_passphrase: &str) {
    let addresses = phrase.addresses(bip39_passphrase);
    let which = if bip39_passphrase.is_empty() { "no BIP39 passphrase" } else { "with your BIP39 passphrase" };
    println!("Fingerprint:  {}", hex_encode(&phrase.fingerprint()));
    println!("First ETH address ({which}): {}", addresses.eth);
    println!("First BTC address ({which}): {}", addresses.btc);
}

fn run_slip39_split(args: Slip39SplitArgs) -> Result<(), String> {
    let input = Zeroizing::new(resolve_secret(
        args.secret,
        "the recovery phrase or key",
        "Recovery phrase, or private key (hex or WIF): ",
    )?);
    let passphrase = resolve_passphrase(args.passphrase, args.passphrase_prompt, true)?;
    let bip39_passphrase = resolve_bip39_passphrase(args.bip39_passphrase_prompt)?;
    let groups: Vec<rpb_slip39::GroupSpec> = args.groups.iter().map(|s| parse_group(s)).collect::<Result<_, _>>()?;

    let secret: Zeroizing<Vec<u8>> = match parse_backup_input(&input)? {
        BackupInput::Phrase(phrase) => {
            println!("Backing up a {}-word recovery phrase.", phrase.word_count());
            print_phrase_check(&phrase, &bip39_passphrase);
            println!("Write the fingerprint on every share: it shows the shares belong together, and a recovery must reproduce it.");
            phrase.entropy()
        }
        BackupInput::Key(key) => {
            println!("Backing up a raw private key, not a recovery phrase.");
            println!("ETH address: {}", rpb_core::eth_address(&key));
            Zeroizing::new(key.to_bytes().to_vec())
        }
    };

    let shares = rpb_slip39::split(&secret, args.group_threshold, &groups, &passphrase, args.iteration_exponent)
        .map_err(|e| e.to_string())?;

    println!();
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
    let passphrase = resolve_passphrase(args.passphrase, args.passphrase_prompt, false)?;
    let bip39_passphrase = resolve_bip39_passphrase(args.bip39_passphrase_prompt)?;

    let refs: Vec<&str> = mnemonics.iter().map(String::as_str).collect();
    let secret = rpb_slip39::combine(&refs, &passphrase).map_err(|e| e.to_string())?;

    // A backup doesn't record what it holds. A phrase-sized secret is shown
    // as a phrase; 32 bytes could also be a raw key, so it's shown both
    // ways: the fingerprint or address you recognise tells you which.
    let mut shown = false;
    if let Ok(phrase) = rpb_core::Phrase::from_entropy(&secret) {
        println!("As a recovery phrase ({} words): {}", phrase.word_count(), phrase.to_words().as_str());
        print_phrase_check(&phrase, &bip39_passphrase);
        shown = true;
    }
    if let Ok(bytes) = <[u8; 32]>::try_from(secret.as_slice()) {
        if let Ok(key) = rpb_core::PrivateKey::from_bytes(bytes) {
            println!("As a raw private key: {}", hex_encode(&secret));
            println!("ETH address: {}", rpb_core::eth_address(&key));
            shown = true;
        }
    }
    if !shown {
        println!("Secret ({} bits): {}", secret.len() * 8, hex_encode(&secret));
    }
    println!(
        "A wrong passphrase or a wrong share gives a different result, not an error: check the fingerprint or address."
    );
    Ok(())
}

fn run_slip39_inspect(args: Slip39InspectArgs) -> Result<(), String> {
    let mnemonic = resolve_secret(args.mnemonic, "the mnemonic share", "Mnemonic share: ")?;
    let info = rpb_slip39::inspect(&mnemonic).map_err(|e| e.to_string())?;
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
