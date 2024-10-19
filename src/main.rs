mod error;
mod key_store;
mod template;

use crate::error::AppError;
use crate::key_store::KeyStore;
use crate::template::home_page;
use axum::response::{IntoResponse, Redirect};
use axum::{extract::State, routing::get, Form, Router};
use bdk_esplora::{esplora_client, esplora_client::AsyncClient, EsploraAsyncExt};
use bdk_sqlx::Store;
use bdk_wallet::bitcoin::{script::PushBytesBuf, Address, Amount, FeeRate, Network, Txid};
use bdk_wallet::chain::{ChainPosition, ConfirmationBlockTime};
use bdk_wallet::descriptor::IntoWalletDescriptor;
use bdk_wallet::template::Bip86;
use bdk_wallet::KeychainKind::{External, Internal};
use bdk_wallet::{PersistedWallet, SignOptions, Wallet, WalletTx};
use serde::Deserialize;
use sqlx::{Sqlite, SqlitePool};
use std::{str::FromStr, sync::Arc};
use tokio::{net::TcpListener, sync::RwLock};
use tracing::debug;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

const ESPLORA_URL: &str = "https://mutinynet.com/api";
const PARALLEL_REQUESTS: usize = 5;
const NETWORK: Network = Network::Signet;
const DEFAULT_KEY_DB_URL: &str = "sqlite://bdk_key.sqlite?mode=rwc";
const DEFAULT_WALLET_DB_URL: &str = "sqlite://bdk_wallet.sqlite?mode=rwc";
const WALLET_NAME: &str = "primary";

struct AppState {
    wallet: RwLock<PersistedWallet<Store<Sqlite>>>,
    store: RwLock<Store<Sqlite>>,
    client: AsyncClient,
}

#[tokio::main]
async fn main() -> Result<(), AppError> {
    // configure async logging
    tracing_subscriber::registry()
        .with(EnvFilter::new(std::env::var("RUST_LOG").unwrap_or_else(
            |_| format!("sqlx=warn,{}=debug", env!("CARGO_CRATE_NAME")),
        )))
        .with(tracing_subscriber::fmt::layer())
        .try_init()
        .expect("init logging");

    // create esplora client
    let client = esplora_client::Builder::new(ESPLORA_URL).build_async()?;

    // create database connection pools, URL from env or use default DB URL
    let key_db_url = std::env::var("KEY_DB_URL").unwrap_or(DEFAULT_KEY_DB_URL.to_string());
    debug!("key_db_url: {:?}", &key_db_url);
    let wallet_db_url = std::env::var("WALLET_DB_URL").unwrap_or(DEFAULT_WALLET_DB_URL.to_string());
    debug!("wallet_db_url: {:?}", &wallet_db_url);

    // create key database connection pool and run key database schema migrations
    let key_pool = SqlitePool::connect(key_db_url.as_str()).await?;
    let key_store = KeyStore::new(key_pool).await?;

    // load or create and store new BIP-39 secret key mnemonic
    let mnemonic = key_store
        .load_or_generate_key(WALLET_NAME.to_string())
        .await?;
    debug!("mnemonic: {}", &mnemonic);

    // create wallet database connection pool and wallet database store
    let wallet_pool = SqlitePool::connect(wallet_db_url.as_str()).await?;
    let mut wallet_store: Store<Sqlite> =
        Store::<Sqlite>::new(wallet_pool.clone(), WALLET_NAME.to_string(), true).await?;

    // create BIP86 taproot descriptors
    let (external_descriptor, external_keymap) =
        Bip86(mnemonic.clone(), External).into_wallet_descriptor(&Default::default(), NETWORK)?;
    debug!("external_descriptor: {}", &external_descriptor);
    let (internal_descriptor, internal_keymap) =
        Bip86(mnemonic.clone(), Internal).into_wallet_descriptor(&Default::default(), NETWORK)?;
    debug!("internal_descriptor: {}", &internal_descriptor);

    // load or create and store a new wallet
    let loaded_wallet = Wallet::load()
        .descriptor(
            External,
            Some((external_descriptor.clone(), external_keymap.clone())),
        )
        .descriptor(
            Internal,
            Some((internal_descriptor.clone(), internal_keymap.clone())),
        )
        .extract_keys()
        .check_network(NETWORK)
        .load_wallet_async(&mut wallet_store)
        .await?;
    let wallet = match loaded_wallet {
        Some(wallet) => wallet,
        None => {
            Wallet::create(
                (external_descriptor, external_keymap),
                (internal_descriptor, internal_keymap),
            )
            .network(NETWORK)
            .create_wallet_async(&mut wallet_store)
            .await?
        }
    };

    // web app state
    let state = Arc::new(AppState {
        wallet: RwLock::new(wallet),
        store: RwLock::new(wallet_store),
        client,
    });

    // configure web server routes
    let app = Router::new()
        .route("/", get(home).post(spend))
        .with_state(state);

    // start the web server
    let listener = TcpListener::bind("127.0.0.1:3000").await?;
    debug!("listening on: http://{}", listener.local_addr()?);
    axum::serve(listener, app).await.map_err(|e| e.into())
}

