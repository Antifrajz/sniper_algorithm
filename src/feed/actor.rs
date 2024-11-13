use ::futures::future::join_all;
use barter_data::event::MarketEvent;
use barter_data::exchange::binance::spot::{BinanceSpot, BinanceSpotTestnet};
use barter_data::exchange::ExchangeId;
use barter_data::instrument;
use barter_data::streams::builder::{self, StreamBuilder};
use barter_data::streams::Streams;
use barter_data::subscription::book::{OrderBook, OrderBookL1, OrderBooksL1, OrderBooksL2};
use barter_integration::model::instrument::kind::InstrumentKind;
use barter_integration::model::instrument::symbol::Symbol;
use barter_integration::model::instrument::{symbol, Instrument};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::{
    prelude::{FromPrimitive, Zero},
    Decimal,
};
use std::collections::{HashMap, HashSet};
use std::future::Future;

use std::hash::{Hash, Hasher};
use std::marker::Send;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{futures, Mutex};

use std::{fmt, string, thread};

pub struct Feed;

#[derive(Clone)]
pub struct TrackedSender<T> {
    pub sender: mpsc::Sender<T>,
    pub receiver_id: String, // Unique identifier for the receiver
}

impl<T> TrackedSender<T> {
    fn new(sender: mpsc::Sender<T>, receiver_id: String) -> Self {
        Self {
            sender,
            receiver_id,
        }
    }
}

impl<T> PartialEq for TrackedSender<T> {
    fn eq(&self, other: &Self) -> bool {
        self.receiver_id == other.receiver_id
    }
}

impl<T> Eq for TrackedSender<T> {}

// Implement Hash based on receiver_id to be able to store it in a HashMap
impl<T> Hash for TrackedSender<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.receiver_id.hash(state);
    }
}

pub struct FeedState {}

impl FeedState {
    pub fn new() -> Self {
        FeedState {}
    }
}

#[derive(Debug, Clone, Copy)]

pub struct Level {
    pub level: i32,
    pub quantity: Decimal,
    pub price: Decimal,
}

impl Level {
    pub fn new(level: i32, quantity: Decimal, price: Decimal) -> Self {
        Level {
            level: level,
            quantity: quantity,
            price: price,
        }
    }
}

impl fmt::Display for Level {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Level: {} {{quantity: {:.6}, price: {:.2} }}",
            self.level, self.quantity, self.price
        )
    }
}

#[derive(Debug, Clone)]
pub struct L1Data {
    pub symbol: String,
    pub best_bid_level: Level,
    pub best_ask_level: Level,
}

impl L1Data {
    pub fn new<Symbol>(
        symbol: Symbol,
        best_bid_quantity: Decimal,
        best_bid_price: Decimal,
        best_ask_quantity: Decimal,
        best_ask_price: Decimal,
    ) -> Self
    where
        Symbol: Into<String>,
    {
        L1Data {
            symbol: symbol.into(),
            best_bid_level: Level::new(1, best_bid_quantity, best_bid_price),
            best_ask_level: Level::new(1, best_ask_quantity, best_ask_price),
        }
    }
}

impl fmt::Display for L1Data {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "L1Data {{ symbol: {}, best_bid: {}, best_ask: {} }}",
            self.symbol, self.best_bid_level, self.best_ask_level
        )
    }
}

#[derive(Debug, Clone)]

pub struct L2Data {
    pub symbol: String,
    pub bid_side_levels: Vec<Level>,
    pub ask_side_levels: Vec<Level>,
}

impl L2Data {
    pub fn new<Symbol>(
        symbol: Symbol,
        bid_side_levels: Vec<Level>,
        ask_side_levels: Vec<Level>,
    ) -> Self
    where
        Symbol: Into<String>,
    {
        L2Data {
            symbol: symbol.into(),
            bid_side_levels,
            ask_side_levels,
        }
    }
}

impl fmt::Display for L2Data {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "L2Data {{ symbol: {}, bid_side_levels: [", self.symbol)?;
        for (i, level) in self.bid_side_levels.iter().enumerate() {
            if i > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{}", level)?;
        }
        write!(f, "], ask_side_levels: [")?;
        for (i, level) in self.ask_side_levels.iter().enumerate() {
            if i > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{}", level)?;
        }
        write!(f, "] }}")
    }
}

#[derive(Debug, Clone)]
pub enum FeedUpdate {
    L1Update(Vec<AlgoId>, L1Data),
    L2Update(Vec<AlgoId>, L2Data),
}

