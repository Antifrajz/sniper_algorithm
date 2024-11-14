use ::futures::future::join_all;
use barter_data::event::MarketEvent;
use barter_data::exchange::binance::spot::{BinanceSpot, BinanceSpotTestnet};
use barter_data::exchange::ExchangeId;
use barter_data::instrument;
use barter_data::streams::builder::{self, StreamBuilder};
use barter_data::streams::Streams;
use barter_data::subscription::book::{OrderBook, OrderBookL1, OrderBooksL1, OrderBooksL2};
use barter_integration::model::instrument::kind::InstrumentKind;
use barter_integration::model::instrument::{symbol, Instrument};
use rust_decimal::{
    prelude::{FromPrimitive, Zero},
    Decimal,
};
use std::collections::{HashMap, HashSet};
use std::future::Future;

use std::marker::Send;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{futures, Mutex};

use tokio::sync::mpsc::{self, Sender, UnboundedReceiver};
use tokio::task::{self, JoinHandle};

use crate::common_types::tracked_sender::TrackedSender;

use super::messages::l1_data::L1Data;
use super::messages::l2_data::L2Data;
use super::messages::level::Level;
use super::messages::messages::FeedUpdate;
use super::FeedMessages;

type AlgoId = String;
type InstrumentId = String;

pub(super) struct FeedActor {
    receiver: mpsc::Receiver<FeedMessages>,
    handles: Vec<task::JoinHandle<()>>,
    l1_subscriber: Arc<Mutex<UnboundedReceiver<MarketEvent<Instrument, OrderBookL1>>>>,
    l2_subscriber: Arc<Mutex<UnboundedReceiver<MarketEvent<Instrument, OrderBook>>>>,
    l1_subscribers:
        Arc<Mutex<HashMap<InstrumentId, HashMap<TrackedSender<FeedUpdate>, Vec<AlgoId>>>>>,
    l2_subscribers:
        Arc<Mutex<HashMap<InstrumentId, HashMap<TrackedSender<FeedUpdate>, Vec<AlgoId>>>>>,
}

impl FeedActor {
    pub fn new(
        receiver: mpsc::Receiver<FeedMessages>,
        l1_sub: UnboundedReceiver<MarketEvent<Instrument, OrderBookL1>>,
        l2_sub: UnboundedReceiver<MarketEvent<Instrument, OrderBook>>,
    ) -> Self {
        FeedActor {
            receiver,
            handles: Vec::new(),
            l1_subscriber: Arc::new(tokio::sync::Mutex::new(l1_sub)),
            l2_subscriber: Arc::new(tokio::sync::Mutex::new(l2_sub)),
            l1_subscribers: Arc::new(Mutex::new(HashMap::new())),
            l2_subscribers: Arc::new(Mutex::new(HashMap::new())),
        }
    }
    async fn handle_message(&mut self, msg: FeedMessages) {
        match msg {
            FeedMessages::SubscribeToL1 {
                algo_id,
                base,
                quote,
                subscriber,
            } => {
                let mut subscribers = self.l1_subscribers.lock().await;
                let instrument = base + quote.as_str();

                let senders_map = subscribers.entry(instrument).or_insert_with(HashMap::new);

                if let Some(algo_ids) = senders_map.get_mut(&subscriber) {
                    algo_ids.push(algo_id);
                } else {
                    senders_map.insert(subscriber, vec![algo_id]);
                }
            }

            FeedMessages::UnsubscribeFromL1 {
                algo_id,
                base,
                quote,
                subscriber,
            } => {
                let mut subscribers = self.l1_subscribers.lock().await;
                let instrument = base + quote.as_str();

                if let Some(senders_map) = subscribers.get_mut(&instrument) {
                    if let Some(algo_ids) = senders_map.get_mut(&subscriber) {
                        algo_ids.retain(|id| id != &algo_id);

                        if algo_ids.is_empty() {
                            senders_map.remove(&subscriber);
                        }

                        if senders_map.is_empty() {
                            subscribers.remove(&instrument);
                        }
                    }
                }
            }
            FeedMessages::SubscribeToL2 {
                algo_id,
                base,
                quote,
                subscriber,
            } => {
                let mut subscribers = self.l2_subscribers.lock().await;
                let instrument = base + quote.as_str();

                let senders_map = subscribers.entry(instrument).or_insert_with(HashMap::new);

                if let Some(algo_ids) = senders_map.get_mut(&subscriber) {
                    algo_ids.push(algo_id);
                } else {
                    senders_map.insert(subscriber, vec![algo_id]);
                }
            }

            FeedMessages::UnsubscribeFromL2 {
                algo_id,
                base,
                quote,
                subscriber,
            } => {
                let mut subscribers = self.l2_subscribers.lock().await;
                let instrument = base + quote.as_str();

                if let Some(senders_map) = subscribers.get_mut(&instrument) {
                    if let Some(algo_ids) = senders_map.get_mut(&subscriber) {
                        algo_ids.retain(|id| id != &algo_id);

                        if algo_ids.is_empty() {
                            senders_map.remove(&subscriber);
                        }

                        if senders_map.is_empty() {
                            subscribers.remove(&instrument);
                        }
                    }
                }
            }
        }
    }
}

