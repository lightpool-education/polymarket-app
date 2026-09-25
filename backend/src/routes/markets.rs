// Copyright (c) LightPool Labs
// Author: xiaoyu1998

use std::collections::HashMap;

use axum::{
    extract::{Path, Query, State},
    routing::{get, post},
    Json, Router,
};
use lightpool_sdk::parse_token_contract;
use tokio::task::JoinSet;

use crate::auth::AuthUser;
use crate::chain::{format_token_amount, parse_order_size};
use crate::crypto_util::parse_address;
use crate::error::{AppError, AppResult};
use crate::market_metadata::MarketMetadata;
use crate::models::{Market, MarketsPage, MintBurnRequest, MintBurnResponse, QueryMarketsParams};
use crate::state::AppState;
use crate::yes_rate::yes_rate_from_book;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(query_markets))
        .route("/:slug/mint", post(mint_market))
        .route("/:slug/burn", post(burn_market))
        .route("/:slug", get(get_market))
}

async fn resolve_market(state: &AppState, slug: &str) -> AppResult<Market> {
    let market = state.clob.get_market_by_slug(slug).await?;
    enrich_market(state, market).await
}

async fn query_markets(
    State(state): State<AppState>,
    Query(params): Query<QueryMarketsParams>,
) -> AppResult<Json<MarketsPage>> {
    let mut page = state.clob.query_markets(&params).await?;
    page.markets = enrich_markets(&state, page.markets).await?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let min_deadline = now.saturating_add(7 * 24 * 60 * 60);
    page.markets
        .retain(|market| market.resolution_deadline > min_deadline);
    page.total = page.markets.len();
    Ok(Json(page))
}

async fn get_market(
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> AppResult<Json<Market>> {
    resolve_market(&state, &slug).await.map(Json)
}

async fn mint_market(
    State(state): State<AppState>,
    user: AuthUser,
    Path(slug): Path<String>,
    Json(body): Json<MintBurnRequest>,
) -> AppResult<Json<MintBurnResponse>> {
    let market = resolve_market(&state, &slug).await?;

    let amount = parse_order_size(&body.amount)?;
    let market_address = parse_token_contract(&market.market_address)
        .map_err(|e| AppError::BadRequest(format!("invalid market address: {e}")))?;
    let collateral_token = parse_token_contract(&market.collateral_token)
        .map_err(|e| AppError::BadRequest(format!("invalid collateral token: {e}")))?;
    let yes_token = parse_token_contract(&market.yes_token)
        .map_err(|e| AppError::BadRequest(format!("invalid yes token: {e}")))?;
    let no_token = parse_token_contract(&market.no_token)
        .map_err(|e| AppError::BadRequest(format!("invalid no token: {e}")))?;

    let record = state.users.get_or_create(&user.lp_address).await?;
    if !record.agent_authorized {
        return Err(AppError::BadRequest(
            "agent not authorized; call set_agent first".into(),
        ));
    }
    let agent_signer = state.users.agent_signer(&record)?;
    let account = parse_address(&record.lp_address)?;

    let result = state
        .chain
        .mint_event_contract(
            &agent_signer,
            account,
            market_address,
            collateral_token,
            yes_token,
            no_token,
            amount,
        )
        .await?;

    Ok(Json(MintBurnResponse {
        slug: market.slug,
        amount: format_token_amount(result.amount),
        tx_digest: result.tx_digest,
    }))
}

async fn burn_market(
    State(state): State<AppState>,
    user: AuthUser,
    Path(slug): Path<String>,
    Json(body): Json<MintBurnRequest>,
) -> AppResult<Json<MintBurnResponse>> {
    let market = resolve_market(&state, &slug).await?;

    let amount = parse_order_size(&body.amount)?;
    let market_address = parse_token_contract(&market.market_address)
        .map_err(|e| AppError::BadRequest(format!("invalid market address: {e}")))?;
    let collateral_token = parse_token_contract(&market.collateral_token)
        .map_err(|e| AppError::BadRequest(format!("invalid collateral token: {e}")))?;
    let yes_token = parse_token_contract(&market.yes_token)
        .map_err(|e| AppError::BadRequest(format!("invalid yes token: {e}")))?;
    let no_token = parse_token_contract(&market.no_token)
        .map_err(|e| AppError::BadRequest(format!("invalid no token: {e}")))?;

    let record = state.users.get_or_create(&user.lp_address).await?;
    if !record.agent_authorized {
        return Err(AppError::BadRequest(
            "agent not authorized; call set_agent first".into(),
        ));
    }
    let agent_signer = state.users.agent_signer(&record)?;
    let account = parse_address(&record.lp_address)?;

    let result = state
        .chain
        .burn_event_contract(
            &agent_signer,
            account,
            market_address,
            collateral_token,
            yes_token,
            no_token,
            amount,
        )
        .await?;

    Ok(Json(MintBurnResponse {
        slug: market.slug,
        amount: format_token_amount(result.amount),
        tx_digest: result.tx_digest,
    }))
}

async fn enrich_market(state: &AppState, market: Market) -> AppResult<Market> {
    let mut markets = enrich_markets(state, vec![market]).await?;
    markets
        .pop()
        .ok_or_else(|| AppError::Internal("missing enriched market".into()))
}

async fn enrich_markets(state: &AppState, mut markets: Vec<Market>) -> AppResult<Vec<Market>> {
    if markets.is_empty() {
        return Ok(markets);
    }

    let addresses: Vec<String> = markets
        .iter()
        .map(|market| market.market_address.clone())
        .collect();
    let metadata_rows = state
        .market_metadata
        .get_many_by_market_addresses(&addresses)
        .await?;
    let metadata_by_address: HashMap<String, MarketMetadata> = metadata_rows
        .into_iter()
        .map(|row| (row.market_address.to_lowercase(), row))
        .collect();

    for market in &mut markets {
        if let Some(meta) = metadata_by_address.get(&market.market_address.to_lowercase()) {
            if meta.icon_url.is_some() {
                market.icon_url = meta.icon_url.clone();
            }
            market.event_slug = meta.event_slug.clone();
            market.group_item_title = meta.group_item_title.clone();
        }
    }

    let mut join_set = JoinSet::new();
    for (index, market) in markets.iter().enumerate() {
        let clob = state.clob.clone();
        let spot = market.yes_spot_market.clone();
        join_set.spawn(async move {
            let book = clob.get_spot_book(&spot, 1).await.ok();
            let rate = book.as_ref().and_then(yes_rate_from_book);
            (index, rate)
        });
    }

    while let Some(joined) = join_set.join_next().await {
        if let Ok((index, rate)) = joined {
            if let Some(market) = markets.get_mut(index) {
                market.yes_rate = rate;
            }
        }
    }

    Ok(markets)
}
