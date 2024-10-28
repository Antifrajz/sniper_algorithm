use std::{
    collections::HashMap,
    fmt::{self, Display},
    sync::Arc,
};

use tokio::{
    sync::{mpsc, Mutex},
    task,
};

use binance::{
    account::{self, Account, OrderSide, TimeInForce},
    websockets::*,
};
use binance::{api::*, config::Config};
use binance::{model::OrderTradeEvent, userstream::*};
use std::sync::atomic::{AtomicBool, Ordering};
use uuid::Uuid;

use crate::algo_context;

#[derive(Debug)]
pub enum ExecutionType {
    New,
    Canceled,
    Replaced,
    Rejected,
    Trade,
    Expired,
    TradePrevention,
    Unknown, // For any unmatched string
}

impl ExecutionType {
    fn from_str(input: &str) -> Self {
        match input {
            "NEW" => ExecutionType::New,
            "CANCELED" => ExecutionType::Canceled,
            "REPLACED" => ExecutionType::Replaced,
            "REJECTED" => ExecutionType::Rejected,
            "TRADE" => ExecutionType::Trade,
            "EXPIRED" => ExecutionType::Expired,
            "TRADE_PREVENTION" => ExecutionType::TradePrevention,
            _ => ExecutionType::Unknown, // Fallback case for unmatched strings
        }
    }
}

impl fmt::Display for ExecutionType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let variant_str = match self {
            ExecutionType::New => "NEW",
            ExecutionType::Canceled => "CANCELED",
            ExecutionType::Replaced => "REPLACED",
            ExecutionType::Rejected => "REJECTED",
            ExecutionType::Trade => "TRADE",
            ExecutionType::Expired => "EXPIRED",
            ExecutionType::TradePrevention => "TRADE_PREVENTION",
            ExecutionType::Unknown => "UNKNOWN",
        };
        write!(f, "{}", variant_str)
    }
}

pub enum Side {
    Buy,
    Sell,
}

impl Side {
    pub fn from_str(value: &str) -> Option<Self> {
        match value.to_uppercase().as_str() {
            "BUY" => Some(Side::Buy),
            "SELL" => Some(Side::Sell),
            _ => None,
        }
    }

    pub fn to_int(&self) -> i32 {
        match self {
            Side::Buy => 1,
            Side::Sell => 2,
        }
    }
}

impl Display for Side {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Buy => write!(f, "BUY"),
            Self::Sell => write!(f, "SELL"),
        }
    }
}

pub enum TIF {
    GTC,
    IOC,
    FOK,
}

impl TIF {
    pub fn to_int(&self) -> i32 {
        match self {
            TIF::GTC => 1,
            TIF::IOC => 2,
            TIF::FOK => 3,
        }
    }
}

impl Display for TIF {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::GTC => write!(f, "GTC"),
            Self::IOC => write!(f, "IOC"),
            Self::FOK => write!(f, "FOK"),
        }
    }
}

pub enum MarketMessages {
    CreateOrder(
        String,
        f64,
        f64,
        Side,
        TIF,
        mpsc::Sender<MarketResponses>,
        String,
        String,
    ),
}

pub enum MarketResponses {
    CreateOrderAck {
        order_id: String,
        symbol: String,
        execution_status: ExecutionType,
        order_quantity: f64,
        price: f64,
        side: Side,
    },
    OrderPartiallyFilled {
        order_id: String,
        symbol: String,
        execution_status: ExecutionType,
        quantity: f64,
        fill_price: f64,
        side: Side,
        executed_quantity: f64,
        cumulative_quantity: f64,
        leaves_quantity: f64,
    },
    OrderFullyFilled {
        order_id: String,
        symbol: String,
        execution_status: ExecutionType,
        quantity: f64,
        fill_price: f64,
        side: Side,
        executed_quantity: f64,
        cumulative_quantity: f64,
        leaves_quantity: f64,
    },
    OrderExpired {
        order_id: String,
        symbol: String,
        execution_status: ExecutionType,
        quantity: f64,
        side: Side,
        executed_quantity: f64,
        cumulative_quantity: f64,
        leaves_quantity: f64,
    },
    OrderRejected {
        order_id: String,
        symbol: String,
        execution_status: ExecutionType,
        order_quantity: f64,
        price: f64,
        side: Side,
        rejection_reason: String,
    },
    OrderCanceled {
        order_id: String,
        symbol: String,
        execution_status: ExecutionType,
        quantity: f64,
        side: Side,
        executed_quantity: f64,
        cumulative_quantity: f64,
        leaves_quantity: f64,
    },
}

