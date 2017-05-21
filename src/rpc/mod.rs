//! # JSON RPC module

mod http;
mod serialize;
mod error;

pub use self::error::Error;
use super::contract::Contracts;
use super::core::{self, Address, Transaction};
use super::keystore::KeyFile;
use super::storage::{ChainStorage, Storages, default_path};
use super::util::{ToHex, align_bytes, to_arr, to_u64, trim_hex};
use futures;
use jsonrpc_core::{Error as JsonRpcError, ErrorCode, MetaIoHandler, Metadata, Params};
use jsonrpc_core::futures::Future;
use jsonrpc_minihttp_server::{DomainsValidation, Req, ServerBuilder, cors};
use log::LogLevel;
use rustc_serialize::json;
use serde_json::Value;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::sync::mpsc::Sender;

/// RPC methods
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum Method {
    /// [web3_clientVersion](https://github.com/ethereum/wiki/wiki/JSON-RPC#web3_clientversion)
    ClientVersion,

    /// [eth_syncing](https://github.com/ethereum/wiki/wiki/JSON-RPC#eth_syncing)
    EthSyncing,

    /// [eth_blockNumber](https://github.com/ethereum/wiki/wiki/JSON-RPC#eth_blocknumber)
    EthBlockNumber,

    /// [eth_accounts](https://github.com/ethereum/wiki/wiki/JSON-RPC#eth_accounts)
    EthAccounts,

    /// [eth_getBalance](https://github.com/ethereum/wiki/wiki/JSON-RPC#eth_getbalance)
    EthGetBalance,

    /// [eth_getTransactionCount](
    /// https://github.com/ethereum/wiki/wiki/JSON-RPC#eth_gettransactioncount)
    EthGetTxCount,

    /// [eth_sendRawTransaction](
    /// https://github.com/paritytech/parity/wiki/JSONRPC-eth-module#eth_sendrawtransaction)
    EthSendRawTransaction,

    /// [eth_call](https://github.com/ethereum/wiki/wiki/JSON-RPC#eth_call)
    EthCall,

    /// `trace_call`
    EthTraceCall,

    /// [eth_getTransactionByHash](
    /// https://github.com/ethereumproject/wiki/wiki/JSON-RPC#eth_gettransactionbyhash)
    GetTxByHash,

    /// import account
    ImportAccountCall,

    /// creates new account
    PersonalNewAccount,
}

/// RPC method's request metadata
#[derive(Clone, Debug)]
enum MethodMetadata {
    /// Nothing special
    None,
    /// Given account passphrase
    Passphrase(String),
}

impl MethodMetadata {
    fn with_authorization(str: &str) -> MethodMetadata {
        MethodMetadata::Passphrase(str.to_string())
    }
}

impl Default for MethodMetadata {
    fn default() -> Self {
        MethodMetadata::None
    }
}

impl Metadata for MethodMetadata {}

/// PRC method's parameters
#[derive(Clone, Debug, PartialEq)]
pub struct MethodParams<'a>(pub Method, pub &'a Params);

