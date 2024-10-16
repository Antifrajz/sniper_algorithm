use std::{collections::HashMap, sync::Arc};

use tokio::{
    sync::{mpsc, Mutex},
    task,
};

use binance::{
    account::{self, Account},
    websockets::*,
};
use binance::{api::*, config::Config};
use binance::{model::OrderTradeEvent, userstream::*};
use std::sync::atomic::{AtomicBool, Ordering};
use uuid::Uuid;

pub enum MarketMessages {
    CreateOrder(f64, mpsc::Sender<MarketResponses>, String),
    ExecutionReport(OrderTradeEvent),
}

pub enum MarketResponses {
    CreateOrderAck(String),
    CreateOrderRejected(String),
}

#[derive(Clone)]

pub struct MarketSessionHandle {
    sender: mpsc::Sender<MarketMessages>,
}

impl MarketSessionHandle {
    pub async fn new() -> Self {
        let (sender, receiver) = mpsc::channel(100);

        let actor = MarketSession::new(receiver, sender.clone()).await;

        tokio::spawn(run_my_actor(actor, sender.clone()));

        Self { sender }
    }

    pub fn create_order(
        &self,
        price: f64,
        meesage_sender: mpsc::Sender<MarketResponses>,
        cl_order_id: String,
    ) {
        self.sender
            .try_send(MarketMessages::CreateOrder(
                price,
                meesage_sender,
                cl_order_id,
            ))
            .unwrap();
    }
}

struct MarketSession {
    receiver: mpsc::Receiver<MarketMessages>,
    handles: Vec<task::JoinHandle<()>>,
    account: Account,
    sender: mpsc::Sender<MarketMessages>,
    algo_contexts: HashMap<String, mpsc::Sender<MarketResponses>>,
}

impl MarketSession {
    pub async fn new(
        receiver: mpsc::Receiver<MarketMessages>,
        sender: mpsc::Sender<MarketMessages>,
    ) -> Self {
        // let api_key: Option<String> =
        //     Some("8Rz2XDFFItHESqPZ7CUl3Gk4JyQU73jBsqMAEhOKaizD4Fx28ryWNQuOg9OMweND".into());
        // let api_secret: Option<String> =
        //     Some("3Bo4CXMF95vj17hzREUxIzLP5wQfueU8YMplVREKr1pGQqCkBz088XOCw4N8mGV6".into());
        // let config = Config::testnet();
        // let user_stream: Account = Binance::new_with_config(api_key, api_secret, &config);

        let result = task::spawn_blocking(move || {
            // Here, place your blocking code that interacts with the API
            // Replace with actual logic for user_stream
            let api_key =
                Some("8Rz2XDFFItHESqPZ7CUl3Gk4JyQU73jBsqMAEhOKaizD4Fx28ryWNQuOg9OMweND".into());
            let api_secret =
                Some("3Bo4CXMF95vj17hzREUxIzLP5wQfueU8YMplVREKr1pGQqCkBz088XOCw4N8mGV6".into());

            // Simulate blocking API request here
            let config = Config::testnet();

            let user_stream: Account = Binance::new_with_config(api_key, api_secret, &config);

            user_stream // This should return the actual result
        })
        .await; // Await the result of the blocking tas

        let account = result.unwrap();
        // task::spawn_blocking(move || match account2.market_buy("BTCUSDT", 0.001) {
        //     Ok(answer) => println!("{:?}", answer),
        //     Err(e) => println!("Error: {}", e),
        // })
        // .await
        // .unwrap();

        Self {
            receiver,
            handles: Vec::new(),
            account: account,
            sender: sender,
            algo_contexts: HashMap::new(),
        }
    }

