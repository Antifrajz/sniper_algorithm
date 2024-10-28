use std::sync::Arc;

use feed::actor::{Feed, FeedHandle, FeedMessages};
use tokio::{
    sync::{mpsc, Mutex},
    task,
};

use crate::{
    feed::{
        self,
        actor::{FeedService, FeedUpdate},
    },
    market::market::{ExecutionType, MarketResponses, MarketService, MarketSessionHandle, Side},
};

pub struct AlgoService {
    sender: mpsc::Sender<FeedMessages>,
}

impl AlgoService {
    pub async fn new(feed_handle: FeedHandle, market_session_handle: MarketSessionHandle) -> Self {
        let (sender, receiver) = mpsc::channel(10);

        let actor = AlgoContext::new(receiver, feed_handle, market_session_handle);

        tokio::spawn(run_my_actor(actor));

        Self { sender }
    }
}

struct AlgoContext {
    algo_context_id: String,
    receiver: mpsc::Receiver<FeedMessages>,
    feed_sender: mpsc::Sender<FeedUpdate>,
    feed_receiver: mpsc::Receiver<FeedUpdate>,
    market_sender: mpsc::Sender<MarketResponses>,
    market_receiver: mpsc::Receiver<MarketResponses>,
    handles: Vec<task::JoinHandle<()>>,
    algo: SniperAlgo,
    feed_hadnle: FeedHandle,
    market_session_handle: MarketSessionHandle,
}

impl AlgoContext {
    pub fn new(
        receiver: mpsc::Receiver<FeedMessages>,
        feed_hadnle: FeedHandle,
        market_session_handle: MarketSessionHandle,
    ) -> Self {
        let (feed_sender, feed_receiver) = mpsc::channel(10);
        let (market_sender, market_receiver) = mpsc::channel(10);

        let market_service = MarketService::new(
            market_session_handle.clone(),
            market_sender.clone(),
            String::from("algoId"),
        );

        let algo_context_id = String::from("contextId");
        let algo_id = String::from("algo_id");

        let feed_service = FeedService::new(
            feed_hadnle.clone(),
            algo_context_id.clone(),
            algo_id.clone(),
            feed_sender.clone(),
        );

        Self {
            algo_context_id,
            receiver,
            feed_sender,
            feed_receiver,
            market_sender,
            market_receiver,
            handles: Vec::new(),
            algo: SniperAlgo::new(market_service, feed_service),
            feed_hadnle,
            market_session_handle,
        }
    }

    pub async fn handle_feed_update(&mut self, feed_update: FeedUpdate) {
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
    feed_service: FeedService,
    order_sent: bool,
}

impl SniperAlgo {
    pub async fn handle(&mut self, feed_update: FeedUpdate) {
        // println!("Handling L1 update, sending order to market");
        match feed_update {
            FeedUpdate::L1Update(algo_ids, data) => {
                if !self.order_sent {
                    println!("{}", data);
                }
                // if data.best_ask_level.price <= 66600.04f64 && !self.order_sent {
                //     println!("Saljem order");
                //     self.order_sent = true;
                //     self.market_sevice.create_ioc_order(
                //         "BTCUSDT",
                //         66600.04_f64,
                //         0.0001_f64,
                //         Side::Buy,
                //     );
                // } else if !self.order_sent {
                //     println!("Cijena je previsoka da reagujem");
                // }
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

    pub fn new(market_sevice: MarketService, feed_service: FeedService) -> Self {
        feed_service.subscribe_to_l1("btc", "usdt");

        Self {
            market_sevice,
            feed_service,
            order_sent: false,
        }
    }
}
