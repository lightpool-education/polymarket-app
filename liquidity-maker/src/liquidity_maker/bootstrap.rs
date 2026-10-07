// Copyright (c) LightPool Labs
// Author: xiaoyu1998

//! Bootstrap LightPool markets from top-N Polymarket markets in an event.

use anyhow::{Context, Result, bail};
use nautilus_lightpool::{
    common::signer::signer_from_private_key,
    config::{clob_index_http_from_env, resolve_collateral_token, resolve_private_key},
    http::bootstrap::{BootstrappedMarket, bootstrap_one_market},
    http::clob_index::ClobIndexHttpClient,
};
use nautilus_network::retry::RetryConfig;
use nautilus_polymarket::{
    config::proxy_url_from_env,
    http::{
        gamma::PolymarketGammaHttpClient,
        models::GammaMarket,
        query::{GetGammaEventsParams, GetGammaMarketsParams},
    },
};

/// One Polymarket condition paired with a LightPool market slug.
#[derive(Debug, Clone)]
pub struct MarketPair {
    pub condition_id: String,
    pub question: String,
    pub lightpool_slug: String,
    pub market_address: String,
}

#[derive(Debug, Clone)]
pub struct BootstrapConfig {
    pub polymarket_event_slugs: Vec<String>,
    pub max_markets: u32,
    pub mint_amount: u64,
    pub order_field: String,
    /// Skip markets where any Yes/No outcome price is >= this value (0–1 scale).
    /// Default 0.96 (= 96¢) keeps only still-active markets.
    pub max_outcome_price: f64,
}

impl Default for BootstrapConfig {
    fn default() -> Self {
        Self {
            polymarket_event_slugs: Vec::new(),
            max_markets: 5,
            mint_amount: 1_000_000_000_000_000, // 1e9 tokens at 6 decimals
            order_field: "liquidity".into(),
            max_outcome_price: 0.96,
        }
    }
}

fn parse_deadline_unix(raw: &str) -> Option<u64> {
    // Accept "2026-12-31T23:59:59Z" or with fractional seconds.
    let cleaned = raw.trim_end_matches('Z');
    let cleaned = cleaned.split('.').next().unwrap_or(cleaned);
    if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(cleaned, "%Y-%m-%dT%H:%M:%S") {
        return Some(dt.and_utc().timestamp().max(0) as u64);
    }
    if let Ok(d) = chrono::NaiveDate::parse_from_str(raw, "%Y-%m-%d") {
        return d
            .and_hms_opt(23, 59, 59)
            .map(|dt| dt.and_utc().timestamp().max(0) as u64);
    }
    None
}

fn parse_resolution_deadline(end_date: Option<&str>) -> u64 {
    // Prefer ISO date; fall back to end of 2026.
    const FALLBACK: u64 = 1_798_761_599; // 2026-12-31T23:59:59Z approx
    end_date
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .and_then(parse_deadline_unix)
        .unwrap_or(FALLBACK)
}

const MIN_DEADLINE_HORIZON_SECS: u64 = 7 * 24 * 60 * 60;

fn deadline_still_open(end_date: Option<&str>) -> bool {
    let Some(deadline) = end_date
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .and_then(parse_deadline_unix)
    else {
        return false;
    };
    let now = chrono::Utc::now().timestamp().max(0) as u64;
    deadline > now.saturating_add(MIN_DEADLINE_HORIZON_SECS)
}

fn parse_outcome_prices(raw: &Option<String>) -> Option<Vec<f64>> {
    let raw = raw.as_ref()?.trim();
    let body = raw
        .strip_prefix('[')
        .and_then(|s| s.strip_suffix(']'))?
        .trim();
    if body.is_empty() {
        return None;
    }
    let mut values = Vec::new();
    for part in body.split(',') {
        let token = part.trim().trim_matches('"').trim_matches('\'');
        values.push(token.parse::<f64>().ok()?);
    }
    (!values.is_empty()).then_some(values)
}

fn gamma_client() -> Result<PolymarketGammaHttpClient> {
    let proxy = proxy_url_from_env().or_else(|| Some("http://127.0.0.1:8118".into()));
    PolymarketGammaHttpClient::new_with_proxy(None, proxy, 30, RetryConfig::default())
        .context("create Polymarket Gamma HTTP client")
}