fn handle_order_trade_event(algo: &mpsc::Sender<MarketResponses>, event: OrderTradeEvent) {
    let execution_type = ExecutionType::from_str(&event.execution_type);

    match execution_type {
        ExecutionType::New => {
            algo.try_send(MarketResponses::CreateOrderAck {
                order_id: event.new_client_order_id,
                symbol: event.symbol,
                execution_status: execution_type,
                order_quantity: event.qty.parse::<f64>().unwrap(),
                side: Side::Buy, //FIX This
                price: event.price.parse::<f64>().unwrap(),
            })
            .unwrap();
        }
        ExecutionType::Trade => {
            if event.order_status == String::from("FILLED") {
                algo.try_send(MarketResponses::OrderFullyFilled {
                    order_id: event.new_client_order_id,
                    symbol: event.symbol,
                    execution_status: execution_type,
                    quantity: event.qty.parse::<f64>().unwrap(),
                    fill_price: event.price_last_filled_trade.parse::<f64>().unwrap(),
                    side: Side::Buy, //FIX This
                    executed_quantity: event.qty_last_filled_trade.parse::<f64>().unwrap(),
                    cumulative_quantity: event
                        .accumulated_qty_filled_trades
                        .parse::<f64>()
                        .unwrap(),
                    leaves_quantity: event.qty.parse::<f64>().unwrap()
                        - event.qty_last_filled_trade.parse::<f64>().unwrap(),
                })
                .unwrap();
            } else {
                algo.try_send(MarketResponses::OrderPartiallyFilled {
                    order_id: event.new_client_order_id,
                    symbol: event.symbol,
                    execution_status: execution_type,
                    quantity: event.qty.parse::<f64>().unwrap(),
                    fill_price: event.price_last_filled_trade.parse::<f64>().unwrap(),
                    side: Side::Buy, //FIX This
                    executed_quantity: event.qty_last_filled_trade.parse::<f64>().unwrap(),
                    cumulative_quantity: event
                        .accumulated_qty_filled_trades
                        .parse::<f64>()
                        .unwrap(),
                    leaves_quantity: event.qty.parse::<f64>().unwrap()
                        - event.qty_last_filled_trade.parse::<f64>().unwrap(),
                })
                .unwrap();
            }
        }
        ExecutionType::Expired => {
            algo.try_send(MarketResponses::OrderExpired {
                order_id: event.new_client_order_id,
                symbol: event.symbol,
                execution_status: execution_type,
                quantity: event.qty.parse::<f64>().unwrap(),
                side: Side::Buy, //FIX This
                executed_quantity: event.qty_last_filled_trade.parse::<f64>().unwrap(),
                cumulative_quantity: event.accumulated_qty_filled_trades.parse::<f64>().unwrap(),
                leaves_quantity: event.qty.parse::<f64>().unwrap()
                    - event.qty_last_filled_trade.parse::<f64>().unwrap(),
            })
            .unwrap();
        }
        ExecutionType::Rejected => {
            algo.try_send(MarketResponses::OrderRejected {
                order_id: event.new_client_order_id,
                symbol: event.symbol,
                execution_status: execution_type,
                order_quantity: event.qty.parse::<f64>().unwrap(),
                side: Side::Buy, //FIX This
                price: event.price.parse::<f64>().unwrap(),
                rejection_reason: event.order_reject_reason,
            })
            .unwrap();
        }
        ExecutionType::Canceled => {
            algo.try_send(MarketResponses::OrderCanceled {
                order_id: event.new_client_order_id,
                symbol: event.symbol,
                execution_status: execution_type,
                quantity: event.qty.parse::<f64>().unwrap(),
                side: Side::Buy, //FIX This
                executed_quantity: event.qty_last_filled_trade.parse::<f64>().unwrap(),
                cumulative_quantity: event.accumulated_qty_filled_trades.parse::<f64>().unwrap(),
                leaves_quantity: event.qty.parse::<f64>().unwrap()
                    - event.qty_last_filled_trade.parse::<f64>().unwrap(),
            })
            .unwrap();
        }
        _ => (),
    }
}

#[derive(Clone)]

pub struct MarketSessionHandle {
    sender: mpsc::Sender<MarketMessages>,
}

impl MarketSessionHandle {
    pub async fn new() -> Self {
        let (sender, receiver) = mpsc::channel(100);

        let actor = MarketSession::new(receiver, sender.clone()).await;

        tokio::spawn(run_my_actor(actor));

        Self { sender }
    }

    pub fn create_order(
        &self,
        symbol: String,
        price: f64,
        quantity: f64,
        side: Side,
        time_inforce: TIF,
        meesage_sender: mpsc::Sender<MarketResponses>,
        cl_order_id: String,
        algo_id: String,
    ) {
        self.sender
            .try_send(MarketMessages::CreateOrder(
                symbol,
                price,
                quantity,
                side,
                time_inforce,
                meesage_sender,
                cl_order_id,
                algo_id,
            ))
            .unwrap();
    }
}

