use crate::TxDetails;
use bdk_esplora::esplora_client::Txid;
use bdk_wallet::bitcoin::Address;
use bdk_wallet::chain::{Anchor, ChainPosition};
use bdk_wallet::Balance;
use maud::{html, Markup};

const MUTINYNET_URL: &str = "https://mutinynet.com";

pub fn home_page(next_unused_address: Address, balance: Balance, txs: Vec<TxDetails>) -> Markup {
    html! {
        head {
            meta charset="UTF-8";
            title { "TABConf24 BDK 1.0 Workshop" }
            //link rel="icon" href="data:image/svg+xml,<svg xmlns=%22http://www.w3.org/2000/svg%22 viewBox=%220 0 100 100%22><text y=%22.9em%22 font-size=%2290%22>🕸️</text></svg>";
            link rel="icon" href="https://bitcoindevkit.org/favicon.ico";
        }

        h1 { "TABConf24 BDK 1.0 Workshop" }
        h2 { "Wallet" }
        h3 { "Balance" }
        table border="1" {
            thead {
                tr {
                    th { "state" }
                    th { "sats" }
                }
            }
            tbody {
                tr {
                    td { "confirmed" }
                    td { (balance.confirmed.to_sat()) }
                }
                tr {
                    td { "trusted pending" }
                    td { (balance.trusted_pending.to_sat()) }
                }
                tr {
                    td { "untrusted pending" }
                    td { (balance.untrusted_pending.to_sat()) }
                }
                tr {
                    td { "immature" }
                    td { (balance.immature.to_sat()) }
                }
            }
        }
        // getting last unused addresses could modify keychain's last used index
        h3 { "Receive Address"} p { ( next_unused_address.clone() ) }
        form action="https://faucet.mutinynet.com/" method="get" target="_blank" rel="noopener noreferrer" {
            input type="submit" value="Get Sats";
        }
        h3 { "Transactions"}
        table border="1" {
            thead {
                tr {
                    th { "txid" }
                    th { "sent (sats)" }
                    th { "received (sats)" }
                    th { "fee (sats)" }
                    th { "fee rate (sats/vb)" }
                    th { "chain_position" }
                }
            }
            tbody {
                @for tx in txs {
                    tr {
                        td { (transaction_link(tx.txid)) }
                        td { (tx.sent.to_sat()) }
                        td { (tx.received.to_sat()) }
                        td { (tx.fee.to_sat()) }
                        td { (tx.fee_rate.to_sat_per_vb_floor()) }
                        @match tx.chain_position {
                            ChainPosition::Confirmed(block) => {
                                td { "confirmed("(block.confirmation_height_upper_bound())")" }
                            }
                            ChainPosition::Unconfirmed(_) => {
                                td { "unconfirmed" }
                            }
                        }
                    }
                }
            }
        }

        h3 { "Create Transaction" }
        form method="post" action="/" {
            label for="address" { "To address"} br;
            input #address type="text" name="address" value="tb1qd28npep0s8frcm3y7dxqajkcy2m40eysplyr9v";br;br;
            label for="amount" { "Amount (sats)" } br;
            input #amount type="text" name="amount" value="1000";br;br;
            label for="fee_rate" { "Fee Rate (sats/vb)" } br;
            input #fee_rate type="text" name="fee_rate" value="1";br;br;
            label for="note" { "Note" } br;
            input #note type="text" name="note" value="TABConf24 BDK 1.0 Workshop";br;br;
            input type="submit" value="Spend";
        }
    }
}

fn transaction_link(txid: Txid) -> Markup {
    html! {
        a href={ (MUTINYNET_URL) "/tx/" (txid) } target="_blank" rel="noopener noreferrer" { (txid) }
    }
}
