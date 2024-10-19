-- Schema version control
CREATE TABLE IF NOT EXISTS version (
    version INTEGER PRIMARY KEY
);

-- Mnemonic keys for each wallet. DON'T STORE REAL BITCOIN KEYS THIS WAY.
CREATE TABLE IF NOT EXISTS key (
    wallet_name TEXT PRIMARY KEY,
    mnemonic TEXT NOT NULL
);
