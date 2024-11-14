use super::feed_actor::{run_my_actor, FeedActor};
use super::messages::messages::FeedUpdate;
use super::FeedMessages;
use crate::common_types::tracked_sender::TrackedSender;
use barter_data::event::MarketEvent;
use barter_data::exchange::binance::spot::BinanceSpotTestnet;
use barter_data::exchange::ExchangeId;
use barter_data::streams::Streams;
use barter_data::subscription::book::{OrderBook, OrderBookL1, OrderBooksL1, OrderBooksL2};
use barter_integration::model::instrument::kind::InstrumentKind;
use barter_integration::model::instrument::Instrument;
use std::collections::HashSet;
use tokio::sync::mpsc::{self, UnboundedReceiver};
use tokio::task::JoinHandle;

#[derive(Clone)]
pub struct FeedHandle {
    sender: mpsc::Sender<FeedMessages>,
}

impl FeedHandle {
    pub async fn new(trading_pairs: HashSet<(&str, &str)>) -> (Self, JoinHandle<()>) {
        let (sender, receiver) = mpsc::channel(100);

        let mut streams_l1 = trading_pairs
            .iter()
            .fold(
                Streams::<OrderBooksL1>::builder(),
                |builder, &(base, quote)| {
                    builder.subscribe([(
                        BinanceSpotTestnet::default(),
                        base,
                        quote,
                        InstrumentKind::Spot,
                        OrderBooksL1,
                    )])
                },
            )
            .init()
            .await
            .unwrap();

        let binance_l1_stream: UnboundedReceiver<MarketEvent<Instrument, OrderBookL1>> =
            streams_l1.select(ExchangeId::BinanceSpot).unwrap();

        let mut streams_l2 = trading_pairs
            .iter()
            .fold(
                Streams::<OrderBooksL2>::builder(),
                |builder, &(base, quote)| {
                    builder.subscribe([(
                        BinanceSpotTestnet::default(),
                        base,
                        quote,
                        InstrumentKind::Spot,
                        OrderBooksL2,
                    )])
                },
            )
            .init()
            .await
            .unwrap();

        let binance_l2_stream: UnboundedReceiver<MarketEvent<Instrument, OrderBook>> =
            streams_l2.select(ExchangeId::BinanceSpot).unwrap();

        let actor = FeedActor::new(receiver, binance_l1_stream, binance_l2_stream);

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
