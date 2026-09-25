// Copyright (c) LightPool Labs
// Author: xiaoyu1998

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Market {
    pub id: Uuid,
    pub slug: String,
    pub question: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub event_slug: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_item_title: Option<String>,
    /// Yes mark price in cents (0-100), from last trade or (best bid + best ask) / 2.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub yes_rate: Option<String>,
    pub market_address: String,
    pub collateral_token: String,
    pub yes_token: String,
    pub no_token: String,
    pub yes_spot_market: String,
    pub no_spot_market: String,
    pub state: String,
    pub resolution_deadline: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpotBookLevel {
    pub price: String,
    pub size: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpotBook {
    pub sequence: u64,
    pub bids: Vec<SpotBookLevel>,
    pub asks: Vec<SpotBookLevel>,
    #[serde(default)]
    pub last_trade_price: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    pub id: Uuid,
    pub market_id: Uuid,
    #[serde(default)]
    pub market_slug: String,
    pub question: String,
    pub outcome: String,
    pub side: String,
    pub price: String,
    pub size: String,
    pub status: String,
    #[serde(default)]
    pub chain_order_id: String,
    #[serde(default)]
    pub spot_market: String,
    #[serde(default)]
    pub user_address: String,
    #[serde(default)]
    pub size_raw: u64,
    #[serde(default)]
    pub filled_raw: u64,
    #[serde(default, skip_serializing_if = "is_zero_u64")]
    pub status_ts_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BalanceEntry {
    pub token: String,
    pub symbol: String,
    pub total: String,
    pub locked: String,
    pub available: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateTokenRequest {
    pub name: String,
    pub symbol: String,
    pub total_supply: u64,
    pub mintable: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct CreateTokenResponse {
    pub token_address: String,
    pub tx_digest: String,
    pub name: String,
    pub symbol: String,
    pub creator: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateEventContractRequest {
    pub question: String,
    pub icon_url: Option<String>,
    pub event_slug: Option<String>,
    pub group_item_title: Option<String>,
    pub collateral_token: Option<String>,
    pub oracle: Option<String>,
    pub resolution_deadline: u64,
    pub tick_size: Option<u64>,
    pub min_order_size: Option<u64>,
    pub maker_fee_bps: Option<u16>,
    pub taker_fee_bps: Option<u16>,
    pub allow_market_orders: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateMarketMetadataRequest {
    pub event_slug: Option<String>,
    pub group_item_title: Option<String>,
    pub icon_url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct MintBurnRequest {
    pub amount: String,
}

#[derive(Debug, Serialize)]
pub struct MintBurnResponse {
    pub slug: String,
    pub amount: String,
    pub tx_digest: String,
}

#[derive(Debug, Serialize)]
pub struct CreateEventContractResponse {
    pub market_id: Uuid,
    pub slug: String,
    pub question: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_slug: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group_item_title: Option<String>,
    pub market_address: String,
    pub collateral_token: String,
    pub yes_token: String,
    pub no_token: String,
    pub yes_spot_market: String,
    pub no_spot_market: String,
    pub state: String,
    pub resolution_deadline: u64,
    pub tx_digest: String,
    pub creator: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct QueryMarketsParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slug: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slugs: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub market_ids: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub market_addresses: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ascending: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketsPage {
    pub markets: Vec<Market>,
    pub total: usize,
    pub limit: u32,
    pub offset: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultAsset {
    pub market: String,
    pub amount: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_price: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quote_value: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vault {
    pub id: Uuid,
    pub name: String,
    pub vault_address: String,
    pub vault_account: String,
    pub manager: String,
    pub quote_token: String,
    pub share_token: String,
    pub equity: String,
    #[serde(default)]
    pub user_deposit: String,
    #[serde(default)]
    pub portfolio: Vec<VaultAsset>,
    pub allow_deposit: bool,
    pub is_closed: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct QueryVaultsParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manager: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vault_addresses: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultsPage {
    pub vaults: Vec<Vault>,
    pub total: usize,
    pub limit: u32,
    pub offset: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultDepositRequest {
    pub amount: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultWithdrawRequest {
    pub shares: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultDepositWithdrawResponse {
    pub vault_address: String,
    pub amount: String,
    pub shares: String,
    pub tx_digest: String,
}

fn is_zero_u64(value: &u64) -> bool {
    *value == 0
}
