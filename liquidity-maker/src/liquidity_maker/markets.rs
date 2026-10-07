// Copyright (c) LightPool Labs
// Author: xiaoyu1998

//! Slug-to-market id mapping built from Polymarket and LightPool instrument caches.

use ahash::AHashMap;
use nautilus_common::cache::Cache;
use nautilus_model::{
    identifiers::{InstrumentId, Venue},
    instruments::{Instrument, InstrumentAny},
};
use rust_decimal::Decimal;

/// YES/NO instrument ids for a single prediction market.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlugMarketIds {
    /// Polymarket condition id or LightPool market slug.
    pub condition_id: String,
    pub yes_id: InstrumentId,
    pub no_id: InstrumentId,
}

/// Builds paired YES/NO markets from Polymarket binary options in cache.
pub fn discover_polymarket_markets_from_cache(cache: &Cache) -> AHashMap<String, SlugMarketIds> {
    let venue = Venue::from("POLYMARKET");
    let mut by_condition: AHashMap<String, (Option<InstrumentId>, Option<InstrumentId>)> =
        AHashMap::new();

    for instrument in cache.instruments(&venue, None) {
        let InstrumentAny::BinaryOption(opt) = instrument else {
            continue;
        };
        let id = instrument.id();
        let sym = id.symbol.as_str();
        let Some((condition_id, _token_id)) = sym.split_once('-') else {
            continue;
        };
        let outcome = opt
            .outcome
            .map(|v| v.to_string().to_lowercase())
            .unwrap_or_default();
        let entry = by_condition
            .entry(condition_id.to_string())
            .or_insert((None, None));
        match outcome.as_str() {
            "yes" | "up" => entry.0 = Some(id),
            "no" | "down" => entry.1 = Some(id),
            _ => {}
        }
    }

    let mut markets = AHashMap::new();
    for (condition_id, (yes_id, no_id)) in by_condition {
        let (Some(yes_id), Some(no_id)) = (yes_id, no_id) else {
            continue;
        };
        markets.insert(
            condition_id.clone(),
            SlugMarketIds {
                condition_id,
                yes_id,
                no_id,
            },
        );
    }
    markets
}

/// Builds paired YES/NO markets from LightPool binary options in cache.
pub fn discover_lightpool_markets_from_cache(cache: &Cache) -> AHashMap<String, SlugMarketIds> {
    let venue = Venue::from("LIGHTPOOL");
    let mut by_slug: AHashMap<String, (Option<InstrumentId>, Option<InstrumentId>)> =
        AHashMap::new();

    for instrument in cache.instruments(&venue, None) {
        let InstrumentAny::BinaryOption(opt) = instrument else {
            continue;
        };
        let id = instrument.id();
        let market_slug = opt
            .info
            .as_ref()
            .and_then(|params| params.get("market_slug"))
            .and_then(|value| value.as_str())
            .map(str::to_string)
            .unwrap_or_else(|| market_slug_from_symbol(id.symbol.as_str()));
        let outcome = opt
            .outcome
            .map(|v| v.to_string().to_lowercase())
            .unwrap_or_default();
        let entry = by_slug
            .entry(market_slug)
            .or_insert((None, None));
        match outcome.as_str() {
            "yes" => entry.0 = Some(id),
            "no" => entry.1 = Some(id),
            _ => {}
        }
    }

    let mut markets = AHashMap::new();
    for (market_slug, (yes_id, no_id)) in by_slug {
        let (Some(yes_id), Some(no_id)) = (yes_id, no_id) else {
            continue;
        };
        markets.insert(
            market_slug.clone(),
            SlugMarketIds {
                condition_id: market_slug,
                yes_id,
                no_id,
            },
        );
    }
    markets
}

fn market_slug_from_symbol(symbol: &str) -> String {
    symbol
        .strip_suffix("-YES")
        .or_else(|| symbol.strip_suffix("-NO"))
        .unwrap_or(symbol)
        .to_string()
}

/// Assigns discovered LightPool markets to configured slugs.
pub fn assign_lightpool_markets_to_slugs(
    slugs: &[String],
    discovered: &AHashMap<String, SlugMarketIds>,
    slug_markets: &mut AHashMap<String, SlugMarketIds>,
) -> usize {
    let mut new_count = 0;
    for slug in slugs {
        let Some(market) = discovered.get(slug) else {
            continue;
        };
        if slug_markets.insert(slug.clone(), market.clone()).is_none() {
            new_count += 1;
        }
    }
    new_count
}

