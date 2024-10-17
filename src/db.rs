use crate::error::AppError;
use crate::{WALLET_NAME, WORD_COUNT};
use bdk_wallet::bip39::{Language, Mnemonic};
use bdk_wallet::bitcoin::key::rand;
use bdk_wallet::bitcoin::key::rand::Rng;
use bdk_wallet::keys::{GeneratableKey, GeneratedKey};
use bdk_wallet::miniscript::Tap;
use sqlx::{Row, Sqlite, Transaction as DbTransaction};
use tracing::debug;

// generate and store a new secret key mnemonic
pub(crate) async fn store_secret_key_mnemonic(
    tx: &mut DbTransaction<'_, Sqlite>,
) -> Result<String, AppError> {
    // create new key entropy
    debug!("generating new key");
    let mut rng = rand::thread_rng();
    let mut entropy = [0u8; 32];
    rng.fill(&mut entropy);

    // create mnemonic words from entropy
    let generated_key: GeneratedKey<_, Tap> =
        Mnemonic::generate_with_entropy((WORD_COUNT, Language::English), entropy).unwrap();
    let generated_key = generated_key.to_string();

    sqlx::query("INSERT INTO keys (wallet_name, mnemonic) VALUES ($1, $2)")
        .bind(WALLET_NAME.to_string())
        .bind(&generated_key)
        .execute(&mut **tx)
        .await?;
    Ok(generated_key)
}

// load an existing secret key mnemonic
pub(crate) async fn load_secret_key_mnemonic(
    tx: &mut DbTransaction<'_, Sqlite>,
) -> Result<Option<String>, AppError> {
    // load mnemonic words if they exist
    let row = sqlx::query::<Sqlite>("SELECT mnemonic FROM keys WHERE wallet_name = $1")
        .bind(WALLET_NAME.to_string())
        .fetch_optional(&mut **tx)
        .await?;
    let stored_key: Option<String> = row.map(|r| r.get(0));
    Ok(stored_key)
}
