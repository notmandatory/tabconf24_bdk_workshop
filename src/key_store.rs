use crate::error::AppError;
use bdk_wallet::bip39::{Language, Mnemonic};
use bdk_wallet::bitcoin::key::rand;
use bdk_wallet::bitcoin::key::rand::Rng;
use bdk_wallet::keys::bip39::WordCount;
use bdk_wallet::keys::bip39::WordCount::Words12;
use bdk_wallet::keys::{GeneratableKey, GeneratedKey};
use bdk_wallet::miniscript::Tap;
use sqlx::migrate::MigrateError;
use sqlx::sqlx_macros::migrate;
use sqlx::{Database, Pool, Row, Sqlite, Transaction as DbTransaction};
use tracing::{debug, info};

const WORD_COUNT: WordCount = Words12;

#[derive(Debug, Clone)]
pub(crate) struct KeyStore<DB: Database>(Pool<DB>);

impl KeyStore<Sqlite> {
    /// Construct a new [`Store`] with an existing sqlite connection.
    pub(crate) async fn new(pool: Pool<Sqlite>) -> Result<Self, MigrateError> {
        info!("new store");
        migrate!("./migrations").run(&pool).await?;
        Ok(Self(pool))
    }

    pub(crate) async fn load_or_generate_key(
        &self,
        wallet_name: String,
    ) -> Result<Mnemonic, AppError> {
        match self.load_key(wallet_name.clone()).await? {
            Some(mnemonic) => Ok(mnemonic),
            None => self.generate_and_store_key(wallet_name).await,
        }
    }

    // generate and store a new secret key mnemonic
    async fn generate_and_store_key(&self, wallet_name: String) -> Result<Mnemonic, AppError> {
        let mut tx: DbTransaction<Sqlite> = self.0.begin().await?;
        // create new key entropy
        debug!("generating new key");
        let mut rng = rand::thread_rng();
        let mut entropy = [0u8; 32];
        rng.fill(&mut entropy);

        // create mnemonic words from entropy
        let generated_key: GeneratedKey<_, Tap> =
            Mnemonic::generate_with_entropy((WORD_COUNT, Language::English), entropy).unwrap();
        let generated_mnemonic = generated_key.to_string();

        sqlx::query("INSERT INTO key (wallet_name, mnemonic) VALUES ($1, $2)")
            .bind(wallet_name)
            .bind(&generated_mnemonic)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        let mnemonic = Mnemonic::parse_in(Language::English, generated_mnemonic)?;
        Ok(mnemonic)
    }

    // load an existing secret key mnemonic
    async fn load_key(&self, wallet_name: String) -> Result<Option<Mnemonic>, AppError> {
        let mut tx: DbTransaction<Sqlite> = self.0.begin().await?;
        // load mnemonic words if they exist
        let row = sqlx::query::<Sqlite>("SELECT mnemonic FROM key WHERE wallet_name = $1")
            .bind(wallet_name)
            .fetch_optional(&mut *tx)
            .await?;
        let stored_mnemonic: Option<String> = row.map(|r| r.get(0));
        tx.commit().await?;
        let mnemonic = stored_mnemonic
            .map(|m| Mnemonic::parse_in(Language::English, m))
            .transpose()?;
        Ok(mnemonic)
    }
}
