# TABConf24 BDK 1.0 Workshop

## What's new in BDK 1.0

* `bdk_wallet` crate built on new `bdk_chain` crate
  * `bdk_chain` monotone block data tracking functions, minimal dependencies
  * `bdk_wallet` handles higher level transaction monitoring, building and signing
  * can build unique wallet and non-wallet apps directly on `bdk_chain`
  * improved `no-std` support
  * built on latest `rust-bitcoin` (0.32) and `rust-miniscript` (12.0)
  
* Decoupled data syncing and storage crates from `bdk_wallet` and `bdk_chain`
  * can sync and read/write wallet and chain data with async or blocking methods
  * sync and store crates only need to implement simple traits
  * able use async or blocking I/O 

* Improved blockchain sync clients
  * better electrum and esplora client performance and error handling
  * experimental block-by-block core RPC client
  * experimental compact block filter client based on `kyoto`

* Improved wallet and chain data stores
  * async PostgreSQL and SQLite `sqlx` based stores
  * blocking SQLite store based on `rusqlite`
  * experimental blocking flat file based store

## Purpose of Workshop

The purpose of this workshop is to demonstrate how to build a simple, pure Rust [bdk-wallet 1.0](https://github.com/bitcoindevkit/bdk/releases) based app using the [Axum](https://github.com/tokio-rs/axum) web framework, the [rust-esplora-esplora](https://github.com/bitcoindevkit/rust-esplora-client) blockchain client, and a SQLite embedded database. 

## Quick Start

### Setup

1. [Git](https://github.com/git-guides/install-git)
2. [Rust](https://www.rust-lang.org/tools/install)
3. [SQLite](https://medium.com/@techwithjulles/part-5-how-to-install-sqlite-on-your-machine-windows-linux-and-mac-simple-version-f05b7963b6cd)
4. Editor (eg. [RustRover](https://www.jetbrains.com/rust/), *Vim, [VSCode](https://code.visualstudio.com/docs/languages/rust), [Zed](https://zed.dev/), etc.)

### Build/Run

1. Clone this repo
   ```
   git clone https://github.com/notmandatory/tabconf24_bdk_workshop.git
   ```
2. Build and run
   ```
   cd tabconf24_bdk_workshop
   cargo run
   ```
3. Change DB file URL (optional)
   ```aiignore
   export WALLET_DB_URL="sqlite://YOUR_CUSTOM_NAME.sqlite?mode=rwc`
   ```
   
## Code Walkthrough



## BDK Links

* [Home](https://bitcoindevkit.org)
* [Repo](https://github.com/bitcoindevkit/bdk)
* [API docs for `bdk_wallet`](https://docs.rs/bdk_wallet/latest/bdk_wallet/)
* [Discord](https://discord.gg/dstn4dQ)
* [Nostr](https://primal.net/p/npub13dk3dke4zm9vdkucm7f6vv7vhqgkevgg3gju9kr2wzumz7nrykdq0dgnvc)
* [bdk-ffi (Kotlin,Swift,Python)](https://github.com/bitcoindevkit/bdk-ffi)
* [WIP "Book of BDK"](https://bitcoindevkit.github.io/book-of-bdk/)

