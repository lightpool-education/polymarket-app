// Copyright (c) LightPool Labs
// Author: xiaoyu1998

//! Liquidity maker strategy: subscribe Polymarket book deltas and mirror onto LightPool via orders.

use std::fmt::Debug;
use std::num::NonZeroUsize;

use ahash::{AHashMap, AHashSet};
use nautilus_common::actor::DataActor;
use nautilus_model::{
    data::OrderBookDeltas,
    enums::BookType,
    events::{OrderAccepted, OrderCanceled, OrderFilled, OrderUpdated},
    identifiers::InstrumentId,
    instruments::{Instrument, InstrumentAny},
    orderbook::OrderBook,
};
use nautilus_trading::{nautilus_strategy, strategy::StrategyCore};

use super::config::LiquidityMakerConfig;
use super::markets::{
    SlugMarketIds, assign_lightpool_markets_to_slugs, assign_polymarket_markets_to_slugs,
    discover_lightpool_markets_from_cache, discover_polymarket_markets_from_cache,
};

const POLYMARKET_VENUE: &str = "POLYMARKET";
const LIGHTPOOL_VENUE: &str = "LIGHTPOOL";

/// Subscribes to Polymarket `OrderBookDeltas` and mirrors depth onto LightPool using own orders.
///
/// Requires `managed_book = true` so the data engine `BookUpdater` maintains
/// Polymarket `cache.order_book()` while deltas stream in.
pub struct LiquidityMaker {
    pub(super) core: StrategyCore,
    pub(super) config: LiquidityMakerConfig,
    /// Event slug -> condition_id -> YES/NO instrument ids (Polymarket).
    pub(super) polymarket_slug_markets: AHashMap<String, AHashMap<String, SlugMarketIds>>,
    /// condition_id -> YES/NO instrument ids (Polymarket).
    pub(super) polymarket_markets: AHashMap<String, SlugMarketIds>,
    /// market_slug -> YES/NO instrument ids (LightPool).
    pub(super) lightpool_markets: AHashMap<String, SlugMarketIds>,
    pub(super) instrument_to_market_key: AHashMap<InstrumentId, String>,
    pub(super) subscribed_instruments: AHashSet<InstrumentId>,
    pub(super) delta_batches: AHashMap<InstrumentId, u64>,
    /// Polymarket instrument id -> paired LightPool instrument id.
    pub(super) pm_to_lp: AHashMap<InstrumentId, InstrumentId>,
    /// Polymarket instrument ids with deltas in the current reconcile batch.
    pub(super) pending_reconcile_pm_instruments: AHashSet<InstrumentId>,
    /// Polymarket delta count in the current reconcile batch.
    pub(super) pending_reconcile_delta_count: u64,
}

impl LiquidityMaker {
    /// Creates a new [`LiquidityMaker`] from config.
    #[must_use]
    pub fn new(config: LiquidityMakerConfig) -> Self {
        Self {
            core: StrategyCore::new(config.base.clone()),
            config,
            polymarket_slug_markets: AHashMap::new(),
            polymarket_markets: AHashMap::new(),
            lightpool_markets: AHashMap::new(),
            instrument_to_market_key: AHashMap::new(),
            subscribed_instruments: AHashSet::new(),
            delta_batches: AHashMap::new(),
            pm_to_lp: AHashMap::new(),
            pending_reconcile_pm_instruments: AHashSet::new(),
            pending_reconcile_delta_count: 0,
        }
    }

    fn collect_polymarket_delta_for_reconcile(&mut self, instrument_id: InstrumentId) {
        if !self.config.trading_enabled {
            return;
        }
        self.pending_reconcile_pm_instruments.insert(instrument_id);
        self.pending_reconcile_delta_count += 1;
        if self.pending_reconcile_delta_count >= self.config.reconcile_delta_batch_size {
            self.reconcile_batched_polymarket_deltas();
        }
    }

    fn reconcile_batched_polymarket_deltas(&mut self) {
        if !self.config.trading_enabled {
            self.pending_reconcile_pm_instruments.clear();
            self.pending_reconcile_delta_count = 0;
            return;
        }
        let instrument_ids: Vec<InstrumentId> = self.pending_reconcile_pm_instruments.drain().collect();
        self.pending_reconcile_delta_count = 0;
        for instrument_id in instrument_ids {
            if let Err(e) = self.reconcile_from_polymarket_delta(instrument_id) {
                log::warn!(
                    "Failed to reconcile LightPool liquidity for {instrument_id}: {e:#}"
                );
            }
        }
    }

    fn sync_polymarket_markets_from_cache(&mut self) -> usize {
        let discovered = discover_polymarket_markets_from_cache(&self.cache());
        let new_count = assign_polymarket_markets_to_slugs(
            &self.config.polymarket_slugs,
            &discovered,
            &mut self.polymarket_markets,
            &mut self.polymarket_slug_markets,
        );
        log::info!(
            "sync_polymarket_markets_from_cache discovered={} assigned_new={} \
             polymarket_markets={}",
            discovered.len(),
            new_count,
            self.polymarket_markets.len(),
        );
        for (condition_id, market) in &self.polymarket_markets {
            log::info!(
                "polymarket market condition={condition_id} yes={} no={}",
                market.yes_id,
                market.no_id,
            );
        }
        new_count
    }

