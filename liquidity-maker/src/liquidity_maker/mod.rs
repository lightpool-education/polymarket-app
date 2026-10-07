// Copyright (c) LightPool Labs
// Author: xiaoyu1998

pub mod bootstrap;
pub mod config;
pub mod markets;
pub mod strategy;
mod sync;

pub use bootstrap::{
    BootstrapConfig, MarketPair, bootstrap_markets_from_polymarket, fetch_hottest_event_slugs,
};
pub use config::LiquidityMakerConfig;
pub use markets::SlugMarketIds;
pub use strategy::LiquidityMaker;
