mod db;
mod error;
mod template;

use crate::db::{load_secret_key_mnemonic, store_secret_key_mnemonic};
use crate::error::AppError;
use crate::template::home_page;
use axum::extract::State;
use axum::response::{IntoResponse, Redirect};
use axum::{routing::get, Form, Router};
use bdk_esplora::esplora_client::AsyncClient;
use bdk_esplora::{esplora_client, EsploraAsyncExt};
use bdk_sqlx::Store;
use bdk_wallet::bip39::{Language, Mnemonic};
use bdk_wallet::bitcoin::script::PushBytesBuf;
use bdk_wallet::bitcoin::{Address, Amount, FeeRate, Txid};
use bdk_wallet::chain::{ChainPosition, ConfirmationBlockTime};
use bdk_wallet::descriptor::IntoWalletDescriptor;
use bdk_wallet::keys::bip39::WordCount::{self, Words12};
use bdk_wallet::template::Bip86;
use bdk_wallet::KeychainKind::{External, Internal};
use bdk_wallet::{bitcoin::Network, PersistedWallet, SignOptions, Wallet, WalletTx};
use serde::Deserialize;
use sqlx::sqlx_macros::migrate;
use sqlx::{Sqlite, SqlitePool, Transaction as DbTransaction};
use std::str::FromStr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::RwLock;
use tracing::debug;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

const ESPLORA_URL: &str = "https://mutinynet.com/api";
const PARALLEL_REQUESTS: usize = 5;
const WORD_COUNT: WordCount = Words12;
const NETWORK: Network = Network::Signet;
const DEFAULT_DB_URL: &str = "sqlite://bdk_wallet.sqlite?mode=rwc";
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

    // create database connection pool, URL from env or use default DB URL
    let database_url = std::env::var("WALLET_DB_URL").unwrap_or(DEFAULT_DB_URL.to_string());
    debug!("database_url: {:?}", &database_url);

    // run database schema migrations
    let pool = SqlitePool::connect(database_url.as_str()).await?;
    migrate!("./migrations").run(&pool).await?;

    // create wallet database store
    let mut store: Store<Sqlite> =
        Store::<Sqlite>::new(pool.clone(), Some(WALLET_NAME.to_string()), false).await?;

    // load or create and store new BIP-39 secret key mnemonic
    let mut tx: DbTransaction<Sqlite> = pool.begin().await?;
    let loaded_key = load_secret_key_mnemonic(&mut tx).await?;
    let mnemonic = match loaded_key {
        Some(mnemonic) => mnemonic,
        None => store_secret_key_mnemonic(&mut tx).await?,
    };
    let mnemonic = Mnemonic::parse_in(Language::English, mnemonic)?;
    tx.commit().await?;
    debug!("mnemonic: {}", &mnemonic);

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
        .load_wallet_async(&mut store)
        .await?;
    let wallet = match loaded_wallet {
        Some(wallet) => wallet,
        None => {
            Wallet::create(
                (external_descriptor, external_keymap),
                (internal_descriptor, internal_keymap),
            )
            .network(NETWORK)
            .create_wallet_async(&mut store)
            .await?
        }
    };

    // web app state
    let state = Arc::new(AppState {
        wallet: RwLock::new(wallet),
        store: RwLock::new(store),
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
        let (mut sent, mut received) = wallet.sent_and_received(&tx);
        let fee = wallet.calculate_fee(&tx).unwrap();
        if sent > received {
            sent = sent - received - fee;
            received = Amount::ZERO;
        } else {
            sent = Amount::ZERO;
            received -= sent;
        }
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