/// Start an HTTP RPC endpoint
pub fn start(addr: &SocketAddr,
             client_addr: &SocketAddr,
             base_path: Option<PathBuf>,
             node_sender: Sender<String>) {
    let mut io = MetaIoHandler::default();

    let url = Arc::new(http::AsyncWrapper::new(&format!("http://{}", client_addr)));
    let node = Arc::new(Mutex::new(node_sender));

    {
        let url = url.clone();

        io.add_async_method("web3_clientVersion",
                            move |p| url.request(&MethodParams(Method::ClientVersion, &p)));
    }

    {
        let url = url.clone();

        io.add_async_method("eth_syncing",
                            move |p| url.request(&MethodParams(Method::EthSyncing, &p)));
    }

    {
        let url = url.clone();

        io.add_async_method("eth_blockNumber",
                            move |p| url.request(&MethodParams(Method::EthBlockNumber, &p)));
    }

    {
        let url = url.clone();

        io.add_async_method("eth_accounts",
                            move |p| url.request(&MethodParams(Method::EthAccounts, &p)));
    }

    {
        let url = url.clone();

        io.add_async_method("eth_getBalance",
                            move |p| url.request(&MethodParams(Method::EthGetBalance, &p)));
    }

    {
        let url = url.clone();

        io.add_async_method("eth_getTransactionCount",
                            move |p| url.request(&MethodParams(Method::EthGetTxCount, &p)));
    }

    {
        let url = url.clone();

        io.add_async_method("eth_getTransactionByHash",
                            move |p| url.request(&MethodParams(Method::GetTxByHash, &p)));
    }

    {
        let url = url.clone();
        let callback = move |p: Params, m| if let MethodMetadata::Passphrase(ref passphrase) = m {
            let pk = KeyFile::default().decrypt_key(passphrase);
            match Transaction::try_from(&p) {
                Ok(tr) => {
                    url.request(&MethodParams(Method::EthSendRawTransaction,
                                              &tr.to_raw_params(pk.unwrap())))
                }
                Err(err) => {
                    futures::done(Err(JsonRpcError::invalid_params(err.to_string()))).boxed()
                }
            }
        } else {
            futures::failed(JsonRpcError::invalid_request()).boxed()
        };
        io.add_method_with_meta("eth_sendTransaction", callback);
    }

    {
        let url = url.clone();

        io.add_async_method("eth_sendRawTransaction",
                            move |p| url.request(&MethodParams(Method::EthSendRawTransaction, &p)));
    }

    {
        let url = url.clone();

        io.add_async_method("eth_call",
                            move |p| url.request(&MethodParams(Method::EthCall, &p)));
    }

    {
        let url = url.clone();

        io.add_async_method("eth_traceCall",
                            move |p| url.request(&MethodParams(Method::EthTraceCall, &p)));
    }

    {
        let import_callback = move |p| match Params::parse::<Value>(p) {
            Ok(ref v) if v.as_str().is_some() => {
                let str = v.as_str().unwrap();
                match json::decode::<KeyFile>(str) {
                    Ok(kf) => {
                        let addr = Address::default().to_string();
                        match kf.flush(&default_path(), None) {
                            Ok(_) => futures::done(Ok(Value::String(addr))).boxed(),
                            Err(_) => futures::done(Err(JsonRpcError::internal_error())).boxed(),
                        }
                    }
                    Err(_) => {
                        futures::done(Err(JsonRpcError::invalid_params("Invalid Keyfile data \
                                                                        format")))
                                .boxed()
                    }
                }
            }
            Ok(_) => {
                futures::done(Err(JsonRpcError::invalid_params("Invalid JSON object"))).boxed()
            }
            Err(_) => futures::failed(JsonRpcError::invalid_params("Invalid JSON object")).boxed(),
        };
        io.add_async_method("backend_importWallet", import_callback);
    }

    {
        let switch_callback =
            move |p| match Params::parse::<Value>(p) {
                Ok(ref v) if v.as_array().is_some() => {
                    let chain = v.as_array()
                        .and_then(|arr| arr[0].as_str())
                        .and_then(|s| Some(s.to_owned()))
                        .expect("Node restart: Expect to extract chain label");

                    let tx = node.lock().expect("Node restart: Expect acquire channel");
                    let channel = tx.clone();
                    match channel.send(chain.clone()) {
                        Ok(_) => futures::done(Ok(Value::String(chain))).boxed(),
                        Err(err) => futures::failed(JsonRpcError::invalid_params(
                            format!("Node not responding: {}", err.to_string())))
                            .boxed(),
                    }
                }
                Ok(_) | Err(_) => {
                    futures::failed(JsonRpcError::invalid_params("Invalid chain label for node"))
                        .boxed()
                }
            };
        io.add_async_method("backend_switchChain", switch_callback);
    }

    {
        let create_callback = move |p| match Params::parse::<Value>(p) {
            Ok(ref v) if v.as_array().is_some() => {
                let passwd = v.as_array().and_then(|arr| arr[0].as_str()).unwrap();

                match KeyFile::new(passwd) {
                    Ok(kf) => {
                        let addr_res = kf.decrypt_address(passwd);
                        if addr_res.is_err() {
                            return futures::done(Err(JsonRpcError::internal_error())).boxed();
                        }
                        let addr = addr_res.unwrap();

                        match kf.flush(&default_path(), Some(addr)) {
                            Ok(_) => futures::done(Ok(Value::String(addr.to_string()))).boxed(),
                            Err(_) => futures::done(Err(JsonRpcError::internal_error())).boxed(),
                        }
                    }
                    Err(_) => {
                        futures::done(Err(JsonRpcError::invalid_params("Invalid Keyfile data \
                                                                        format")))
                                .boxed()
                    }
                }
            }
            Ok(_) => {
                futures::done(Err(JsonRpcError::invalid_params("Invalid JSON object"))).boxed()
            }
            Err(_) => futures::failed(JsonRpcError::invalid_params("Invalid JSON object")).boxed(),
        };
        io.add_async_method("personal_newAccount", create_callback);
    }


    let storage = match base_path {
        Some(p) => Storages::new(p),
        None => Storages::default(),
    };

    if storage.init().is_err() {
        panic!("Unable to initialize storage");
    }

    let chain = ChainStorage::new(&storage, "default".to_string());

    if chain.init().is_err() {
        panic!("Unable to initialize chain");
    }

    let dir = chain
        .get_path("contracts".to_string())
        .expect("Expect directory for contracts");

    let contracts = Arc::new(Contracts::new(dir));

    {
        let contracts = contracts.clone();

        io.add_async_method("emerald_contracts",
                            move |_| futures::finished(Value::Array(contracts.list())).boxed());
    }

    {
        let contracts = contracts.clone();

        io.add_async_method("emerald_addContract", move |p| match p {
            Params::Array(ref vec) => {
                match contracts.add(&vec[0]) {
                    Ok(_) => futures::finished(Value::Bool(true)).boxed(),
                    Err(_) => futures::failed(JsonRpcError::new(ErrorCode::InternalError)).boxed(),
                }
            }
            _ => futures::failed(JsonRpcError::new(ErrorCode::InvalidParams)).boxed(),
        });
    }

    let server = ServerBuilder::new(io)
        .meta_extractor(|req: &Req| {
                            req.header("Authorization")
                                .map(MethodMetadata::with_authorization)
                                .unwrap_or_default()
                        })
        .cors(DomainsValidation::AllowOnly(vec![cors::AccessControlAllowOrigin::Any,
                                                cors::AccessControlAllowOrigin::Null]))
        .start_http(addr)
        .expect("Expect to build HTTP RPC server");

    if log_enabled!(LogLevel::Info) {
        info!("Connector started on http://{}", server.address());
    }

    server.wait().expect("Expect to start HTTP RPC server");
}
