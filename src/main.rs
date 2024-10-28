use core::task;
use std::{collections::HashSet, thread, time::Duration};

use algo_context::algo_context::AlgoService;
use feed::actor::{Feed, FeedHandle, FeedMessages};
use market::market::MarketSessionHandle;
use ractor::Actor;
mod algo_context;
mod market;

mod feed;
// MAIN //
#[tokio::main(flavor = "multi_thread", worker_threads = 4)]
async fn main() {
    let mut trading_pairs: HashSet<(&str, &str)> = HashSet::new();

    trading_pairs.insert(("btc", "usdt"));
    // trading_pairs.insert(("eth", "usdt"));
    // trading_pairs.insert(("sol", "usdt"));

    let mut feed_service = FeedHandle::new(trading_pairs).await;

    let mut market_service = MarketSessionHandle::new().await;

    let mut algo_service = AlgoService::new(feed_service.clone(), market_service.clone()).await;

    thread::sleep(Duration::from_secs(3000));

    // actor_handle.await.expect("Actor failed to exit cleanly");
}
