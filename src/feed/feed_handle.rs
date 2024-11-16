use super::feed_actor::{run_my_actor, FeedActor};
use super::messages::messages::FeedUpdate;
use super::FeedMessages;
use crate::common_types::tracked_sender::TrackedSender;
use barter_data_sniper::exchange::binance::spot::BinanceSpotTestnet;
use barter_data_sniper::streams::Streams;
use barter_data_sniper::subscription::book::{OrderBooksL1, OrderBooksL2};
use barter_instrument_copy::instrument::market_data::kind::MarketDataInstrumentKind;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::mpsc::{self};
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

#[derive(Clone)]
pub struct FeedHandle {
    sender: mpsc::Sender<FeedMessages>,
}

impl FeedHandle {
    pub async fn new(trading_pairs: HashSet<(String, String)>) -> (Self, JoinHandle<()>) {
        let (sender, receiver) = mpsc::channel(100);

        let l1_stream = Arc::new(Mutex::new(
            trading_pairs
                .iter()
                .fold(
                    Streams::<OrderBooksL1>::builder(),
                    |builder, (base, quote)| {
                        builder.subscribe([(
                            BinanceSpotTestnet::default(),
                            base.as_str(),
                            quote.as_str(),
                            MarketDataInstrumentKind::Spot,
                            OrderBooksL1,
                        )])
                    },
                )
                .init()
                .await
                .unwrap(),
        ));

        let l2_stream = Arc::new(Mutex::new(
            trading_pairs
                .iter()
                .fold(
                    Streams::<OrderBooksL2>::builder(),
                    |builder, (base, quote)| {
                        builder.subscribe([(
                            BinanceSpotTestnet::default(),
                            base.as_str(),
                            quote.as_str(),
                            MarketDataInstrumentKind::Spot,
                            OrderBooksL2,
                        )])
                    },
                )
                .init()
                .await
                .unwrap(),
        ));

        let actor = FeedActor::new(receiver, l1_stream, l2_stream);
        let handle = tokio::spawn(run_my_actor(actor));

        (Self { sender }, handle)
    }

    pub fn subscribe_to_l1<AlgoId, Symbol>(
        &self,
        algo_id: AlgoId,
        base: Symbol,
        quote: Symbol,
        subscriber: TrackedSender<FeedUpdate>,
    ) where
        Symbol: Into<String>,
        AlgoId: Into<String>,
    {
        let sending_result = self.sender.try_send(FeedMessages::SubscribeToL1 {
            algo_id: algo_id.into(),
            base: base.into(),
            quote: quote.into(),
            subscriber: subscriber,
        });

        if sending_result.is_err() {
            eprintln!("Failed to send message: {:?}", sending_result);
        }
    }

    pub fn unsubscribe_from_l1<Symbol, AlgoId>(
        &self,
        algo_id: AlgoId,
        base: Symbol,
        quote: Symbol,
        subscriber: &TrackedSender<FeedUpdate>,
    ) where
        AlgoId: Into<String>,
        Symbol: Into<String>,
    {
        let sending_result = self.sender.try_send(FeedMessages::UnsubscribeFromL1 {
            algo_id: algo_id.into(),
            base: base.into(),
            quote: quote.into(),
            subscriber: subscriber.clone(),
        });

        if sending_result.is_err() {
            eprintln!("Failed to send message: {:?}", sending_result);
        }
    }

    pub fn subscribe_to_l2<AlgoId, Symbol>(
        &self,
        algo_id: AlgoId,
        base: Symbol,
        quote: Symbol,
        subscriber: TrackedSender<FeedUpdate>,
    ) where
        Symbol: Into<String>,
        AlgoId: Into<String>,
    {
        let sending_result = self.sender.try_send(FeedMessages::SubscribeToL2 {
            algo_id: algo_id.into(),
            base: base.into(),
            quote: quote.into(),
            subscriber: subscriber,
        });

        if sending_result.is_err() {
            eprintln!("Failed to send message: {:?}", sending_result);
        }
    }

    pub fn unsubscribe_from_l2<Symbol, AlgoId>(
        &self,
        algo_id: AlgoId,
        base: Symbol,
        quote: Symbol,
        subscriber: &TrackedSender<FeedUpdate>,
    ) where
        AlgoId: Into<String>,
        Symbol: Into<String>,
    {
        let sending_result = self.sender.try_send(FeedMessages::UnsubscribeFromL2 {
            algo_id: algo_id.into(),
            base: base.into(),
            quote: quote.into(),
            subscriber: subscriber.clone(),
        });

        if sending_result.is_err() {
            eprintln!("Failed to send message: {:?}", sending_result);
        }
    }
}
