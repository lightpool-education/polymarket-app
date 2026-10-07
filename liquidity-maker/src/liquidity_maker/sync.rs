// Copyright (c) LightPool Labs
// Author: xiaoyu1998

//! Reconcile LightPool resting liquidity against Polymarket cache books using bot-owned orders.

use indexmap::IndexMap;
use nautilus_model::{
    enums::OrderSide,
    identifiers::{ClientOrderId, InstrumentId, Venue},
    orderbook::OrderBook,
    orders::{Order, OrderAny},
    types::{Price, Quantity},
};
use nautilus_trading::strategy::Strategy;
use rust_decimal::Decimal;

use super::markets::{
    instrument_outcome_side, instrument_spot_market, resolve_yes_no_pair,
};
use super::strategy::LiquidityMaker;
use crate::SlugMarketIds;

const LIGHTPOOL_VENUE: &str = "LIGHTPOOL";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct BookSideSnapshot {
    pub levels: IndexMap<Decimal, Decimal>,
}

impl BookSideSnapshot {
    fn from_book(book: &OrderBook, depth: usize, bids: bool) -> Self {
        let levels = if bids {
            book.bids_as_map(Some(depth))
        } else {
            book.asks_as_map(Some(depth))
        };
        Self { levels }
    }

    fn from_orders(orders: &[OrderAny], depth: usize, bids: bool) -> Self {
        let side = if bids {
            OrderSide::Buy
        } else {
            OrderSide::Sell
        };
        let mut totals: IndexMap<Decimal, Decimal> = IndexMap::new();
        for order in orders {
            if order.order_side() != side {
                continue;
            }
            let Some(price) = order.price().map(|p| p.as_decimal()) else {
                continue;
            };
            let qty = order.quantity().as_decimal();
            if qty.is_zero() {
                continue;
            }
            *totals.entry(price).or_insert(Decimal::ZERO) += qty;
        }
        Self {
            levels: trim_side_levels(totals, depth, bids),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct BookSnapshot {
    pub bids: BookSideSnapshot,
    pub asks: BookSideSnapshot,
}

impl BookSnapshot {
    fn from_book(book: &OrderBook, depth: usize) -> Self {
        Self {
            bids: BookSideSnapshot::from_book(book, depth, true),
            asks: BookSideSnapshot::from_book(book, depth, false),
        }
    }

    fn from_open_orders(orders: &[OrderAny], depth: usize) -> Self {
        Self {
            bids: BookSideSnapshot::from_orders(orders, depth, true),
            asks: BookSideSnapshot::from_orders(orders, depth, false),
        }
    }
}

fn trim_side_levels(
    levels: IndexMap<Decimal, Decimal>,
    depth: usize,
    bids: bool,
) -> IndexMap<Decimal, Decimal> {
    let mut keys: Vec<Decimal> = levels.keys().copied().collect();
    if bids {
        keys.sort_by(|a, b| b.cmp(a));
    } else {
        keys.sort();
    }
    keys.truncate(depth);
    keys.into_iter()
        .filter_map(|price| {
            levels
                .get(&price)
                .copied()
                .map(|size| (price, size))
        })
        .collect()
}

pub(super) fn books_match(reference: &BookSnapshot, actual: &BookSnapshot) -> bool {
    reference.bids.levels == actual.bids.levels && reference.asks.levels == actual.asks.levels
}

#[derive(Debug, Clone)]
struct LevelOrder {
    client_order_id: ClientOrderId,
    quantity: Quantity,
    has_venue_id: bool,
}

#[derive(Debug, Default)]
struct OrdersByLevel {
    bids: IndexMap<Decimal, Vec<LevelOrder>>,
    asks: IndexMap<Decimal, Vec<LevelOrder>>,
}

impl OrdersByLevel {
    fn from_open_orders(orders: &[OrderAny]) -> Self {
        let mut grouped = Self::default();
        for order in orders {
            let Some(price) = order.price().map(|p| p.as_decimal()) else {
                continue;
            };
            let entry = LevelOrder {
                client_order_id: order.client_order_id(),
                quantity: order.quantity(),
                has_venue_id: order.venue_order_id().is_some(),
            };
            match order.order_side() {
                OrderSide::Buy => grouped.bids.entry(price).or_default().push(entry),
                OrderSide::Sell => grouped.asks.entry(price).or_default().push(entry),
                _ => {}
            }
        }
        grouped
    }
}

#[derive(Debug)]
enum ReconcileAction {
    Place {
        side: OrderSide,
        price: Price,
        quantity: Quantity,
    },
    Cancel {
        client_order_id: ClientOrderId,
    },
    Modify {
        client_order_id: ClientOrderId,
        quantity: Quantity,
    },
}

fn diff_side(
    side: OrderSide,
    reference: &IndexMap<Decimal, Decimal>,
    actual: &IndexMap<Decimal, Decimal>,
    orders: &IndexMap<Decimal, Vec<LevelOrder>>,
    actions: &mut Vec<ReconcileAction>,
) {
    let mut prices: Vec<Decimal> = reference.keys().chain(actual.keys()).copied().collect();
    prices.sort();
    prices.dedup();

    for price in prices {
        let ref_qty = reference.get(&price).copied().unwrap_or(Decimal::ZERO);
        let act_qty = actual.get(&price).copied().unwrap_or(Decimal::ZERO);
        if ref_qty == act_qty {
            continue;
        }

        let level_orders = orders.get(&price).cloned().unwrap_or_default();
        let Ok(price_value) = Price::from_decimal(price) else {
            continue;
        };

        if act_qty.is_zero() {
            let Ok(quantity) = Quantity::from_decimal(ref_qty) else {
                continue;
            };
            if quantity.is_zero() {
                continue;
            }
            actions.push(ReconcileAction::Place {
                side,
                price: price_value,
                quantity,
            });
            continue;
        }

        if ref_qty.is_zero() {
            for order in level_orders {
                if order.has_venue_id {
                    actions.push(ReconcileAction::Cancel {
                        client_order_id: order.client_order_id,
                    });
                }
            }
            continue;
        }

        let actionable: Vec<_> = level_orders
            .into_iter()
            .filter(|order| order.has_venue_id)
            .collect();
        if actionable.len() == 1 {
            let Ok(quantity) = Quantity::from_decimal(ref_qty) else {
                continue;
            };
            if quantity.is_zero() {
                continue;
            }
            actions.push(ReconcileAction::Modify {
                client_order_id: actionable[0].client_order_id,
                quantity,
            });
            continue;
        }

        for order in actionable {
            actions.push(ReconcileAction::Cancel {
                client_order_id: order.client_order_id,
            });
        }
        let Ok(quantity) = Quantity::from_decimal(ref_qty) else {
            continue;
        };
        if !quantity.is_zero() {
            actions.push(ReconcileAction::Place {
                side,
                price: price_value,
                quantity,
            });
        }
    }
}

fn build_reconcile_actions(
    reference: &BookSnapshot,
    actual: &BookSnapshot,
    orders_by_level: &OrdersByLevel,
) -> Vec<ReconcileAction> {
    let mut actions = Vec::new();
    diff_side(
        OrderSide::Buy,
        &reference.bids.levels,
        &actual.bids.levels,
        &orders_by_level.bids,
        &mut actions,
    );
    diff_side(
        OrderSide::Sell,
        &reference.asks.levels,
        &actual.asks.levels,
        &orders_by_level.asks,
        &mut actions,
    );
    actions
}

impl LiquidityMaker {
    pub(super) fn rebuild_instrument_pairs(&mut self) {
        self.pm_to_lp.clear();
        if self.config.lightpool_slugs.is_empty() || !self.config.trading_enabled {
            return;
        }

        let new_pairs = {
            let cache = self.cache();
            let mut pairs = Vec::new();

            if !self.config.market_pairs.is_empty() {
                // N:N — each PM condition_id maps to its own LP slug.
                for pair in &self.config.market_pairs {
                    let Some(pm_market) = self.polymarket_markets.get(&pair.condition_id).cloned()
                    else {
                        log::warn!(
                            "missing Polymarket instruments for condition={}",
                            pair.condition_id
                        );
                        continue;
                    };
                    let Some(lp_market) =
                        self.lightpool_markets.get(&pair.lightpool_slug).cloned()
                    else {
                        log::warn!(
                            "missing LightPool instruments for slug={}",
                            pair.lightpool_slug
                        );
                        continue;
                    };

                    let Some((pm_yes_id, pm_no_id)) = resolve_yes_no_pair(&cache, &pm_market)
                    else {
                        continue;
                    };
                    let Some((lp_yes_id, lp_no_id)) = resolve_yes_no_pair(&cache, &lp_market)
                    else {
                        continue;
                    };

                    log::info!(
                        "pm_to_lp N:N condition={} -> slug={} \
                         pm_yes={pm_yes_id} lp_yes={lp_yes_id} \
                         pm_no={pm_no_id} lp_no={lp_no_id}",
                        pair.condition_id,
                        pair.lightpool_slug,
                    );
                    pairs.push((pm_yes_id, lp_yes_id));
                    pairs.push((pm_no_id, lp_no_id));
                }
            } else {
                // Legacy M:1 — all PM conditions under first event slug → first LP slug.
                let Some(lp_slug) = self.config.lightpool_slugs.first() else {
                    return;
                };
                let Some(lp_market) = self.lightpool_markets.get(lp_slug).cloned() else {
                    return;
                };
                let Some(pm_slug) = self.config.polymarket_slugs.first() else {
                    return;
                };
                let Some(pm_markets) = self.polymarket_slug_markets.get(pm_slug) else {
                    return;
                };
                let pm_markets: Vec<SlugMarketIds> = pm_markets.values().cloned().collect();
                let Some((lp_yes_id, lp_no_id)) = resolve_yes_no_pair(&cache, &lp_market) else {
                    return;
                };

                for pm_market in &pm_markets {
                    let Some((pm_yes_id, pm_no_id)) = resolve_yes_no_pair(&cache, pm_market)
                    else {
                        continue;
                    };
                    log::debug!(
                        "pm_to_lp M:1 condition={} -> slug={lp_slug}",
                        pm_market.condition_id,
                    );
                    pairs.push((pm_yes_id, lp_yes_id));
                    pairs.push((pm_no_id, lp_no_id));
                }
            }

            pairs
        };

        for (pm_id, lp_id) in new_pairs {
            self.pm_to_lp.insert(pm_id, lp_id);
        }
    }

    fn lightpool_for_polymarket(&self, polymarket_instrument_id: InstrumentId) -> Option<InstrumentId> {
        self.pm_to_lp.get(&polymarket_instrument_id).copied()
    }

    fn polymarket_for_lightpool(&self, lightpool_instrument_id: InstrumentId) -> Option<InstrumentId> {
        self.pm_to_lp
            .iter()
            .find_map(|(pm, lp)| (*lp == lightpool_instrument_id).then_some(*pm))
    }

    pub(super) fn reconcile_from_polymarket_delta(
        &mut self,
        polymarket_instrument_id: InstrumentId,
    ) -> anyhow::Result<()> {
        let Some(lightpool_instrument_id) =
            self.lightpool_for_polymarket(polymarket_instrument_id)
        else {
            return Ok(());
        };
        self.reconcile_pair(polymarket_instrument_id, lightpool_instrument_id)
    }

    fn reconcile_pair(
        &mut self,
        polymarket_instrument_id: InstrumentId,
        lightpool_instrument_id: InstrumentId,
    ) -> anyhow::Result<()> {
        if !self.config.trading_enabled || self.config.lightpool_slugs.is_empty() {
            return Ok(());
        }

        let depth = self.config.depth.max(1);
        let strategy_id = self.core.strategy_id();
        let client_id = self.config.lightpool_client_id;
        let venue = Venue::from(LIGHTPOOL_VENUE);

        let actions = {
            let cache = self.cache();
            let Some(polymarket_book) = cache.order_book(&polymarket_instrument_id) else {
                log::warn!(
                    "Polymarket book missing in cache for {polymarket_instrument_id}; skip reconcile"
                );
                return Ok(());
            };

            let Some(strategy_id) = strategy_id else {
                return Ok(());
            };

            let inflight = cache
                .orders_inflight(
                    Some(&venue),
                    Some(&lightpool_instrument_id),
                    Some(&strategy_id),
                    None,
                    None,
                );
            if !inflight.is_empty() {
                return Ok(());
            }

            let open_orders = cache
                .orders_open(
                    Some(&venue),
                    Some(&lightpool_instrument_id),
                    Some(&strategy_id),
                    None,
                    None,
                )
                .into_iter()
                .map(|order| order.cloned())
                .collect::<Vec<_>>();

            let reference = BookSnapshot::from_book(polymarket_book, depth);
            let actual = BookSnapshot::from_open_orders(&open_orders, depth);

            if books_match(&reference, &actual) {
                return Ok(());
            }

            let orders_by_level = OrdersByLevel::from_open_orders(&open_orders);
            build_reconcile_actions(&reference, &actual, &orders_by_level)
        };

        if actions.is_empty() {
            return Ok(());
        }

        for action in actions {
            match action {
                ReconcileAction::Cancel { client_order_id } => {
                    // if let Err(e) = self.cancel_order(client_order_id, Some(client_id), None) {
                    //     log::warn!("Failed to cancel {client_order_id}: {e}");
                    // }
                    let _ = client_order_id;
                }
                ReconcileAction::Modify {
                    client_order_id,
                    quantity,
                } => {
                    if let Err(e) = self.modify_order(
                        client_order_id,
                        Some(quantity),
                        None,
                        None,
                        Some(client_id),
                        None,
                    ) {
                        log::warn!("Failed to modify {client_order_id}: {e}");
                    }
                }
                ReconcileAction::Place {
                    side,
                    price,
                    quantity,
                } => {
                    if quantity.is_zero() {
                        continue;
                    }
                    let order = self.core.order_factory().limit(
                        lightpool_instrument_id,
                        side,
                        quantity,
                        price,
                        None,
                        None,
                        None,
                        None,
                        None,
                        None,
                        None,
                        None,
                        None,
                        None,
                        None,
                        None,
                    );
                    if let Err(e) = self.submit_order(order, None, Some(client_id), None) {
                        log::warn!(
                            "Failed to submit mirror order on {lightpool_instrument_id} {:?} {}@{}: {e}",
                            side,
                            quantity,
                            price,
                        );
                    }
                }
            }
        }

        Ok(())
    }
}
