// Copyright (c) LightPool Labs
// Author: xiaoyu1998

//! Configuration for the liquidity maker strategy.

use nautilus_model::identifiers::{ClientId, StrategyId};
use nautilus_trading::strategy::StrategyConfig;

use super::bootstrap::MarketPair;

/// Configuration for subscribing Polymarket order book deltas with managed cache books.
#[derive(Debug, Clone)]
pub struct LiquidityMakerConfig {
    /// Base strategy configuration.
    pub base: StrategyConfig,
    /// Polymarket event slugs to mirror (resolved to instruments via data client filters).
    pub polymarket_slugs: Vec<String>,
    /// LightPool market slugs to mirror (resolved via clob-index bootstrap).
    pub lightpool_slugs: Vec<String>,
    /// Explicit PM condition_id ↔ LP slug pairs (N:N). When non-empty, used for pairing.
    pub market_pairs: Vec<MarketPair>,
    /// Number of price levels to retain per side.
    pub depth: usize,
    /// Log top-of-book snapshot every N delta batches. `0` disables logging.
    pub log_interval: u64,
    /// When `true`, subscribe with managed books so the data engine also
    /// maintains an `OrderBook` in cache (`cache.order_book()`).
    pub managed_book: bool,
    /// When `true`, print Polymarket order book snapshots from cache.
    pub log_polymarket: bool,
    /// When `true`, mirror Polymarket depth onto LightPool via execution client.
    pub trading_enabled: bool,
    /// Execution client id for LightPool order submission.
    pub lightpool_client_id: ClientId,
    /// Number of Polymarket book deltas to batch before reconciling once.
    pub reconcile_delta_batch_size: u64,
}

impl LiquidityMakerConfig {
    /// Creates a new config for the given Polymarket event slugs.
    #[must_use]
    pub fn new(polymarket_slugs: Vec<String>) -> Self {
        Self {
            base: StrategyConfig {
                strategy_id: Some(StrategyId::from("LIQUIDITY_MAKER-001")),
                order_id_tag: Some("001".to_string()),
                ..Default::default()
            },
            polymarket_slugs,
            lightpool_slugs: Vec::new(),
            market_pairs: Vec::new(),
            depth: 10,
            log_interval: 0,
            managed_book: true,
            log_polymarket: true,
            trading_enabled: false,
            lightpool_client_id: ClientId::from("LIGHTPOOL"),
            reconcile_delta_batch_size: 10,
        }
    }

    #[must_use]
    pub fn with_lightpool_slugs(mut self, lightpool_slugs: Vec<String>) -> Self {
        self.lightpool_slugs = lightpool_slugs;
        self
    }

    #[must_use]
    pub fn with_market_pairs(mut self, market_pairs: Vec<MarketPair>) -> Self {
        self.lightpool_slugs = market_pairs
            .iter()
            .map(|pair| pair.lightpool_slug.clone())
            .collect();
        self.market_pairs = market_pairs;
        self
    }

    #[must_use]
    pub fn with_depth(mut self, depth: usize) -> Self {
        self.depth = depth.max(1);
        self
    }

    #[must_use]
    pub fn with_log_interval(mut self, log_interval: u64) -> Self {
        self.log_interval = log_interval;
        self
    }

    #[must_use]
    pub fn with_managed_book(mut self, managed: bool) -> Self {
        self.managed_book = managed;
        self
    }

    #[must_use]
    pub fn with_log_polymarket(mut self, enabled: bool) -> Self {
        self.log_polymarket = enabled;
        self
    }

    #[must_use]
    pub fn with_strategy_id(mut self, strategy_id: StrategyId) -> Self {
        self.base.strategy_id = Some(strategy_id);
        self
    }

    #[must_use]
    pub fn with_trading_enabled(mut self, enabled: bool) -> Self {
        self.trading_enabled = enabled;
        self
    }

    #[must_use]
    pub fn with_lightpool_client_id(mut self, client_id: ClientId) -> Self {
        self.lightpool_client_id = client_id;
        self
    }

    #[must_use]
    pub fn with_reconcile_delta_batch_size(mut self, reconcile_delta_batch_size: u64) -> Self {
        self.reconcile_delta_batch_size = reconcile_delta_batch_size.max(1);
        self
    }
}
