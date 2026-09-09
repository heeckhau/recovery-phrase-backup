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
    /// 24-word BIP39 mnemonic (quote it as one shell argument).
    input: String,

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
}

#[derive(Args)]
struct Slip39SplitArgs {
    /// Private key as raw hex, WIF, or a 24-word BIP39 mnemonic.
    private_key: String,

    /// Number of groups required to reconstruct the key.
    #[arg(long)]
    group_threshold: u8,

    /// One group as "threshold:count" (e.g. "2:3"). Repeat for multiple groups.
    #[arg(long = "group", value_name = "THRESHOLD:COUNT", required = true)]
    groups: Vec<String>,

    /// Optional passphrase (printable ASCII only). SLIP-39 always encrypts,
    /// even with no passphrase — this is not the same as leaving it unset.
    #[arg(long, default_value = "")]
    passphrase: String,

    /// PBKDF2 cost is 2500 << this value (0..=15). Higher is slower but
    /// more resistant to brute-forcing a guessed passphrase.
    #[arg(long, default_value_t = 0)]
    iteration_exponent: u8,
}

#[derive(Args)]
struct Slip39CombineArgs {
    /// Mnemonic shares, enough to meet the threshold (quote each as one
    /// shell argument).
    #[arg(required = true, num_args = 1..)]
    mnemonics: Vec<String>,

    /// Passphrase used when the shares were created. A wrong passphrase is
    /// NOT reported as an error: SLIP-39 decryption is intentionally
    /// unauthenticated, so it silently produces a different, wrong key —
    /// check the resulting addresses look right before trusting the result.
    #[arg(long, default_value = "")]
    passphrase: String,
}

fn parse_group(spec: &str) -> Result<pkc_slip39::GroupSpec, String> {
    let (threshold, count) = spec
        .split_once(':')
        .ok_or_else(|| format!("group \"{spec}\" must be in THRESHOLD:COUNT form, e.g. \"2:3\""))?;
    let member_threshold: u8 = threshold.parse().map_err(|_| format!("invalid threshold in group \"{spec}\""))?;
    let member_count: u8 = count.parse().map_err(|_| format!("invalid count in group \"{spec}\""))?;
    Ok(pkc_slip39::GroupSpec { member_threshold, member_count })
}

fn run_convert(args: ConvertArgs) -> Result<(), String> {
    let network = match args.network {
        NetworkArg::Mainnet => pkc_core::Network::Mainnet,
        NetworkArg::Testnet => pkc_core::Network::Testnet,
    };
    let format = if args.uncompressed { pkc_core::KeyFormat::Uncompressed } else { pkc_core::KeyFormat::Compressed };

    let result = pkc_core::convert(&args.input, network, format).map_err(|e| e.to_string())?;
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
    let key = pkc_core::parse_private_key(&args.private_key).map_err(|e| e.to_string())?.key;
    let groups: Vec<pkc_slip39::GroupSpec> = args.groups.iter().map(|s| parse_group(s)).collect::<Result<_, _>>()?;

    let shares = pkc_slip39::split(&key.to_bytes(), args.group_threshold, &groups, &args.passphrase, args.iteration_exponent)
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
    let refs: Vec<&str> = args.mnemonics.iter().map(String::as_str).collect();
    let secret = pkc_slip39::combine(&refs, &args.passphrase).map_err(|e| e.to_string())?;
    println!("Private key: {}", hex_encode(&secret));
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
    };

    if let Err(err) = result {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}