/// Active Polymarket event slugs ranked by 24-hour volume, highest first.
pub async fn fetch_hottest_event_slugs(limit: u32) -> Result<Vec<String>> {
    let limit = limit.max(1);
    let client = gamma_client()?;
    let params = GetGammaEventsParams {
        active: Some(true),
        closed: Some(false),
        archived: Some(false),
        order: Some("volume24hr".into()),
        ascending: Some(false),
        limit: Some(limit.saturating_mul(5).max(limit)),
        ..Default::default()
    };
    let events = client
        .inner()
        .get_gamma_events(params)
        .await
        .context("fetch hottest Polymarket events by volume24hr")?;

    let mut slugs = Vec::new();
    for event in events {
        if event.closed.unwrap_or(false) || event.archived.unwrap_or(false) {
            continue;
        }
        let event_open = if event.markets.is_empty() {
            deadline_still_open(event.end_date.as_deref())
        } else {
            event
                .markets
                .iter()
                .any(|market| deadline_still_open(market.end_date.as_deref()))
        };
        if !event_open {
            log::info!(
                "skip event past deadline slug={} title={} end={}",
                event.slug.as_deref().unwrap_or(""),
                event.title.as_deref().unwrap_or(""),
                event.end_date.as_deref().unwrap_or(""),
            );
            continue;
        }
        if !event.markets.is_empty() && !event.markets.iter().any(has_order_book) {
            log::info!(
                "skip event without order book slug={} title={}",
                event.slug.as_deref().unwrap_or(""),
                event.title.as_deref().unwrap_or(""),
            );
            continue;
        }
        let Some(slug) = event.slug.as_deref().map(str::trim).filter(|s| !s.is_empty()) else {
            continue;
        };
        log::info!(
            "hot event rank={} slug={slug} title={} volume24hr={}",
            slugs.len() + 1,
            event.title.as_deref().unwrap_or(""),
            event.volume_24hr.unwrap_or(0.0),
        );
        slugs.push(slug.to_string());
        if slugs.len() >= limit as usize {
            break;
        }
    }
    if slugs.is_empty() {
        bail!("no active Polymarket events returned for volume24hr ranking");
    }
    Ok(slugs)
}

fn normalize_outcome_price(price: f64) -> f64 {
    // Gamma usually returns 0–1; accept cents (e.g. 96) as well.
    if price > 1.5 {
        price / 100.0
    } else {
        price
    }
}

/// Closed markets and markets with no CLOB book cannot be mirrored.
fn has_order_book(market: &GammaMarket) -> bool {
    if market.closed.unwrap_or(false) {
        return false;
    }
    if market.enable_order_book == Some(false) || market.accepting_orders == Some(false) {
        return false;
    }
    market.best_bid.is_some() || market.best_ask.is_some()
}

/// Still-active market: every Yes/No outcome price is strictly below `max_price` (0–1).
fn is_still_active_market(market: &GammaMarket, max_price: f64) -> bool {
    let Some(prices) = parse_outcome_prices(&market.outcome_prices) else {
        return false;
    };
    if prices.is_empty() {
        return false;
    }
    prices
        .iter()
        .copied()
        .map(normalize_outcome_price)
        .all(|price| price < max_price)
}

async fn fetch_top_polymarket_markets(
    event_slug: &str,
    max_markets: u32,
    order_field: &str,
    max_outcome_price: f64,
) -> Result<Vec<GammaMarket>> {
    let client = gamma_client()?;

    // Fetch full sorted list first, then filter near-final markets, then take top-N.
    let params = GetGammaMarketsParams {
        active: Some(true),
        closed: Some(false),
        archived: Some(false),
        order: Some(order_field.to_string()),
        ascending: Some(false),
        max_markets: None,
        ..Default::default()
    };

    let markets = client
        .request_gamma_markets_by_event_query(event_slug, params)
        .await
        .with_context(|| format!("fetch Polymarket markets for event '{event_slug}'"))?;

    if markets.is_empty() {
        log::warn!("no Polymarket markets found for event '{event_slug}'");
        return Ok(Vec::new());
    }

    let mut active = Vec::new();
    for market in markets {
        if !deadline_still_open(market.end_date.as_deref()) {
            log::info!(
                "skip PM market past deadline condition={} question={} end={}",
                market.condition_id,
                market.question,
                market.end_date.as_deref().unwrap_or(""),
            );
            continue;
        }
        if !has_order_book(&market) {
            log::info!(
                "skip PM market without order book condition={} question={}",
                market.condition_id,
                market.question,
            );
            continue;
        }
        if is_still_active_market(&market, max_outcome_price) {
            active.push(market);
        } else {
            log::info!(
                "skip near-final PM market condition={} prices={:?} (need all < {max_outcome_price})",
                market.condition_id,
                market.outcome_prices,
            );
        }
        if active.len() >= max_markets as usize {
            break;
        }
    }

    if active.is_empty() {
        log::warn!(
            "no still-active Polymarket markets for event '{event_slug}' \
             (all outcomes must be < {max_outcome_price})"
        );
    }
    Ok(active)
}

