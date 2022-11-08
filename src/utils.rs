use bytesize::ByteSize;
use color_eyre::eyre::Result;
use std::fs::create_dir_all;
use std::{
    path::{Path, PathBuf},
    str::FromStr,
};
use subspace_sdk::PublicKey;
use tracing::level_filters::LevelFilter;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_bunyan_formatter::{BunyanFormattingLayer, JsonStorageLayer};
use tracing_error::ErrorLayer;
use tracing_subscriber::prelude::*;
use tracing_subscriber::{fmt, fmt::format::FmtSpan, EnvFilter, Layer};

const KEEP_LAST_N_DAYS: usize = 7;

pub(crate) fn print_ascii_art() {
    println!("
 ____             __                                              __  __          __                               __
/\\  _`\\          /\\ \\                                            /\\ \\/\\ \\        /\\ \\__                           /\\ \\
\\ \\,\\L\\_\\  __  __\\ \\ \\____    ____  _____      __      ___     __\\ \\ `\\\\ \\     __\\ \\ ,_\\  __  __  __    ___   _ __\\ \\ \\/'\\
 \\/_\\__ \\ /\\ \\/\\ \\\\ \\ '__`\\  /',__\\/\\ '__`\\  /'__`\\   /'___\\ /'__`\\ \\ , ` \\  /'__`\\ \\ \\/ /\\ \\/\\ \\/\\ \\  / __`\\/\\`'__\\ \\ , <
   /\\ \\L\\ \\ \\ \\_\\ \\\\ \\ \\L\\ \\/\\__, `\\ \\ \\L\\ \\/\\ \\L\\.\\_/\\ \\__//\\  __/\\ \\ \\`\\ \\/\\  __/\\ \\ \\_\\ \\ \\_/ \\_/ \\/\\ \\L\\ \\ \\ \\/ \\ \\ \\\\`\\
   \\ `\\____\\ \\____/ \\ \\_,__/\\/\\____/\\ \\ ,__/\\ \\__/.\\_\\ \\____\\ \\____\\\\ \\_\\ \\_\\ \\____\\\\ \\__\\\\ \\___x___/'\\ \\____/\\ \\_\\  \\ \\_\\ \\_\\
    \\/_____/\\/___/   \\/___/  \\/___/  \\ \\ \\/  \\/__/\\/_/\\/____/\\/____/ \\/_/\\/_/\\/____/ \\/__/ \\/__//__/   \\/___/  \\/_/   \\/_/\\/_/
                                      \\ \\_\\
                                       \\/_/
");
}

pub(crate) fn print_version() {
    let version: &str = env!("CARGO_PKG_VERSION");
    println!("version: {version}");
}

pub(crate) fn get_user_input(
    prompt: &str,
    default_value: Option<&str>,
    condition: fn(input: &str) -> bool,
    error_msg: &str,
) -> Result<String> {
    let user_input = loop {
        print!("{prompt}");
        std::io::Write::flush(&mut std::io::stdout())?;
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        let user_input = input.trim().to_string();

        if condition(&user_input) {
            break user_input;
        }
        if let Some(default) = default_value && user_input.is_empty() {
            break default.to_string();
        }

        println!("{error_msg}");
    };

    Ok(user_input)
}

pub(crate) fn is_valid_node_name(node_name: &str) -> bool {
    node_name.is_ascii() && !node_name.trim().is_empty()
}

pub(crate) fn is_valid_address(address: &str) -> bool {
    PublicKey::from_str(address).is_ok()
}

pub(crate) fn is_valid_location(location: &str) -> bool {
    Path::new(location).is_dir()
}

pub(crate) fn is_valid_size(size: &str) -> bool {
    size.parse::<ByteSize>().is_ok()
}

pub(crate) fn is_valid_chain(chain: &str) -> bool {
    // TODO: instead of a hardcoded list, get the chain names from telemetry
    let chain_list = vec!["gemini-2a", "dev"];
    chain_list.contains(&chain)
}

pub(crate) fn plot_location_getter() -> PathBuf {
    dirs::data_dir().unwrap().join("subspace-cli").join("plots")
}

pub(crate) fn node_directory_getter() -> PathBuf {
    dirs::data_dir().unwrap().join("subspace-cli").join("node")
}

fn custom_log_dir() -> PathBuf {
    let id = "subspace-cli";

    #[cfg(target_os = "macos")]
    let path = dirs::home_dir().map(|dir| dir.join("Library/Logs").join(id));
    // evaluates to: `~/Library/Logs/${bundle_name}/

    #[cfg(target_os = "linux")]
    let path = dirs::data_local_dir().map(|dir| dir.join(id).join("logs"));
    // evaluates to: `~/.local/share/${bundle_name}/logs/

    #[cfg(target_os = "windows")]
    let path = dirs::data_local_dir().map(|dir| dir.join(id).join("logs"));
    // evaluates to: `C:/Users/Username/AppData/Local/${bundle_name}/logs/

    path.expect("Could not resolve custom log directory path!")
}

pub(crate) fn support_message() -> String {
    format!(
        "This is a bug, please submit it to our forums: {}",
        ansi_term::Style::new()
            .underline()
            .paint("https://forum.subspace.network")
    )
}

pub(crate) fn install_tracing(is_verbose: bool) {
    let log_dir = custom_log_dir();
    let _ = create_dir_all(log_dir.clone());

    let file_appender = RollingFileAppender::builder()
        .max_log_files(KEEP_LAST_N_DAYS)
        .rotation(Rotation::DAILY)
        .filename_prefix("subspace-cli.log")
        .build(log_dir)
        .expect("building should always succeed");

    // filter for logging
    let filter = || {
        EnvFilter::builder()
            .with_default_directive(LevelFilter::INFO.into())
            .from_env_lossy()
            .add_directive(
                "subspace_cli=info"
                    .parse()
                    .expect("hardcoded value is true"),
            )
            .add_directive("regalloc2=off".parse().expect("hardcoded value is true"))
    };

    // start logger, after we acquire the bundle identifier
    let tracing_layer = tracing_subscriber::registry()
        .with(
            BunyanFormattingLayer::new("subspace-cli".to_owned(), file_appender)
                .and_then(JsonStorageLayer)
                .with_filter(filter()),
        )
        .with(ErrorLayer::default());

    // if verbose, then also print to stdout
    if is_verbose {
        tracing_layer
            .with(
                fmt::layer()
                    .with_ansi(!cfg!(windows))
                    .with_span_events(FmtSpan::CLOSE)
                    .with_filter(filter()),
            )
            .init();
    } else {
        tracing_layer.init();
    }
}