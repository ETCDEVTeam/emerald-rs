//! # CLI wrapper for ethereum classic web3 like connector

#![cfg(feature = "cli")]

#![cfg_attr(feature = "dev", feature(plugin))]
#![cfg_attr(feature = "dev", plugin(clippy))]

#[macro_use]
extern crate log;

#[macro_use]
extern crate lazy_static;

extern crate docopt;
extern crate env_logger;
extern crate emerald;
extern crate rustc_serialize;
extern crate futures_cpupool;

use docopt::Docopt;
use emerald::storage::default_path;
use env_logger::LogBuilder;
use futures_cpupool::CpuPool;
use log::{LogLevel, LogLevelFilter};
use std::{env, fs, io};
use std::ffi::OsStr;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::process::*;
use std::sync::Arc;

const USAGE: &'static str = include_str!("../usage.txt");

const VERSION: Option<&'static str> = option_env!("CARGO_PKG_VERSION");

lazy_static! {
    static ref pool: Arc<CpuPool> = Arc::new(CpuPool::new_num_cpus());
}

#[derive(Debug, RustcDecodable)]
struct Args {
    flag_version: bool,
    flag_verbose: bool,
    flag_quiet: bool,
    flag_host: String,
    flag_port: String,
    flag_client_host: String,
    flag_client_port: String,
    flag_base_path: String,
}

enum Node_chain {
    MAINNET,
    TESTNET,
}

/// Launches  node in child process
fn launch_node<I, C>(cmd: C, args: I) -> io::Result<Child>
    where I: IntoIterator<Item = C>,
          C: AsRef<OsStr>
{
    Command::new(cmd)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::piped())
        .spawn()
}

/// Redirects log output of node into file
fn redirect_log(node: &mut Child, log_file: &mut fs::File) {
    pool.spawn_fn(move || io::copy(&mut node.stderr.unwrap(), log_file))
        .forget();
}

fn main() {
    env::set_var("RUST_BACKTRACE", "1");

    let mut log_builder = LogBuilder::new();

    log_builder.filter(None, LogLevelFilter::Info);

    if env::var("RUST_LOG").is_ok() {
        log_builder.parse(&env::var("RUST_LOG").unwrap());
    }

    log_builder.init().expect("Expect to initialize logger");

    let args: Args = Docopt::new(USAGE)
        .and_then(|d| d.decode())
        .unwrap_or_else(|e| e.exit());

    if args.flag_version {
        println!("v{}", VERSION.unwrap_or("unknown"));
        exit(0);
    }

    let addr = format!("{}:{}", args.flag_host, args.flag_port)
        .parse::<SocketAddr>()
        .expect("Expect to parse address");

    let client_addr = format!("{}:{}", args.flag_client_host, args.flag_client_port)
        .parse::<SocketAddr>()
        .expect("Expect to parse client address");

    let base_path_str = args.flag_base_path
        .parse::<String>()
        .expect("Expect to parse base path");

    let base_path = if !base_path_str.is_empty() {
        Some(PathBuf::from(&base_path_str))
    } else {
        None
    };

    if log_enabled!(LogLevel::Info) {
        info!("Starting Emerald Connector - v{}",
              VERSION.unwrap_or("unknown"));
    }

    let mut log = default_path();
    log.push("log");
    if fs::create_dir_all(log.as_path()).is_ok() {};

    log.push("geth_log.txt");
    let mut log_file = match fs::File::create(log.as_path()) {
        Ok(f) => f,
        Err(err) => {
            error!("Unable to open node log file: {}", err);
            exit(1);
        }
    };

    let mut np = default_path();
    np.push("bin");
    np.push("geth");

    let mut node = match launch_node(np.as_os_str(), &["--fast"]) {
        Ok(pr) => pr,
        Err(err) => {
            error!("Unable to launch Ethereum node: {}", err);
            exit(1);
        }
    };
    redirect_log(&mut node, &mut log_file);

    let restart_callback = |chain: String| {
        node.kill();
        node = match chain {
            "MAINNET" => {
                launch_node(np.as_os_str(), &["--testnet, --fast"])
                    .and_then(|&mut n| redirect_log(n, &mut log_file)).unwrap()
            }
            "TESTNET" => {
                launch_node(np.as_os_str(), &["--fast"]).and_then(|&mut n| redirect_log(n, &mut log_file)).unwrap()
            }
        }
    };

    emerald::rpc::start(&addr, &client_addr, base_path, restart_callback);
}
