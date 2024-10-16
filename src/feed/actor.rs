// Copyright (c) Sean Lawlor
//
// This source code is licensed under both the MIT license found in the
// LICENSE-MIT file in the root directory of this source tree.

//! A ping-pong actor implementation

use barter_data::event::MarketEvent;
use barter_data::exchange::binance::spot::{BinanceSpot, BinanceSpotTestnet};
use barter_data::exchange::ExchangeId;
use barter_data::instrument;
use barter_data::streams::builder::{self, StreamBuilder};
use barter_data::streams::Streams;
use barter_data::subscription::book::{OrderBook, OrderBookL1, OrderBooksL1, OrderBooksL2};
use barter_integration::model::instrument::kind::InstrumentKind;
use barter_integration::model::instrument::{symbol, Instrument};
use ractor::concurrency::JoinHandle;
use ractor::{actor, Actor, ActorProcessingErr, ActorRef};
use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::marker::Send;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

use std::{string, thread};

pub struct Feed;

fn assert_send<T: Send>(_: &T) {}

// #[derive( Clone, Eq, PartialEq, Default)]

pub struct FeedState {}

impl FeedState {
    pub fn new() -> Self {
        FeedState {}
    }
}

unsafe impl Send for FeedState {}

#[derive(Debug, Clone)]
pub enum FeedData {
    L1Data(MarketEvent<Instrument, OrderBookL1>),
    L2Data(MarketEvent<Instrument, OrderBook>),
    Trade,
}

impl FeedData {
    fn print(&self) {
        match self {
            Self::L1Data(_) => println!("Pingam se"),
            Self::L2Data(_) => println!("Pingam se"),
            Self::Trade => println!("Pingam se"),
        }
    }
}

use tokio::sync::mpsc::{self, Sender, UnboundedReceiver};
use tokio::task;

// Struct to handle async tasks

use tokio::sync::oneshot;

struct FeedActor {
    receiver: mpsc::Receiver<FeedMessages>,
    handles: Vec<task::JoinHandle<()>>,
    l1_subscriber: Arc<Mutex<UnboundedReceiver<MarketEvent<Instrument, OrderBookL1>>>>,
    l2_subscriber: Arc<Mutex<UnboundedReceiver<MarketEvent<Instrument, OrderBook>>>>,
    subscribers: Arc<Mutex<HashMap<Instrument, Vec<mpsc::Sender<FeedData>>>>>,
}

pub enum FeedMessages {
    // Ping(ActorRef<Message>),
    Subscribe(Instrument, mpsc::Sender<FeedData>),
    Unsubscribe,
}

impl FeedMessages {
    fn print(&self) {
        match self {
            Self::Subscribe(_, _) => println!("ping.."),
            Self::Unsubscribe => println!("pong.."),
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
            subscribers: Arc::new(Mutex::new(HashMap::new())),
        }
    }
    async fn handle_message(&mut self, msg: FeedMessages) {
        match msg {
            FeedMessages::Subscribe(instrument, sender) => {
                println!("Uniso");
                let mut map = self.subscribers.lock().await;
                map.entry(instrument)
                    .or_insert_with(Vec::new) // Create a new Vec if there isn't one for the instrument yet
                    .push(sender); // Add the sender to the Vec
            }
            _ => {
                msg.print();
            }
        }
    }
}

#[derive(Clone)]
pub struct FeedService {
    sender: mpsc::Sender<FeedMessages>,
}

impl FeedService {
    pub async fn new(trading_pairs: HashSet<(&str, &str)>) -> Self {
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

        tokio::spawn(run_my_actor(actor));

        Self { sender }
    }

    pub async fn subscribe(&mut self, base: &str, quote: &str, sender: Sender<FeedData>) {
        println!("Bio ovdje");

        let msg =
            FeedMessages::Subscribe(Instrument::new(base, quote, InstrumentKind::Spot), sender);
        let result = self.sender.send(msg).await;
        match result {
            Ok(_) => {
                println!("Message sent successfully!");
            }
            Err(e) => {
                eprintln!("Failed to send message: {:?}", e);
            }
        }
    }
}

async fn run_my_actor(mut actor: FeedActor) {
    let binance_stream = actor.l1_subscriber.clone();
    let subscribers = actor.subscribers.clone();
    actor.handles.push(tokio::spawn(async move {
        while let Some(msg) = binance_stream.lock().await.recv().await {
            let map = subscribers.lock().await;

            let cloned_message = msg.clone();
            let instrument = msg.instrument;
            if let Some(senders) = map.get(&instrument) {
                // println!("Found senders for {:?}", instrument);
                for sender in senders {
                    match sender.send(FeedData::L1Data(cloned_message.clone())).await {
                        Ok(_) => (),
                        Err(_) => (),
                    }
                }
            } else {
                println!("No senders found for {:?}", instrument);
            }
        }
    }));

    let binance_stream = actor.l2_subscriber.clone();
    let subscribers = actor.subscribers.clone();
    actor.handles.push(tokio::spawn(async move {
        while let Some(msg) = binance_stream.lock().await.recv().await {
            let map = subscribers.lock().await;

            let cloned_message = msg.clone();
            let instrument = msg.instrument;
            for instrument in map.keys() {
                // Print the instrument (requires Debug to be implemented for Instrument)
                println!("{:?}", instrument);
            }
            if let Some(senders) = map.get(&instrument) {
                println!("Found senders for {:?}", instrument);
                for sender in senders {
                    match sender.send(FeedData::L2Data(cloned_message.clone())).await {
                        Ok(_) => println!("Okej"),
                        Err(_) => println!("Rec dro"),
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