// web page handlers

async fn home(State(state): State<Arc<AppState>>) -> Result<impl IntoResponse, AppError> {
    debug!("syncing");
    let sync_result = {
        // use wallet read-only lock during esplora client sync, drop lock after sync
        let sync_request = state
            .wallet
            .read()
            .await
            .start_sync_with_revealed_spks()
            .build();
        state.client.sync(sync_request, PARALLEL_REQUESTS).await?
    };

    // after sync get wallet write lock to update and persist changes
    let next_unused_address = {
        let mut wallet = state.wallet.write().await;
        debug!("apply update");
        wallet.apply_update(sync_result)?;
        let next_unused_address = wallet.next_unused_address(External).address;
        debug!("storing");
        let mut store = state.store.write().await;
        wallet.persist_async(&mut store).await?;
        next_unused_address
    };

    // get wallet read lock after update and persist to list transactions
    debug!("transactions list");
    let wallet = state.wallet.read().await;
    let balance = wallet.balance();
    let mut txs = wallet
        .transactions()
        .map(|tx| TxDetails::new(tx, &wallet))
        .collect::<Vec<_>>();
    txs.sort_by(|tx1, tx2| tx1.chain_position.cmp(&tx2.chain_position));

    // render home page from template
    Ok(home_page(next_unused_address, balance, txs))
}

struct TxDetails {
    txid: Txid,
    sent: Amount,
    received: Amount,
    fee: Amount,
    fee_rate: FeeRate,
    chain_position: ChainPosition<ConfirmationBlockTime>,
}

impl<'a> TxDetails {
    fn new(wallet_tx: WalletTx<'a>, wallet: &PersistedWallet<Store<Sqlite>>) -> Self {
        let txid = wallet_tx.tx_node.txid;
        let tx = wallet_tx.tx_node.tx;
        let (sent, received) = wallet.sent_and_received(&tx);
        let fee = wallet.calculate_fee(&tx).unwrap();
        let fee_rate = wallet.calculate_fee_rate(&tx).unwrap();
        let chain_position: ChainPosition<ConfirmationBlockTime> =
            wallet_tx.chain_position.cloned();
        TxDetails {
            txid,
            sent,
            received,
            fee,
            fee_rate,
            chain_position,
        }
    }
}

#[derive(Deserialize, Debug)]
struct SpendRequest {
    address: String,
    amount: String,
    fee_rate: String,
    note: String,
}

async fn spend(
    State(state): State<Arc<AppState>>,
    Form(spend): Form<SpendRequest>,
) -> Result<impl IntoResponse, AppError> {
    // validate form inputs
    debug!(
        "spend {} sats to address {} with fee rate {} sats/vbyte",
        &spend.amount, &spend.address, &spend.fee_rate
    );
    let amount = Amount::from_sat(u64::from_str(spend.amount.as_str())?);
    let address = Address::from_str(&spend.address)?.require_network(NETWORK)?;
    let script_pubkey = address.script_pubkey();
    let fee_rate =
        FeeRate::from_sat_per_vb(u64::from_str(spend.fee_rate.as_str())?).expect("valid fee rate");
    let note = spend.note.into_bytes();
    let note = PushBytesBuf::try_from(note).unwrap();

    let mut wallet = state.wallet.write().await;

    // create and sign PSBT
    let (psbt, is_finalized) = {
        let mut tx_builder = wallet.build_tx();
        tx_builder.add_recipient(script_pubkey, amount);
        tx_builder.fee_rate(fee_rate);
        tx_builder.add_data(&note);
        let mut psbt = tx_builder.finish()?;
        let is_finalized = wallet.sign(&mut psbt, SignOptions::default())?;
        (psbt, is_finalized)
    };

    // broadcast finalized transaction
    if is_finalized {
        let tx = &psbt.extract_tx()?;
        state.client.broadcast(tx).await?;
        // need to store wallet with new internal (change) index
        let mut store = state.store.write().await;
        wallet.persist_async(&mut store).await?;
        Ok(Redirect::to("/"))
    } else {
        debug!("non-finalized psbt: {}", &psbt);
        Err(AppError::Finalize)
    }
}