impl FeedUpdate {
    fn print(&self) {
        match self {
            Self::L1Update(_, _) => println!("Pingam se"),
            Self::L2Update(_, _) => println!("Pingam se"),
        }
    }
}

use tokio::sync::mpsc::{self, Sender, UnboundedReceiver};
use tokio::task::{self, JoinHandle};

// Struct to handle async tasks

type SenderId = String;
type AlgoId = String;
type InstrumentId = String;

struct FeedActor {
    receiver: mpsc::Receiver<FeedMessages>,
    handles: Vec<task::JoinHandle<()>>,
    l1_subscriber: Arc<Mutex<UnboundedReceiver<MarketEvent<Instrument, OrderBookL1>>>>,
    l2_subscriber: Arc<Mutex<UnboundedReceiver<MarketEvent<Instrument, OrderBook>>>>,
    l1_subscribers:
        Arc<Mutex<HashMap<InstrumentId, HashMap<TrackedSender<FeedUpdate>, Vec<AlgoId>>>>>,
    l2_subscribers:
        Arc<Mutex<HashMap<InstrumentId, HashMap<TrackedSender<FeedUpdate>, Vec<AlgoId>>>>>,
}

pub enum FeedMessages {
    // Ping(ActorRef<Message>),
    Subscribe(Instrument, mpsc::Sender<FeedUpdate>),
    SubscribeToL1 {
        algo_id: String,
        base: String,
        quote: String,
        subscriber: TrackedSender<FeedUpdate>,
    },
    UnsubscribeFromL1 {
        algo_id: String,
        base: String,
        quote: String,
        subscriber: TrackedSender<FeedUpdate>,
    },
    Unsubscribe,
}

impl FeedMessages {
    fn print(&self) {
        match self {
            Self::Subscribe(_, _) => println!("ping.."),
            Self::Unsubscribe => println!("pong.."),
            _ => (),
        }
    }
}

impl FeedActor {
    fn new(
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
            FeedMessages::Subscribe(instrument, sender) => {
                println!("Uniso");
                // let mut map = self.subscribers.lock().await;
                // map.entry(instrument)
                // .or_insert_with(Vec::new) // Create a new Vec if there isn't one for the instrument yet
                //         .push(sender); // Add the sender to the Vec
            }
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

            _ => {
                msg.print();
            }
        }
    }
}

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

        // Then, create L2 streams without moving trading_pairs
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

    pub fn subscribe<AlgoId, Symbol>(
        &self,
        algo_id: AlgoId,
        base: Symbol,
        quote: Symbol,
        sender: Sender<FeedUpdate>,
    ) where
        Symbol: Into<String>,
        AlgoId: Into<String>,
    {
        println!("Bio ovdje");

        let msg2 = FeedMessages::SubscribeToL1 {
            algo_id: algo_id.into(),
            base: base.into(),
            quote: quote.into(),
            subscriber: TrackedSender::new(sender, String::from("algoContext")),
        };
        let result = self.sender.try_send(msg2);
        match result {
            Ok(_) => {
                println!("Message sent successfully!");
            }
            Err(e) => {
                eprintln!("Failed to send message: {:?}", e);
            }
        }
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
}

async fn run_my_actor(mut actor: FeedActor) {
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

#[derive(Clone)]
pub struct FeedService {
    feed_handle: FeedHandle,
    algo_id: String,
    meesage_sender: TrackedSender<FeedUpdate>,
}

impl FeedService {
    pub fn new(
        feed_handle: FeedHandle,
        context_id: String,
        algo_id: String,
        sender: mpsc::Sender<FeedUpdate>,
    ) -> Self {
        Self {
            feed_handle,
            algo_id,
            meesage_sender: TrackedSender::new(sender, context_id),
        }
    }

    pub fn subscribe_to_l1<Symbol>(&self, base: Symbol, quote: Symbol)
    where
        Symbol: Into<String>,
    {
        self.feed_handle.subscribe_to_l1(
            self.algo_id.as_str(),
            base.into(),
            quote.into(),
            self.meesage_sender.clone(),
        );
    }

    pub fn unsubscribe_from_l1<Symbol>(&self, base: Symbol, quote: Symbol)
    where
        Symbol: Into<String>,
    {
        self.feed_handle.unsubscribe_from_l1(
            self.algo_id.as_str(),
            base.into(),
            quote.into(),
            &self.meesage_sender,
        );
    }
}
