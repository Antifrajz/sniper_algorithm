use std::sync::Arc;

use feed::actor::{Feed, FeedMessages, FeedService};
use tokio::{
    sync::{mpsc, Mutex},
    task,
};

use crate::{
    feed::{self, actor::FeedData},
    market::market::{MarketResponses, MarketService, MarketSessionHandle},
};

pub struct AlgoService {
    feed_service: FeedService,
    sender: mpsc::Sender<FeedMessages>,
    feed_sender: mpsc::Sender<FeedData>,
    market_sender: mpsc::Sender<MarketResponses>,
}

impl AlgoService {
    pub async fn new(
        mut feed_service: FeedService,
        mut market_session_handle: MarketSessionHandle,
    ) -> Self {
        let (sender, receiver) = mpsc::channel(10);
        let (feed_sender, feed_receiver) = mpsc::channel(10);
        let (market_sender, market_receiver) = mpsc::channel(10);

        let market_service = MarketService::new(market_session_handle, market_sender.clone());

        let actor = AlgoContext::new(receiver, feed_receiver, market_receiver, market_service);

        feed_service
            .subscribe("btc", "usdt", feed_sender.clone())
            .await;

        tokio::spawn(run_my_actor(actor));

        Self {
            feed_service,
            sender,
            feed_sender,
            market_sender,
        }
    }
}

struct AlgoContext {
    receiver: mpsc::Receiver<FeedMessages>,
    feed_receiver: mpsc::Receiver<FeedData>,
    market_receiver: mpsc::Receiver<MarketResponses>,
    handles: Vec<task::JoinHandle<()>>,
    algo: SniperAlgo,
    market_sevice: MarketService,
}

impl AlgoContext {
    pub fn new(
        receiver: mpsc::Receiver<FeedMessages>,
        feed_receiver: mpsc::Receiver<FeedData>,
        market_receiver: mpsc::Receiver<MarketResponses>,
        market_sevice: MarketService,
    ) -> Self {
        Self {
            receiver,
            feed_receiver,
            market_receiver,
            handles: Vec::new(),
            algo: SniperAlgo::new(market_sevice.clone()),
            market_sevice,
        }
    }

    pub async fn handle(&mut self, feed_update: FeedData) {
        // println!("Handling feed update in AlgoContext");
        self.algo.handle(feed_update).await;
    }

    pub async fn handle2(&mut self, market_response: MarketResponses) {
        println!("Handling feed update in AlgoContext");
        // self.algo.handle(feed_update).await;
        match market_response {
            MarketResponses::CreateOrderAck(message) => self.algo.create_order_ack(message).await,
            _ => (),
        }
    }
}

async fn run_my_actor(mut actor: AlgoContext) {
    loop {
        tokio::select! {
            Some(_msg) = actor.receiver.recv() => {
                // actor.handle_message(msg).await;
                println!("U algoContextu siu");

            },
            Some(_msg) = actor.feed_receiver.recv() => {
                // println!("U algoContextu feed siu");
                actor.handle(_msg).await;


            },
            Some(_msg) = actor.market_receiver.recv() => {
                // println!("U algoContextu feed siu");
                actor.handle2(_msg).await;


            },
            else => break,
        }
    }
}
struct SniperAlgo {
    market_sevice: MarketService,
    order_sent: bool,
}

impl SniperAlgo {
    // pub async fn new(feed_service: FeedService) -> Self {
    //     let (sender, receiver) = mpsc::channel(100);

    //     let actor = FeedActor::new(receiver, binance_l1_stream, binance_l2_stream);

    //     tokio::spawn(run_my_actor(actor));

    //     Self { sender }
    // }

    pub async fn handle(&mut self, feed_update: FeedData) {
        // println!("Handling L1 update, sending order to market");
        match feed_update {
            FeedData::L1Data(data) => {
                if !self.order_sent {
                    println!("MarketEvent<OrderBook>: {data:?}");
                }
                if data.kind.best_bid.price <= 66000.04f64 && !self.order_sent {
                    println!("Saljem order");
                    self.order_sent = true;
                    self.market_sevice.create_order(66000.04f64);
                } else if !self.order_sent {
                    println!("Cijena je previsoka da reagujem");
                }
            }
            _ => println!("Siu"),
        }
    }

    pub async fn create_order_ack(&mut self, siu: String) {
        println!("Create Order ack from sniper");
    }

    pub fn new(market_sevice: MarketService) -> Self {
        Self {
            market_sevice,
            order_sent: false,
        }
    }
}
