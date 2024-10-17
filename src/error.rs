use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use bdk_esplora::esplora_client;
use bdk_sqlx::BdkSqlxError;
use bdk_wallet::bitcoin::{address, psbt};
use bdk_wallet::chain::local_chain::CannotConnectError;
use bdk_wallet::chain::tx_graph::CalculateFeeError;
use bdk_wallet::descriptor::DescriptorError;
use bdk_wallet::error::CreateTxError;
use bdk_wallet::keys::bip39;
use bdk_wallet::signer::SignerError;
use bdk_wallet::{CreateWithPersistError, LoadWithPersistError};
use sqlx::migrate::MigrateError;
use std::io;
use std::num::ParseIntError;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("sqlx database error: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("sqlx database migrate error: {0}")]
    SqlxMigrate(#[from] MigrateError),
    #[error("bdk sqlx database error: {0}")]
    BdkSqlx(#[from] BdkSqlxError),
    #[error("esplora client error: {0}")]
    Esplora(#[from] esplora_client::Error),
    #[error("esplora sync error: {0}")]
    EsploraSync(#[from] Box<esplora_client::Error>),
    #[error("descriptor error: {0}")]
    Descriptor(#[from] DescriptorError),
    #[error("bip39 mnemonic error: {0}")]
    Mnemonic(#[from] bip39::Error),
    #[error("load with persist error: {0}")]
    LoadWithPersist(#[from] LoadWithPersistError<BdkSqlxError>),
    #[error("create with persist error: {0}")]
    CreateWithPersist(#[from] CreateWithPersistError<BdkSqlxError>),
    #[error("io error: {0}")]
    Io(#[from] io::Error),
    #[error("wallet update error: {0}")]
    WalletUpdate(#[from] CannotConnectError),
    #[error("parse int error: {0}")]
    ParseInt(#[from] ParseIntError),
    #[error("parse address error: {0}")]
    ParseAddress(#[from] address::ParseError),
    #[error("create tx error: {0}")]
    CreateTx(#[from] CreateTxError),
    #[error("signer error: {0}")]
    Signer(#[from] SignerError),
    #[error("extract tx error: {0}")]
    ExtractTx(#[from] psbt::ExtractTxError),
    #[error("unable to finalize psbt")]
    Finalize,
    #[error("calculate fee error: {0}")]
    CalculateFee(#[from] CalculateFeeError),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Something went wrong: {}", self),
        )
            .into_response()
    }
}