    fn sync_lightpool_markets_from_cache(&mut self) -> usize {
        if self.config.lightpool_slugs.is_empty() {
            return 0;
        }
        let discovered = discover_lightpool_markets_from_cache(&self.cache());
        assign_lightpool_markets_to_slugs(
            &self.config.lightpool_slugs,
            &discovered,
            &mut self.lightpool_markets,
        )
    }

    fn sync_markets_from_cache(&mut self) -> usize {
        self.sync_polymarket_markets_from_cache() + self.sync_lightpool_markets_from_cache()
    }

    fn subscribe_market(
        &mut self,
        market: &SlugMarketIds,
        depth: NonZeroUsize,
        venue_label: &str,
    ) {
        for instrument_id in [market.yes_id, market.no_id] {
            if self.subscribed_instruments.contains(&instrument_id) {
                continue;
            }
            self.subscribe_book_deltas(
                instrument_id,
                BookType::L2_MBP,
                Some(depth),
                None,
                true,
                None,
            );
            self.instrument_to_market_key
                .insert(instrument_id, market.condition_id.clone());
            self.subscribed_instruments.insert(instrument_id);
            log::info!(
                "Subscribed to {venue_label} order book deltas instrument_id={instrument_id} \
                 market_key={} depth={}",
                market.condition_id,
                self.config.depth,
            );
        }
    }

    fn reconcile_subscriptions(&mut self) {
        let Some(depth) = NonZeroUsize::new(self.config.depth.max(1)) else {
            return;
        };

        let polymarket_markets: Vec<SlugMarketIds> =
            self.polymarket_markets.values().cloned().collect();
        for market in polymarket_markets {
            self.subscribe_market(&market, depth, "Polymarket");
        }
    }

    fn venue_label(instrument_id: InstrumentId) -> &'static str {
        match instrument_id.venue.as_str() {
            POLYMARKET_VENUE => "Polymarket",
            LIGHTPOOL_VENUE => "Lightpool",
            _ => "Unknown",
        }
    }

}

fn format_book_side(book: &OrderBook, bids: bool, depth: usize) -> String {
    let levels: Vec<String> = if bids {
        book.bids(Some(depth))
            .map(|level| format!("{}@{}", level.size(), level.price))
            .collect()
    } else {
        book.asks(Some(depth))
            .map(|level| format!("{}@{}", level.size(), level.price))
            .collect()
    };

    if levels.is_empty() {
        "-".into()
    } else {
        levels.join(", ")
    }
}

nautilus_strategy!(LiquidityMaker, {
    // fn on_order_accepted(&mut self, event: OrderAccepted) {
    //     self.maybe_reconcile_lightpool(event.instrument_id);
    // }

    // fn on_order_updated(&mut self, event: OrderUpdated) {
    //     self.maybe_reconcile_lightpool(event.instrument_id);
    // }
});

impl Debug for LiquidityMaker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct(stringify!(LiquidityMaker))
            .field("polymarket_slugs", &self.config.polymarket_slugs)
            .field("lightpool_slugs", &self.config.lightpool_slugs)
            .field("depth", &self.config.depth)
            .field("polymarket_markets", &self.polymarket_markets.len())
            .field("lightpool_markets", &self.lightpool_markets.len())
            .finish()
    }
}

impl DataActor for LiquidityMaker {
    fn on_start(&mut self) -> anyhow::Result<()> {
        if !self.config.managed_book {
            anyhow::bail!("LiquidityMaker requires managed_book=true to read cache order books");
        }

        let synced = self.sync_markets_from_cache();
        log::info!(
            "LiquidityMaker started polymarket_slugs={:?} lightpool_slugs={:?} \
             synced_markets={synced} polymarket_markets={} lightpool_markets={}",
            self.config.polymarket_slugs,
            self.config.lightpool_slugs,
            self.polymarket_markets.len(),
            self.lightpool_markets.len(),
        );
        self.reconcile_subscriptions();
        self.rebuild_instrument_pairs();
        if self.config.trading_enabled {
            log::info!(
                "LightPool mirroring enabled depth={} client_id={} reconcile_delta_batch_size={}",
                self.config.depth,
                self.config.lightpool_client_id,
                self.config.reconcile_delta_batch_size,
            );
        }
        Ok(())
    }

    fn on_stop(&mut self) -> anyhow::Result<()> {
        log::info!("LiquidityMaker stopping; unsubscribing Polymarket book deltas");
        for instrument_id in self.subscribed_instruments.clone() {
            self.unsubscribe_book_deltas(instrument_id, None, None);
        }
        Ok(())
    }

    fn on_book_deltas(&mut self, deltas: &OrderBookDeltas) -> anyhow::Result<()> {
        log::info!("OrderBookDeltas {:?}", deltas);

        let instrument_id = deltas.instrument_id;
        let venue = instrument_id.venue.as_str();

        if venue == POLYMARKET_VENUE {
            self.collect_polymarket_delta_for_reconcile(instrument_id);
        }

        Ok(())
    }

}