    pub async fn handle(&mut self, market_message: MarketMessages) {
        // println!("Handling feed update in AlgoContext");

        match market_message {
            MarketMessages::CreateOrder(price, sender, order_id) => {
                // match self.account.market_buy("BTCUSDT", 0.001) {
                //     Ok(answer) => println!("{:?}", answer),
                //     Err(e) => println!("Error: {}", e),
                // }
                let account2 = self.account.clone();
                let market_sender = self.sender.clone();
                self.algo_contexts.insert(order_id.clone(), sender);
                task::spawn_blocking(move || {
                    // match account2.market_buy("BTCUSDT", 0.0001) {
                    match account2.ioc_buy("BTCUSDT", 0.0001, price, order_id) {
                        Ok(answer) => println!("{:?}", answer),
                        Err(e) => {
                            println!("Error: {}", e);
                            // market_sender.send(MarketMessages::ExecutionReport(OrderTradeEvent {
                            //     event_type: String::from("Execution report"),
                            // })).;
                            //Ideja je poslat sam sebi Execution report da nismo uspjeli polsat order
                        }
                    }
                });
            }
            MarketMessages::ExecutionReport(trade) => {
                println!(
                    "Symbol: {}, Side: {}, Price: {}, Execution Type: {}",
                    trade.symbol, trade.side, trade.price, trade.execution_type
                );
                let algo = self.algo_contexts.get(&trade.new_client_order_id).unwrap();
                algo.send(MarketResponses::CreateOrderAck(String::from(
                    "Uspjeno poslan order",
                )))
                .await
                .unwrap();
            }
        }
    }
}

async fn run_my_actor(mut actor: MarketSession, sender: mpsc::Sender<MarketMessages>) {
    task::spawn_blocking(move || {
        let keep_running = AtomicBool::new(true); // Used to control the event loop
        let api_key =
            Some("8Rz2XDFFItHESqPZ7CUl3Gk4JyQU73jBsqMAEhOKaizD4Fx28ryWNQuOg9OMweND".into());
        let api_secret =
            Some("3Bo4CXMF95vj17hzREUxIzLP5wQfueU8YMplVREKr1pGQqCkBz088XOCw4N8mGV6".into());
        let config = Config::testnet();
        let user_stream: UserStream = Binance::new_with_config(api_key, api_secret, &config);

        if let Ok(answer) = user_stream.start() {
            let listen_key = answer.listen_key;

            println!("SIU market");

            let mut web_socket: WebSockets<'_> = WebSockets::new(|event: WebsocketEvent| {
                match event {
                    WebsocketEvent::OrderTrade(trade) => {
                        println!(
                            "Symbol: {}, Side: {}, Price: {}, Execution Type: {}, Client OrderId: {}",
                            trade.symbol, trade.side, trade.price, trade.execution_type, trade.new_client_order_id
                        );
                        // sender
                        //     .try_send(MarketMessages::ExecutionReport(trade))
                        //     .unwrap();

                        let value = sender.clone();
                        tokio::spawn(async move {
                            if let Err(e) = value.send(MarketMessages::ExecutionReport(trade)).await
                            {
                                eprintln!("Error sending message: {:?}", e);
                            }
                        });
                    }
                    _ => (),
                };

                Ok(())
            });

            web_socket
                .connect_with_config(&listen_key, &config)
                .unwrap(); // check error
            if let Err(e) = web_socket.event_loop(&keep_running) {
                println!("Error: {}", e);
            }
            user_stream.close(&listen_key).unwrap();
            web_socket.disconnect().unwrap();
            println!("Userstrem closed and disconnected");
        }
    });

    println!("SIU prije whiel petlje");

    while let Some(msg) = actor.receiver.recv().await {
        actor.handle(msg).await;
    }
}
#[derive(Clone)]
pub struct MarketService {
    market_session_handle: MarketSessionHandle,
    meesage_sender: mpsc::Sender<MarketResponses>,
}

impl MarketService {
    pub fn new(
        market_session_handle: MarketSessionHandle,
        meesage_sender: mpsc::Sender<MarketResponses>,
    ) -> Self {
        Self {
            market_session_handle,
            meesage_sender,
        }
    }

    pub fn create_order(&self, price: f64) {
        let random_id = Uuid::new_v4().to_string();

        self.market_session_handle
            .create_order(price, self.meesage_sender.clone(), random_id);
    }
}