/// Assigns discovered Polymarket markets to the flat condition map and configured event slugs.
pub fn assign_polymarket_markets_to_slugs(
    slugs: &[String],
    discovered: &AHashMap<String, SlugMarketIds>,
    markets: &mut AHashMap<String, SlugMarketIds>,
    slug_markets: &mut AHashMap<String, AHashMap<String, SlugMarketIds>>,
) -> usize {
    let mut new_count = 0;
    for (condition_id, market) in discovered {
        if markets
            .insert(condition_id.clone(), market.clone())
            .is_none()
        {
            new_count += 1;
        }
    }

    if slugs.is_empty() {
        return new_count;
    }

    let target_slugs: Vec<&String> = if slugs.len() == 1 {
        vec![&slugs[0]]
    } else {
        log::warn!(
            "Multiple slugs configured; cache markets are assigned to the first slug only"
        );
        vec![&slugs[0]]
    };

    for slug in target_slugs {
        let markets_for_slug = slug_markets.entry(slug.clone()).or_default();
        for (condition_id, market) in discovered {
            markets_for_slug.insert(condition_id.clone(), market.clone());
        }
    }
    new_count
}

fn outcome_price_split() -> Decimal {
    Decimal::new(55, 2)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum OutcomeSide {
    Yes,
    No,
    Unknown,
}

impl OutcomeSide {
    fn from_label(label: &str) -> Self {
        match label {
            "yes" | "up" => Self::Yes,
            "no" | "down" => Self::No,
            _ => Self::Unknown,
        }
    }

    #[must_use]
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Yes => "yes",
            Self::No => "no",
            Self::Unknown => "unknown",
        }
    }
}

pub(super) fn instrument_outcome_side(cache: &Cache, instrument_id: InstrumentId) -> OutcomeSide {
    let Some(InstrumentAny::BinaryOption(opt)) = cache.instrument(&instrument_id) else {
        return OutcomeSide::Unknown;
    };
    OutcomeSide::from_label(
        &opt.outcome
            .map(|v| v.to_string().to_lowercase())
            .unwrap_or_default(),
    )
}

pub(super) fn instrument_spot_market(cache: &Cache, instrument_id: InstrumentId) -> Option<String> {
    let instrument = cache.instrument(&instrument_id)?;
    Some(instrument.raw_symbol().to_string())
}

/// Resolve YES/NO ids using live instrument outcome labels (not slot names alone).
pub(super) fn resolve_yes_no_pair(
    cache: &Cache,
    pair: &SlugMarketIds,
) -> Option<(InstrumentId, InstrumentId)> {
    let yes_slot = instrument_outcome_side(cache, pair.yes_id);
    let no_slot = instrument_outcome_side(cache, pair.no_id);

    match (yes_slot, no_slot) {
        (OutcomeSide::Yes, OutcomeSide::No) => Some((pair.yes_id, pair.no_id)),
        (OutcomeSide::No, OutcomeSide::Yes) => {
            log::warn!(
                "YES/NO instrument ids appear swapped in cache for market_key={} \
                 yes_slot={} no_slot={}; correcting by outcome label",
                pair.condition_id,
                pair.yes_id,
                pair.no_id,
            );
            Some((pair.no_id, pair.yes_id))
        }
        _ => {
            log::warn!(
                "Could not confirm YES/NO outcomes for market_key={} \
                 yes_slot={} ({}) no_slot={} ({}); using cache slots as-is",
                pair.condition_id,
                pair.yes_id,
                yes_slot.as_str(),
                pair.no_id,
                no_slot.as_str(),
            );
            Some((pair.yes_id, pair.no_id))
        }
    }
}

pub(super) fn price_matches_outcome(outcome: OutcomeSide, price: Decimal) -> bool {
    let split = outcome_price_split();
    match outcome {
        OutcomeSide::Yes => price <= split,
        OutcomeSide::No => price >= split,
        OutcomeSide::Unknown => true,
    }
}

pub(super) fn warn_outcome_price_mismatch(
    context: &str,
    outcome: OutcomeSide,
    price: Decimal,
    instrument_id: InstrumentId,
    spot_market: Option<&str>,
) {
    if price_matches_outcome(outcome, price) {
        return;
    }
    let spot = spot_market.unwrap_or("-");
    log::warn!(
        "Outcome/price mismatch [{context}] outcome={} price={price} \
         instrument_id={instrument_id} spot_market={spot} \
         (expected YES ~16c / NO ~84c; check pm_to_lp mapping)",
        outcome.as_str(),
    );
}

pub(super) fn format_decimal_levels(
    levels: &indexmap::IndexMap<Decimal, Decimal>,
    limit: usize,
) -> String {
    levels
        .iter()
        .take(limit)
        .map(|(price, size)| format!("{size}@{price}"))
        .collect::<Vec<_>>()
        .join(", ")
}