struct MarketSession {
    receiver: mpsc::Receiver<MarketMessages>,
    handles: Vec<task::JoinHandle<()>>,
    account: Account,
    sender: mpsc::Sender<MarketMessages>,
    algo_contexts: Arc<std::sync::Mutex<HashMap<String, (String, mpsc::Sender<MarketResponses>)>>>,
}

impl MarketSession {
    pub async fn new(
        receiver: mpsc::Receiver<MarketMessages>,
        sender: mpsc::Sender<MarketMessages>,
    ) -> Self {
        let result = task::spawn_blocking(move || {
            let api_key =
                Some("8Rz2XDFFItHESqPZ7CUl3Gk4JyQU73jBsqMAEhOKaizD4Fx28ryWNQuOg9OMweND".into());
            let api_secret =
                Some("3Bo4CXMF95vj17hzREUxIzLP5wQfueU8YMplVREKr1pGQqCkBz088XOCw4N8mGV6".into());

            let config = Config::testnet();
            let user_stream: Account = Binance::new_with_config(api_key, api_secret, &config);

            user_stream // This should return the actual result
        })
        .await; // Await the result of the blocking tas

        let account = result.unwrap();

        Self {
            receiver,
            handles: Vec::new(),
            account: account,
            sender: sender,
            algo_contexts: Arc::new(std::sync::Mutex::new(HashMap::new())),
        }
    }

    pub async fn handle(&mut self, market_message: MarketMessages) {
        match market_message {
            MarketMessages::CreateOrder(
                symbol,
                price,
                quantity,
                side,
                tif,
                sender,
                order_id,
                algo_id,
            ) => {
                let account2 = self.account.clone();
                let order_side = OrderSide::from_int(side.to_int()).unwrap();
                let time_in_force = TimeInForce::from_int(tif.to_int()).unwrap();
                let mut algo_contexts = self.algo_contexts.lock().unwrap(); // Lock the mutex (blocking)
                algo_contexts.insert(order_id.clone(), (algo_id, sender.clone()));
                task::spawn_blocking(move || {
                    match account2.create_order(
                        symbol.clone(),
                        quantity.clone(),
                        price.clone(),
                        order_side,
                        time_in_force,
                        order_id.clone(),
                    ) {
                        Ok(answer) => println!("{:?}", answer),
                        Err(e) => {
                            println!("Error: {}", e);
                            sender
                                .try_send(MarketResponses::OrderRejected {
                                    order_id,
                                    symbol: symbol,
                                    execution_status: ExecutionType::Rejected,
                                    order_quantity: quantity,
                                    side: Side::Buy, //FIX This
                                    price: price,
                                    rejection_reason: e.to_string(),
                                })
                                .unwrap();
                        }
                    }
                });
            }
        }
    }
}

async fn run_my_actor(mut actor: MarketSession) {
    let algo_contexts = actor.algo_contexts.clone();
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

            let mut web_socket: WebSockets<'_> = WebSockets::new(|event: WebsocketEvent| {
                match event {
                    WebsocketEvent::OrderTrade(trade) => {
                        println!(
                            "Symbol: {}, Side: {}, Price: {}, Execution Type: {}, Client OrderId: {}",
                            trade.symbol, trade.side, trade.price, trade.execution_type, trade.new_client_order_id
                        );

                        let algo_contexts = algo_contexts.lock().unwrap();
                        let (_, algo) = algo_contexts.get(&trade.new_client_order_id).unwrap();
                        handle_order_trade_event(algo, trade);
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

    while let Some(msg) = actor.receiver.recv().await {
        actor.handle(msg).await;
    }
}
#[derive(Clone)]
pub struct MarketService {
    market_session_handle: MarketSessionHandle,
    meesage_sender: mpsc::Sender<MarketResponses>,
    algo_id: String,
}

impl MarketService {
    pub fn new(
        market_session_handle: MarketSessionHandle,
        meesage_sender: mpsc::Sender<MarketResponses>,
        algo_id: String,
    ) -> Self {
        Self {
            market_session_handle,
            meesage_sender,
            algo_id,
        }
    }

    pub fn create_ioc_order<Symbol, Decimal>(
        &self,
        symbol: Symbol,
        price: Decimal,
        quantity: Decimal,
        side: Side,
    ) where
        Symbol: Into<String>,
        Decimal: Into<f64>,
    {
        self.create_order(symbol.into(), price, quantity, side, TIF::IOC);
    }

    pub fn create_order<Symbol, Decimal>(
        &self,
        symbol: Symbol,
        price: Decimal,
        quantity: Decimal,
        side: Side,
        time_inforce: TIF,
    ) where
        Symbol: Into<String>,
        Decimal: Into<f64>,
    {
        let random_id = Uuid::new_v4().to_string();

        self.market_session_handle.create_order(
            symbol.into(),
            price.into(),
            quantity.into(),
            side,
            time_inforce,
            self.meesage_sender.clone(),
            random_id,
            self.algo_id.clone(),
        );
    }
}
