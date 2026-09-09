use clap::{Parser, ValueEnum};

/// Convert a private key between hex/WIF and derive Bitcoin + Ethereum addresses.
#[derive(Parser)]
#[command(version)]
struct Args {
    /// Private key as raw hex (optionally 0x-prefixed) or a Bitcoin WIF.
    input: String,

    /// Network to assume when `input` is raw hex. Ignored for WIF input,
    /// which already encodes its own network.
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

fn main() {
    let args = Args::parse();

    let network = match args.network {
        NetworkArg::Mainnet => pkc_core::Network::Mainnet,
        NetworkArg::Testnet => pkc_core::Network::Testnet,
    };
    let format = if args.uncompressed {
        pkc_core::KeyFormat::Uncompressed
    } else {
        pkc_core::KeyFormat::Compressed
    };

    match pkc_core::convert(&args.input, network, format) {
        Ok(result) => {
            println!("Network:      {:?}", result.network);
            println!("Format:       {:?}", result.format);
            println!("Private key:  {}", result.private_key_hex);
            println!("WIF:          {}", result.wif);
            println!("BTC address:  {}", result.btc_address);
            println!("ETH address:  {}", result.eth_address);
        }
        Err(err) => {
            eprintln!("error: {err}");
            std::process::exit(1);
        }
    }
}