pub(super) async fn run_my_actor(mut actor: FeedActor) {
    let binance_stream = actor.l1_subscriber.clone();
    let subscribers = actor.l1_subscribers.clone();
    actor.handles.push(tokio::spawn(async move {
        while let Some(msg) = binance_stream.lock().await.recv().await {
            let subscribers = subscribers.lock().await;

            let instrument =
                msg.instrument.base.as_ref().to_owned() + msg.instrument.quote.as_ref();

            if let Some(senders_map) = subscribers.get(&instrument) {
                let l1_data = L1Data::new(
                    instrument.clone(),
                    Decimal::from_f64(msg.kind.best_bid.amount).unwrap(),
                    Decimal::from_f64(msg.kind.best_bid.price).unwrap(),
                    Decimal::from_f64(msg.kind.best_ask.amount).unwrap(),
                    Decimal::from_f64(msg.kind.best_ask.price).unwrap(),
                );
                let send_futures: Vec<_> = senders_map
                    .iter()
                    .map(|(tracked_sender, algo_ids)| {
                        tracked_sender
                            .sender
                            .send(FeedUpdate::L1Update(algo_ids.clone(), l1_data.clone()))
                    })
                    .collect();

                let results = join_all(send_futures).await;
                for result in results {
                    if result.is_err() {
                        println!("Failed to send to one of the senders.");
                    }
                }
            } else {
                println!("No senders found for {:?}", instrument);
            }
        }
    }));

    let binance_stream = actor.l2_subscriber.clone();
    let l2_subscribers = actor.l2_subscribers.clone();
    actor.handles.push(tokio::spawn(async move {
        while let Some(msg) = binance_stream.lock().await.recv().await {
            let l2_subscribers = l2_subscribers.lock().await;

            let instrument =
                msg.instrument.base.as_ref().to_owned() + msg.instrument.quote.as_ref();

            let l2_data = L2Data::new(
                instrument.clone(),
                msg.kind
                    .bids
                    .levels
                    .iter()
                    .enumerate()
                    .map(|(i, l)| {
                        Level::new(
                            i as i32 + 1,
                            Decimal::from_f64(l.amount).unwrap(),
                            Decimal::from_f64(l.price).unwrap(),
                        )
                    })
                    .collect(),
                msg.kind
                    .asks
                    .levels
                    .iter()
                    .enumerate()
                    .map(|(i, l)| {
                        Level::new(
                            i as i32 + 1,
                            Decimal::from_f64(l.amount).unwrap(),
                            Decimal::from_f64(l.price).unwrap(),
                        )
                    })
                    .collect(),
            );

            if let Some(subscribers) = l2_subscribers.get(&instrument) {
                let send_futures: Vec<_> = subscribers
                    .iter()
                    .map(|(tracked_sender, algo_ids)| {
                        tracked_sender
                            .sender
                            .send(FeedUpdate::L2Update(algo_ids.clone(), l2_data.clone()))
                    })
                    .collect();

                let results = join_all(send_futures).await;
                for result in results {
                    if result.is_err() {
                        println!("Failed to send to one of the senders.");
                    }
                }
            } else {
                println!("No senders found for {:?}", instrument);
            }
        }
    }));

    while let Some(msg) = actor.receiver.recv().await {
        actor.handle_message(msg).await;
    }
}
