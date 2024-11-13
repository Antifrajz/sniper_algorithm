use std::process;
mod algo_context;
mod algorithams;
mod common_types;
mod config;
mod logging;
mod market;

use algo_context::algo_service::AlgoService;
use feed::actor::FeedHandle;
use logging::algo_logger::AlgoLogger;
use market::market::MarketSessionHandle;

use config::{AlgorithmConfig, MarketConfig};

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

    println!("Successfully started");

    let (feed_service, feed_handle) =
        FeedHandle::new(config::extract_trading_pairs(&config.algorithms)).await;

    let (market_service, market_handle) = MarketSessionHandle::new(market_config).await;

    let (algo_service, algo_handle) =
        AlgoService::new(feed_service.clone(), market_service.clone()).await;

    for params in &config.algorithms {
        algo_service.create_algo(params.clone());
    }

    let (feed_result, market_result, algo_result) =
        tokio::join!(feed_handle, market_handle, algo_handle);

    for (name, result) in [
        ("Feed handle", feed_result),
        ("Market handle", market_result),
        ("Algo handle", algo_result),
    ]
    .iter()
    {
        if let Err(e) = result {
            println!("{} finished: {:?}", name, e);
        }
    }
    println!("Algorithms done");
}
