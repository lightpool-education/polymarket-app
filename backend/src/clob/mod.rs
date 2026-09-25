// Copyright (c) LightPool Labs
// Author: xiaoyu1998

use lightpool_sdk::lightpool_types::SignedTransaction;
use lightpool_sdk::spot_events::OrderCreatedEvent;
use lightpool_sdk::types::SubmitTransactionResponse;
use lightpool_sdk::TransactionReceipt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::models::{
    BalanceEntry, Market, MarketsPage, Order, QueryMarketsParams, QueryVaultsParams, SpotBook,
    Vault, VaultsPage,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BalanceTokenSpec {
    pub symbol: String,
    pub address: String,
}

#[derive(Debug, Deserialize)]
struct ErrorBody {
    error: String,
}

#[derive(Debug, Deserialize)]
struct ReadyResponse {
    status: String,
    node: bool,
}

#[derive(Debug, Deserialize)]
struct SubmitTxResponse {
    digest: String,
    block_num: u64,
    receipt: TransactionReceipt,
}

pub struct ClobIndexClient {
    client: Client,
    base_url: String,
}

impl ClobIndexClient {
    pub fn new(base_url: impl Into<String>) -> Self {
        let client = Client::builder()
            .pool_max_idle_per_host(0)
            .build()
            .expect("failed to build clob-index HTTP client");
        Self {
            client,
            base_url: base_url.into().trim_end_matches('/').to_string(),
        }
    }

    async fn get_json<T: for<'de> Deserialize<'de>>(&self, path: &str) -> AppResult<T> {
        let url = format!("{}{}", self.base_url, path);
        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("clob-index request failed: {e}")))?;

        Self::decode_response(response).await
    }

    async fn post_json<B: Serialize, T: for<'de> Deserialize<'de>>(
        &self,
        path: &str,
        body: &B,
    ) -> AppResult<T> {
        let url = format!("{}{}", self.base_url, path);
        let response = self
            .client
            .post(&url)
            .json(body)
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("clob-index request failed: {e}")))?;

        Self::decode_response(response).await
    }

    async fn decode_response<T: for<'de> Deserialize<'de>>(
        response: reqwest::Response,
    ) -> AppResult<T> {
        let status = response.status();
        if status.is_success() {
            return response
                .json::<T>()
                .await
                .map_err(|e| AppError::Internal(format!("decode clob-index response: {e}")));
        }

        let body = response
            .json::<ErrorBody>()
            .await
            .unwrap_or(ErrorBody {
                error: "unknown clob-index error".into(),
            });

        let message = body.error;
        if status.as_u16() == 404 {
            return Err(AppError::NotFound(message));
        }
        if status.as_u16() == 400 {
            return Err(AppError::BadRequest(message));
        }
        Err(AppError::Internal(message))
    }

    async fn get_json_with_query<T, Q>(
        &self,
        path: &str,
        query: &Q,
    ) -> AppResult<T>
    where
        T: for<'de> Deserialize<'de>,
        Q: Serialize + ?Sized,
    {
        let url = format!("{}{}", self.base_url, path);
        let response = self
            .client
            .get(&url)
            .query(query)
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("clob-index request failed: {e}")))?;

        Self::decode_response(response).await
    }

    pub async fn health_check(&self) -> AppResult<bool> {
        let ready: ReadyResponse = self.get_json("/api/health/ready").await?;
        Ok(ready.node && ready.status == "ready")
    }

    pub async fn query_markets(&self, params: &QueryMarketsParams) -> AppResult<MarketsPage> {
        self.get_json_with_query("/api/markets", params).await
    }

    pub async fn fetch_all_markets(&self) -> AppResult<Vec<Market>> {
        const PAGE_LIMIT: u32 = 100;
        let mut offset = 0u32;
        let mut markets = Vec::new();

        loop {
            let page = self
                .query_markets(&QueryMarketsParams {
                    limit: Some(PAGE_LIMIT),
                    offset: Some(offset),
                    ..QueryMarketsParams::default()
                })
                .await?;

            let page_len = page.markets.len();
            markets.extend(page.markets);
            if markets.len() >= page.total || page_len == 0 {
                break;
            }
            offset = offset.saturating_add(PAGE_LIMIT);
        }

        Ok(markets)
    }

    pub async fn get_market_by_slug(&self, slug: &str) -> AppResult<Market> {
        self.get_json(&format!("/api/markets/slug/{slug}")).await
    }

    pub async fn get_spot_book(&self, spot_market: &str, depth: u32) -> AppResult<SpotBook> {
        let encoded = urlencoding_encode(spot_market);
        self.get_json(&format!(
            "/api/spot/{encoded}/book?depth={depth}"
        ))
        .await
    }

    pub async fn query_vaults(&self, params: &QueryVaultsParams) -> AppResult<VaultsPage> {
        self.get_json_with_query("/api/vaults", params).await
    }

    pub async fn fetch_all_vaults(&self) -> AppResult<Vec<Vault>> {
        const PAGE_LIMIT: u32 = 100;
        let mut offset = 0u32;
        let mut vaults = Vec::new();

        loop {
            let page = self
                .query_vaults(&QueryVaultsParams {
                    limit: Some(PAGE_LIMIT),
                    offset: Some(offset),
                    ..QueryVaultsParams::default()
                })
                .await?;

            let page_len = page.vaults.len();
            vaults.extend(page.vaults);
            if vaults.len() >= page.total || page_len == 0 {
                break;
            }
            offset = offset.saturating_add(PAGE_LIMIT);
        }

        Ok(vaults)
    }

    pub async fn get_vault_by_address(&self, address: &str) -> AppResult<Vault> {
        self.get_json(&format!("/api/vaults/address/{address}"))
            .await
    }

    pub async fn get_vault_by_address_for_account(
        &self,
        address: &str,
        account: &str,
    ) -> AppResult<Vault> {
        #[derive(Serialize)]
        struct AccountQuery<'a> {
            account: &'a str,
        }
        self.get_json_with_query(
            &format!("/api/vaults/address/{address}"),
            &AccountQuery { account },
        )
        .await
    }

    pub async fn position_token_specs(&self) -> AppResult<Vec<BalanceTokenSpec>> {
        self.get_json("/api/markets/index/position-token-specs")
            .await
    }

    pub async fn get_balances(
        &self,
        account: &str,
        tokens: &[BalanceTokenSpec],
    ) -> AppResult<Vec<BalanceEntry>> {
        #[derive(Serialize)]
        struct Body<'a> {
            tokens: &'a [BalanceTokenSpec],
        }
        self.post_json(
            &format!("/api/accounts/{account}/balances"),
            &Body { tokens },
        )
        .await
    }

    pub async fn submit_transaction(
        &self,
        tx: SignedTransaction,
    ) -> AppResult<SubmitTransactionResponse> {
        #[derive(Serialize)]
        struct Body {
            tx: SignedTransaction,
        }
        let response: SubmitTxResponse = self
            .post_json("/api/tx/submit", &Body { tx })
            .await?;
        Ok(SubmitTransactionResponse {
            digest: response.digest,
            block_num: response.block_num,
            receipt: response.receipt,
        })
    }

    pub async fn list_orders(&self, user_address: &str) -> AppResult<Vec<Order>> {
        let q = format!("user_address={user_address}");
        let open: Vec<Order> = match self
            .get_json(&format!("/api/orders/openOrders?{q}"))
            .await
        {
            Ok(orders) => orders,
            Err(_) => self.get_json(&format!("/api/orders?{q}")).await?,
        };
        let historical: Vec<Order> = self
            .get_json(&format!("/api/orders/historicalOrders?{q}"))
            .await
            .unwrap_or_default();
        let mut by_id = std::collections::HashMap::new();
        for order in historical {
            by_id.insert(order.id, order);
        }
        for order in open {
            by_id.insert(order.id, order);
        }
        Ok(by_id.into_values().collect())
    }

    pub async fn mark_order_cancelled(
        &self,
        order_id: Uuid,
        user_address: &str,
    ) -> AppResult<()> {
        let _: serde_json::Value = self
            .post_json(
                &format!("/api/orders/{order_id}/cancelled?user_address={user_address}"),
                &serde_json::json!({}),
            )
            .await?;
        Ok(())
    }

    pub async fn index_order_from_event(
        &self,
        event: OrderCreatedEvent,
        skip_book: bool,
        status: &str,
        filled_raw: u64,
    ) -> AppResult<Order> {
        self.post_json(
            "/api/orders/index/from-event",
            &serde_json::json!({
                "event": event,
                "skip_book": skip_book,
                "status": status,
                "filled_raw": filled_raw,
            }),
        )
        .await
    }
}

fn urlencoding_encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.as_bytes() {
        match *byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*byte as char);
            }
            _ => {
                out.push('%');
                out.push(char::from(HEX[(byte >> 4) as usize]));
                out.push(char::from(HEX[(byte & 0x0f) as usize]));
            }
        }
    }
    out
}

const HEX: &[u8; 16] = b"0123456789ABCDEF";

