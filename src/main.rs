use core::task;
use std::{collections::HashSet, process, thread, time::Duration};
mod logging;

use algo_context::algo_context::AlgoService;
use feed::actor::{Feed, FeedHandle, FeedMessages};
use logging::algo_logger::AlgoLogger;
use market::market::MarketSessionHandle;
mod algo_context;
mod config;
mod market;
use config::{AlgorithmConfig, MarketConfig}; // Use the Config struct

mod feed;
#[tokio::main(flavor = "multi_thread", worker_threads = 4)]
async fn main() {
    let config = match AlgorithmConfig::load_from_file("config/config.toml") {
        Ok(config) => {
            println!("{:#?}", config);
            config
        }
        Err(e) => {
            eprintln!("Failed to load config: {}", e);
            process::exit(1);
        }
    };

    let market_config = match MarketConfig::from_env() {
        Ok(config) => config,
        Err(e) => {
            eprintln!("Failed to load market config: {}", e);
            process::exit(1);
        }
    };

    AlgoLogger::init_once().expect("Failed to initialize logger");

    let feed_service = FeedHandle::new(config::extract_trading_pairs(&config.algorithms)).await;

    let market_service = MarketSessionHandle::new(market_config).await;

    let algo_service = AlgoService::new(feed_service.clone(), market_service.clone()).await;

    for params in &config.algorithms {
        algo_service.create_algo(params.clone());
    }

    thread::sleep(Duration::from_secs(3000));
}
