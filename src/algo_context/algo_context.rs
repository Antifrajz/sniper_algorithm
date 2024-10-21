use std::sync::Arc;

use feed::actor::{Feed, FeedMessages, FeedService};
use tokio::{
    sync::{mpsc, Mutex},
    task,
};

use crate::{
    feed::{self, actor::FeedData},
    market::market::{ExecutionType, MarketResponses, MarketService, MarketSessionHandle, Side},
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

    pub async fn handle_feed_update(&mut self, feed_update: FeedData) {
        self.algo.handle(feed_update).await;
    }

    pub fn handle_market_reponse(&mut self, market_response: MarketResponses) {
        println!("Handling market response in AlgoContext");

        self.algo.handle_market_reponse(market_response);
    }
}

async fn run_my_actor(mut actor: AlgoContext) {
    loop {
        tokio::select! {
            Some(_msg) = actor.receiver.recv() => {
                println!("U algoContextu siu");

            },
            Some(_msg) = actor.feed_receiver.recv() => {
                actor.handle_feed_update(_msg).await;


            },
            Some(_msg) = actor.market_receiver.recv() => {
                actor.handle_market_reponse(_msg);


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
    pub async fn handle(&mut self, feed_update: FeedData) {
        // println!("Handling L1 update, sending order to market");
        match feed_update {
            FeedData::L1Data(data) => {
                if !self.order_sent {
                    println!("MarketEvent<OrderBook>: {data:?}");
                }
                if data.kind.best_ask.price <= 69350.04f64 && !self.order_sent {
                    println!("Saljem order");
                    self.order_sent = true;
                    self.market_sevice.create_ioc_order(
                        "BTCUSDT",
                        69350.04_f64,
                        0.000164_f64,
                        Side::Buy,
                    );
                } else if !self.order_sent {
                    println!("Cijena je previsoka da reagujem");
                }
            }
            _ => println!("Siu"),
        }
    }

    fn handle_market_reponse(&mut self, event: MarketResponses) {
        match event {
            MarketResponses::CreateOrderAck {
                order_id,
                symbol,
                execution_status,
                order_quantity,
                price,
                side,
            } => self.handle_create_order_ack(
                order_id,
                symbol,
                execution_status,
                order_quantity,
                price,
                side,
            ),
            MarketResponses::OrderPartiallyFilled {
                order_id,
                symbol,
                execution_status,
                quantity,
                fill_price,
                side,
                executed_quantity,
                cumulative_quantity,
                leaves_quantity,
            } => self.handle_order_partially_filled(
                order_id,
                symbol,
                execution_status,
                quantity,
                fill_price,
                side,
                executed_quantity,
                cumulative_quantity,
                leaves_quantity,
            ),
            MarketResponses::OrderFullyFilled {
                order_id,
                symbol,
                execution_status,
                quantity,
                fill_price,
                side,
                executed_quantity,
                cumulative_quantity,
                leaves_quantity,
            } => self.handle_order_fully_filled(
                order_id,
                symbol,
                execution_status,
                quantity,
                fill_price,
                side,
                executed_quantity,
                cumulative_quantity,
                leaves_quantity,
            ),
            MarketResponses::OrderExpired {
                order_id,
                symbol,
                execution_status,
                quantity,
                side,
                executed_quantity,
                cumulative_quantity,
                leaves_quantity,
            } => self.handle_order_expired(
                order_id,
                symbol,
                execution_status,
                quantity,
                side,
                executed_quantity,
                cumulative_quantity,
                leaves_quantity,
            ),
            MarketResponses::OrderRejected {
                order_id,
                symbol,
                execution_status,
                order_quantity,
                price,
                side,
                rejection_reason,
            } => self.handle_order_rejected(
                order_id,
                symbol,
                execution_status,
                order_quantity,
                price,
                side,
                rejection_reason,
            ),
            MarketResponses::OrderCanceled {
                order_id,
                symbol,
                execution_status,
                quantity,
                side,
                executed_quantity,
                cumulative_quantity,
                leaves_quantity,
            } => self.handle_order_canceled(
                order_id,
                symbol,
                execution_status,
                quantity,
                side,
                executed_quantity,
                cumulative_quantity,
                leaves_quantity,
            ),
        }
    }

    fn handle_create_order_ack(
        &mut self,
        order_id: String,
        symbol: String,
        execution_status: ExecutionType,
        order_quantity: f64,
        price: f64,
        side: Side,
    ) {
        println!(
            "CreateOrderAck: order_id = {}, symbol = {}, execution_status = {:?}, order_quantity = {}, price = {}, side = {}",
            order_id, symbol, execution_status, order_quantity, price, side
        );
    }

    fn handle_order_partially_filled(
        &mut self,
        order_id: String,
        symbol: String,
        execution_status: ExecutionType,
        quantity: f64,
        fill_price: f64,
        side: Side,
        executed_quantity: f64,
        cumulative_quantity: f64,
        leaves_quantity: f64,
    ) {
        println!(
            "OrderPartiallyFilled: order_id = {}, symbol = {}, execution_status = {:?}, quantity = {}, fill_price = {}, side = {}, executed_quantity = {}, cumulative_quantity = {}, leaves_quantity = {}",
            order_id, symbol, execution_status, quantity, fill_price, side, executed_quantity, cumulative_quantity, leaves_quantity
        );
    }

    fn handle_order_fully_filled(
        &mut self,
        order_id: String,
        symbol: String,
        execution_status: ExecutionType,
        quantity: f64,
        fill_price: f64,
        side: Side,
        executed_quantity: f64,
        cumulative_quantity: f64,
        leaves_quantity: f64,
    ) {
        println!(
            "OrderFullyFilled: order_id = {}, symbol = {}, execution_status = {:?}, quantity = {}, fill_price = {}, side = {}, executed_quantity = {}, cumulative_quantity = {}, leaves_quantity = {}",
            order_id, symbol, execution_status, quantity, fill_price, side, executed_quantity, cumulative_quantity, leaves_quantity
        );
    }

    fn handle_order_expired(
        &mut self,
        order_id: String,
        symbol: String,
        execution_status: ExecutionType,
        quantity: f64,
        side: Side,
        executed_quantity: f64,
        cumulative_quantity: f64,
        leaves_quantity: f64,
    ) {
        println!(
            "OrderExpired: order_id = {}, symbol = {}, execution_status = {:?}, quantity = {}, side = {}, executed_quantity = {}, cumulative_quantity = {}, leaves_quantity = {}",
            order_id, symbol, execution_status, quantity, side, executed_quantity, cumulative_quantity, leaves_quantity
        );
    }

    fn handle_order_rejected(
        &mut self,
        order_id: String,
        symbol: String,
        execution_status: ExecutionType,
        order_quantity: f64,
        price: f64,
        side: Side,
        rejection_reason: String,
    ) {
        println!(
            "OrderRejected: order_id = {}, symbol = {}, execution_status = {:?}, order_quantity = {}, price = {}, side = {}, rejection_reason = {}",
            order_id, symbol, execution_status, order_quantity, price, side, rejection_reason
        );
    }

    fn handle_order_canceled(
        &mut self,
        order_id: String,
        symbol: String,
        execution_status: ExecutionType,
        quantity: f64,
        side: Side,
        executed_quantity: f64,
        cumulative_quantity: f64,
        leaves_quantity: f64,
    ) {
        println!(
            "OrderCanceled: order_id = {}, symbol = {}, execution_status = {:?}, quantity = {}, side = {}, executed_quantity = {}, cumulative_quantity = {}, leaves_quantity = {}",
            order_id, symbol, execution_status, quantity, side, executed_quantity, cumulative_quantity, leaves_quantity
        );
    }

    pub fn new(market_sevice: MarketService) -> Self {
        Self {
            market_sevice,
            order_sent: false,
        }
    }
}
