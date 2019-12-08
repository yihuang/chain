use std::sync::Arc;
use std::collections::BTreeSet;

use structopt::StructOpt;
use jsonrpsee;
use jsonrpsee::core::common::Error as RpcError;
use jsonrpc_core::Error as OldRpcError;
use async_std;

use chain_core::tx::fee::LinearFee;
use client_common::storage::SledStorage;
use chain_core::init::coin::Coin;
use client_common::tendermint::types::GenesisExt;
use client_common::tendermint::{Client, WebsocketRpcClient};
use client_core::cipher::MockAbciTransactionObfuscation;
use client_core::signer::WalletSignerManager;
use client_core::transaction_builder::DefaultWalletTransactionBuilder;
use client_core::types::WalletKind;
use client_core::wallet::DefaultWalletClient;
use client_rpc::rpc::transaction_rpc::{TransactionRpc, TransactionRpcImpl};
use client_rpc::rpc::wallet_rpc::{WalletRpc, WalletRpcImpl};
use client_rpc::server::WalletRequest;
use client_rpc::program::Options;

type AppTransactionCipher = MockAbciTransactionObfuscation<WebsocketRpcClient>;
type AppTxBuilder = DefaultWalletTransactionBuilder<SledStorage, LinearFee, AppTransactionCipher>;
type AppWalletClient = DefaultWalletClient<SledStorage, WebsocketRpcClient, AppTxBuilder>;

jsonrpsee::rpc_api! {
    pub(crate) ClientRPC {
        fn wallet_create(wallet: WalletRequest, kind: WalletKind) -> String;
        fn wallet_list() -> Vec<String>;
        fn wallet_balance(wallet: WalletRequest) -> Coin;
        fn wallet_createStakingAddress(wallet: WalletRequest) -> String;
        fn wallet_createTransferAddress(wallet: WalletRequest) -> String;
        fn wallet_listStakingAddresses(wallet: WalletRequest) -> BTreeSet<String>;
        fn wallet_listTransferAddresses(wallet: WalletRequest) -> BTreeSet<String>;

        // fn transaction_createRaw(
        //     inputs: Vec<TxoPointer>,
        //     outputs: Vec<TxOut>,
        //     view_keys: Vec<PublicKey>
        // ) -> RawTransaction;
    }
}

fn make_wallet_client(
    storage: SledStorage,
    tendermint_client: WebsocketRpcClient,
) -> AppWalletClient {
    let transaction_builder = DefaultWalletTransactionBuilder::new(
        WalletSignerManager::new(storage.clone()),
        tendermint_client.genesis().unwrap().fee_policy(),
        MockAbciTransactionObfuscation::new(tendermint_client.clone()),
    );

    DefaultWalletClient::new(storage, tendermint_client, transaction_builder)
}

fn to_rpc_error(err: OldRpcError) -> RpcError {
    RpcError::invalid_params(err.to_string())
}

fn main() {
    let options = Options::from_args();
    let storage = SledStorage::new(options.storage_dir).unwrap();
    let tendermint_client = WebsocketRpcClient::new(&options.websocket_url).unwrap();
    let wallet_rpc_wallet_client = make_wallet_client(storage.clone(), tendermint_client.clone());
    let network_id = b'\xAB';

    let wallet_rpc = Arc::new(WalletRpcImpl::new(wallet_rpc_wallet_client, network_id));
    let transaction_rpc = TransactionRpcImpl::new(network_id);

    let listen_addr = format!("{}:{}", options.host, options.port).parse().unwrap();

    async_std::task::block_on(async move {
        let mut server1 = jsonrpsee::http_server(&listen_addr).await.unwrap();

        while let Ok(request) = ClientRPC::next_request(&mut server1).await {
            match request {
                ClientRPC::WalletCreate {
                    respond,
                    wallet,
                    kind,
                } => {
                    let wallet_rpc = wallet_rpc.clone();
                    respond
                        .respond(async_std::task::spawn_blocking(move || {
                            wallet_rpc.create(wallet, kind).map_err(to_rpc_error)
                        }).await)
                        .await;
                }
                ClientRPC::WalletList { respond } => {
                    respond
                        .respond(wallet_rpc.list().map_err(to_rpc_error))
                        .await;
                }
                ClientRPC::WalletBalance { respond, wallet } => {
                    let wallet_rpc = wallet_rpc.clone();
                    respond.respond(async_std::task::spawn_blocking(move || {
                        wallet_rpc.balance(wallet).map_err(to_rpc_error)
                    }).await).await;
                }
                ClientRPC::WalletCreateTransferAddress { respond, wallet } => {
                    let wallet_rpc = wallet_rpc.clone();
                    respond.respond(async_std::task::spawn_blocking(move || {
                        wallet_rpc.create_transfer_address(wallet).map_err(to_rpc_error)
                    }).await).await;
                }
                ClientRPC::WalletCreateStakingAddress { respond, wallet } => {
                    let wallet_rpc = wallet_rpc.clone();
                    respond.respond(async_std::task::spawn_blocking(move || {
                        wallet_rpc.create_staking_address(wallet).map_err(to_rpc_error)
                    }).await).await;
                }
                ClientRPC::WalletListStakingAddresses { respond, wallet } => {
                    let wallet_rpc = wallet_rpc.clone();
                    respond.respond(async_std::task::spawn_blocking(move || {
                        wallet_rpc.list_staking_addresses(wallet).map_err(to_rpc_error)
                    }).await).await;
                }
                ClientRPC::WalletListTransferAddresses { respond, wallet } => {
                    let wallet_rpc = wallet_rpc.clone();
                    respond.respond(async_std::task::spawn_blocking(move || {
                        wallet_rpc.list_transfer_addresses(wallet).map_err(to_rpc_error)
                    }).await).await;
                }
            }
        }
    });
}