/// Fetch top-N Polymarket markets, create+mint matching LightPool markets, return pairs.
pub async fn bootstrap_markets_from_polymarket(
    config: &BootstrapConfig,
) -> Result<Vec<MarketPair>> {
    let private_key = resolve_private_key().context("resolve LIGHTPOOL_PRIVATE_KEY")?;
    let signer = signer_from_private_key(&private_key)?;
    let collateral = resolve_collateral_token();
    let clob = ClobIndexHttpClient::new(clob_index_http_from_env());

    if config.polymarket_event_slugs.is_empty() {
        bail!("bootstrap requires at least one Polymarket event slug");
    }

    let mut pm_markets = Vec::new();
    for event_slug in &config.polymarket_event_slugs {
        let markets = fetch_top_polymarket_markets(
            event_slug,
            config.max_markets,
            &config.order_field,
            config.max_outcome_price,
        )
        .await?;
        log::info!(
            "event '{event_slug}' contributes {} still-active markets (cap {})",
            markets.len(),
            config.max_markets,
        );
        pm_markets.extend(markets);
    }

    log::info!(
        "Bootstrapping {} LightPool markets from {} Polymarket events \
         (mint_amount={}, max_outcome_price={})",
        pm_markets.len(),
        config.polymarket_event_slugs.len(),
        config.mint_amount,
        config.max_outcome_price,
    );

    let existing = clob
        .fetch_all_markets()
        .await
        .context("list existing LightPool markets")?;
    let mut existing_by_question: std::collections::HashMap<String, (String, String)> =
        std::collections::HashMap::new();
    for market in existing {
        existing_by_question
            .entry(market.question.trim().to_string())
            .or_insert((market.slug, market.market_address));
    }

    let mut pairs = Vec::with_capacity(pm_markets.len());
    let mut token_cap_reached = false;
    for (idx, pm) in pm_markets.iter().enumerate() {
        let question = pm.question.trim();
        if question.is_empty() {
            log::warn!("skip PM market {}: empty question", pm.condition_id);
            continue;
        }
        if let Some((slug, market_address)) = existing_by_question.get(question) {
            log::info!(
                "[{}/{}] reuse LP market slug={slug} condition={}",
                idx + 1,
                pm_markets.len(),
                pm.condition_id,
            );
            pairs.push(MarketPair {
                condition_id: pm.condition_id.clone(),
                question: question.to_string(),
                lightpool_slug: slug.clone(),
                market_address: market_address.clone(),
            });
            continue;
        }
        if token_cap_reached {
            log::warn!(
                "skip PM market {}: education token index cap already reached",
                pm.condition_id
            );
            continue;
        }

        let deadline = parse_resolution_deadline(pm.end_date.as_deref());
        log::info!(
            "[{}/{}] create+mint LP market for condition={} question={question}",
            idx + 1,
            pm_markets.len(),
            pm.condition_id,
        );

        let created = match bootstrap_one_market(
            &clob,
            &signer,
            question,
            &collateral,
            deadline,
            config.mint_amount,
        )
        .await
        {
            Ok(created) => created,
            Err(err) => {
                let rendered = format!("{err:#}");
                if rendered.contains("MAX_MODULE_INDEX") || rendered.contains("Cannot create more tokens")
                {
                    token_cap_reached = true;
                    log::warn!(
                        "stop creating markets at condition={}: {rendered}",
                        pm.condition_id
                    );
                    continue;
                }
                return Err(err).with_context(|| {
                    format!(
                        "bootstrap LightPool market for condition={}",
                        pm.condition_id
                    )
                });
            }
        };

        log::info!(
            "indexed LightPool market slug={} address={}",
            created.slug,
            created.market_address,
        );

        existing_by_question.insert(
            question.to_string(),
            (created.slug.clone(), created.market_address.clone()),
        );
        pairs.push(MarketPair {
            condition_id: pm.condition_id.clone(),
            question: question.to_string(),
            lightpool_slug: created.slug,
            market_address: created.market_address,
        });
    }

    if pairs.is_empty() {
        bail!("bootstrap produced zero LightPool markets");
    }
    Ok(pairs)
}
