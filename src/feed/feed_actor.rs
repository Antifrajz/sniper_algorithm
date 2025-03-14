use super::messages::l1_data::L1Data;
use super::messages::l2_data::L2Data;
use super::messages::level::Level;
use super::messages::messages::FeedUpdate;
use super::FeedMessages;
use crate::common_types::tracked_sender::TrackedSender;
use ::futures::future::join_all;
use barter_data_sniper::error::DataError;
use barter_data_sniper::event::MarketEvent;
use barter_data_sniper::streams::reconnect::stream::ReconnectingStream;
use barter_data_sniper::streams::reconnect::Event;
use barter_data_sniper::streams::Streams;
use barter_data_sniper::subscription::book::{OrderBookEvent, OrderBookL1};
use barter_instrument_copy::exchange::ExchangeId;
use barter_instrument_copy::instrument::market_data::MarketDataInstrument;
use probe::probe_lazy;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc::{self};
use tokio::sync::Mutex;
use tokio::task::{self};
use tokio_stream::StreamExt;

type AlgoId = String;
type InstrumentId = String;
pub(super) type L1Stream = Arc<
    Mutex<
        Streams<
            Event<ExchangeId, Result<MarketEvent<MarketDataInstrument, OrderBookL1>, DataError>>,
        >,
    >,
>;
pub(super) type L2Stream = Arc<
    Mutex<
        Streams<
            Event<ExchangeId, Result<MarketEvent<MarketDataInstrument, OrderBookEvent>, DataError>>,
        >,
    >,
>;

macro_rules! probe {
    ($name:ident) => {
        probe_lazy!(l1_updates, $name, { std::ptr::null::<()>() })
    };
}

pub(super) struct FeedActor {
    receiver: mpsc::Receiver<FeedMessages>,
    handles: Vec<task::JoinHandle<()>>,
    l1_stream: L1Stream,
    l2_stream: L2Stream,
    l1_subscribers:
        Arc<Mutex<HashMap<InstrumentId, HashMap<TrackedSender<FeedUpdate>, Vec<AlgoId>>>>>,
    l2_subscribers:
        Arc<Mutex<HashMap<InstrumentId, HashMap<TrackedSender<FeedUpdate>, Vec<AlgoId>>>>>,
}

impl FeedActor {
    pub fn new(
        receiver: mpsc::Receiver<FeedMessages>,
        l1_stream: L1Stream,
        l2_stream: L2Stream,
    ) -> Self {
        FeedActor {
            receiver,
            handles: Vec::new(),
            l1_stream,
            l2_stream,
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
    let subscribers = actor.l1_subscribers.clone();
    let l1_stream = actor.l1_stream.clone();
    actor.handles.push(tokio::spawn(async move {
        let mut l1_stream = l1_stream.lock().await;

        let mut binance_l1_stream = l1_stream
            .select(ExchangeId::BinanceSpot)
            .unwrap()
            .with_error_handler(|error| eprintln!("MarketStream generated error {}", error));

        while let Some(msg) = binance_l1_stream.next().await {
            let subscribers = subscribers.lock().await;

            match msg {
                barter_data_sniper::streams::reconnect::Event::Item(l1_update) => {
                    let instrument = l1_update.instrument.base.as_ref().to_owned()
                        + l1_update.instrument.quote.as_ref();

                    if let Some(senders_map) = subscribers.get(&instrument) {
                        let l1_data = L1Data::new(
                            instrument.clone(),
                            l1_update.kind.best_bid.amount,
                            l1_update.kind.best_bid.price,
                            l1_update.kind.best_ask.amount,
                            l1_update.kind.best_ask.price,
                        );

                        println!("{:#?}", l1_data);

                        probe!(feed_update_received);

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
                                eprintln!("Failed to send to one of the senders.");
                            }
                        }
                    }
                }
                barter_data_sniper::streams::reconnect::Event::Reconnecting(origin) => {
                    eprintln!("Reconnecting to L1 updates{}.", origin);
                }
            }
        }
    }));

    let l2_stream = actor.l2_stream.clone();
    let l2_subscribers = actor.l2_subscribers.clone();
    actor.handles.push(tokio::spawn(async move {
        let mut l2_stream = l2_stream.lock().await;

        let mut binance_l2_stream = l2_stream
            .select(ExchangeId::BinanceSpot)
            .unwrap()
            .with_error_handler(|error| eprintln!("MarketStream generated error {}", error));

        while let Some(msg) = binance_l2_stream.next().await {
            let l2_subscribers = l2_subscribers.lock().await;

            match msg {
                Event::Item(l2_update) => {
                    let instrument = l2_update.instrument.base.as_ref().to_owned()
                        + l2_update.instrument.quote.as_ref();

                    match l2_update.kind {
                        OrderBookEvent::Snapshot(order_book)
                        | OrderBookEvent::Update(order_book) => {
                            order_book.asks().levels();

                            let l2_data = L2Data::new(
                                instrument.clone(),
                                order_book
                                    .asks()
                                    .levels()
                                    .iter()
                                    .enumerate()
                                    .map(|(i, l)| Level::new(i as i32 + 1, l.amount, l.price))
                                    .collect(),
                                order_book
                                    .asks()
                                    .levels()
                                    .iter()
                                    .enumerate()
                                    .map(|(i, l)| Level::new(i as i32 + 1, l.amount, l.price))
                                    .collect(),
                            );

                            if let Some(subscribers) = l2_subscribers.get(&instrument) {
                                let send_futures: Vec<_> = subscribers
                                    .iter()
                                    .map(|(tracked_sender, algo_ids)| {
                                        tracked_sender.sender.send(FeedUpdate::L2Update(
                                            algo_ids.clone(),
                                            l2_data.clone(),
                                        ))
                                    })
                                    .collect();

                                let results = join_all(send_futures).await;
                                for result in results {
                                    if result.is_err() {
                                        eprintln!("Failed to send to one of the senders.");
                                    }
                                }
                            }
                        }
                    }
                }

                barter_data_sniper::streams::reconnect::Event::Reconnecting(origin) => {
                    eprintln!("Reconnecting to L2 updates {}.", origin);
                }
            }
        }
    }));

    while let Some(msg) = actor.receiver.recv().await {
        actor.handle_message(msg).await;
    }
}
