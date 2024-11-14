use std::{
    collections::HashMap,
    fmt::{self, Display},
    sync::Arc,
};

use barter_integration::model::instrument::symbol;
use serde::Deserialize;
use tokio::{
    sync::{mpsc, Mutex},
    task::{self, JoinHandle},
};

use rust_decimal::{
    prelude::{FromPrimitive, Zero},
    Decimal,
};

use binance::{
    account::{self, Account, OrderSide, OrderType as BinanceOrderType, TimeInForce},
    futures::general,
    general::General,
    model::Filters,
    websockets::*,
};
use binance::{api::*, config::Config};
use binance::{model::OrderTradeEvent, userstream::*};
use rust_decimal::prelude::ToPrimitive;
use std::sync::atomic::{AtomicBool, Ordering};
use uuid::Uuid;

use crate::{algo_context, config::MarketConfig};

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

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "lowercase")]
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

use std::str::FromStr;

#[derive(Debug, PartialEq)]
pub enum TIF {
    GTC, // Good-Till-Cancel
    IOC, // Immediate-Or-Cancel
    FOK, // Fill-Or-Kill
    GTX, // Good-Till-Crossing (Post Only)
    GTD, // Good-Till-Date
}

impl TIF {
    pub fn to_int(&self) -> i32 {
        match self {
            TIF::GTC => 1,
            TIF::IOC => 2,
            TIF::FOK => 3,
            TIF::GTX => 4,
            TIF::GTD => 5,
        }
    }

    pub fn to_str(&self) -> &'static str {
        match self {
            TIF::GTC => "GTC",
            TIF::IOC => "IOC",
            TIF::FOK => "FOK",
            TIF::GTX => "GTX",
            TIF::GTD => "GTD",
        }
    }
}

impl fmt::Display for TIF {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_str())
    }
}

impl FromStr for TIF {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "GTC" => Ok(TIF::GTC),
            "IOC" => Ok(TIF::IOC),
            "FOK" => Ok(TIF::FOK),
            "GTX" => Ok(TIF::GTX),
            "GTD" => Ok(TIF::GTD),
            _ => Err(format!("'{}' is not a valid TIF value", s)),
        }
    }
}

#[derive(Debug)]
pub enum OrderType {
    Limit,
    Market,
    StopLossLimit,
}

impl OrderType {
    pub fn from_int(value: i32) -> Option<Self> {
        match value {
            1 => Some(OrderType::Limit),
            2 => Some(OrderType::Market),
            3 => Some(OrderType::StopLossLimit),
            _ => None,
        }
    }

    pub fn to_int(&self) -> i32 {
        match self {
            OrderType::Limit => 1,
            OrderType::Market => 2,
            OrderType::StopLossLimit => 3,
        }
    }
}

impl Display for OrderType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Limit => write!(f, "LIMIT"),
            Self::Market => write!(f, "MARKET"),
            Self::StopLossLimit => write!(f, "STOP_LOSS_LIMIT"),
        }
    }
}

impl FromStr for OrderType {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "LIMIT" => Ok(OrderType::Limit),
            "MARKET" => Ok(OrderType::Market),
            "STOP_LOSS_LIMIT" => Ok(OrderType::StopLossLimit),
            _ => Err(()),
        }
    }
}

pub enum MarketMessages {
    GetSymbolInformation {
        symbol: String,
        algo_id: String,
        sender: mpsc::Sender<MarketResponses>,
    },
    CreateOrder {
        symbol: String,
        price: Decimal,
        quantity: Decimal,
        side: Side,
        order_type: OrderType,
        time_in_force: TIF,
        sender: mpsc::Sender<MarketResponses>,
        order_id: String,
        algo_id: String,
    },
}

pub enum MarketResponses {
    SymbolInformation {
        algo_id: String,
        min_quantity: Option<Decimal>,
        max_quantity: Option<Decimal>,
        lot_size: Option<Decimal>,
        min_price: Option<Decimal>,
        max_price: Option<Decimal>,
        tick_size: Option<Decimal>,
        min_amount: Option<Decimal>,
    },
    CreateOrderAck {
        order_id: String,
        algo_id: String,
        symbol: String,
        execution_status: ExecutionType,
        order_quantity: Decimal,
        price: Decimal,
        side: Side,
        order_type: OrderType,
        time_in_force: TIF,
    },
    OrderPartiallyFilled {
        order_id: String,
        algo_id: String,
        symbol: String,
        execution_status: ExecutionType,
        quantity: Decimal,
        fill_price: Decimal,
        side: Side,
        executed_quantity: Decimal,
        cumulative_quantity: Decimal,
        leaves_quantity: Decimal,
    },
    OrderFullyFilled {
        order_id: String,
        algo_id: String,
        symbol: String,
        execution_status: ExecutionType,
        quantity: Decimal,
        fill_price: Decimal,
        side: Side,
        executed_quantity: Decimal,
        cumulative_quantity: Decimal,
        leaves_quantity: Decimal,
    },
    OrderExpired {
        order_id: String,
        algo_id: String,
        symbol: String,
        execution_status: ExecutionType,
        quantity: Decimal,
        side: Side,
        executed_quantity: Decimal,
        cumulative_quantity: Decimal,
        leaves_quantity: Decimal,
    },
    OrderRejected {
        order_id: String,
        algo_id: String,
        symbol: String,
        execution_status: ExecutionType,
        order_quantity: Decimal,
        price: Decimal,
        side: Side,
        order_type: OrderType,
        rejection_reason: String,
        time_in_force: TIF,
    },
    OrderCanceled {
        order_id: String,
        algo_id: String,
        symbol: String,
        execution_status: ExecutionType,
        quantity: Decimal,
        side: Side,
        executed_quantity: Decimal,
        cumulative_quantity: Decimal,
        leaves_quantity: Decimal,
    },
}

macro_rules! format_optional {
    ($value:expr) => {
        match $value {
            Some(ref v) => format!("{}", v), // Format the contained value directly
            None => "None".to_string(),      // Display "none" for None
        }
    };
}

impl fmt::Display for MarketResponses {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MarketResponses::SymbolInformation {
                algo_id,
                min_quantity,
                max_quantity,
                lot_size,
                min_price,
                max_price,
                tick_size,
                min_amount,
            } => {
                write!(
                    f,
                    "SymbolInformation {{ algo_id: {}, min_quantity: {}, max_quantity: {}, lot_size: {}, min_price: {}, max_price: {}, tick_size: {}, min_amount{} }}",
                    algo_id,
                    format_optional!(min_quantity),
                    format_optional!(max_quantity),
                    format_optional!(lot_size),
                    format_optional!(min_price),
                    format_optional!(max_price),
                    format_optional!(tick_size),
                    format_optional!(min_amount),
                )
            }
            MarketResponses::CreateOrderAck {
                order_id,
                algo_id,
                symbol,
                execution_status,
                order_quantity,
                price,
                side,
                order_type,
                time_in_force,
            } => {
                write!(
                    f,
                    "CreateOrderAck {{ order_id: {}, algo_id: {}, symbol: {}, execution_status: {}, order_quantity: {}, price: {}, side: {}, time_in_force: {}, order_type: {} }}",
                    order_id, algo_id, symbol, execution_status, order_quantity, price, side,time_in_force, order_type
                )
            }
            MarketResponses::OrderPartiallyFilled {
                order_id,
                algo_id,
                symbol,
                execution_status,
                quantity,
                fill_price,
                side,
                executed_quantity,
                cumulative_quantity,
                leaves_quantity,
            } => {
                write!(
                    f,
                    "OrderPartiallyFilled {{ order_id: {}, algo_id: {}, symbol: {}, execution_status: {}, quantity: {}, fill_price: {}, side: {}, executed_quantity: {}, cumulative_quantity: {}, leaves_quantity: {} }}",
                    order_id, algo_id, symbol, execution_status, quantity, fill_price, side, executed_quantity, cumulative_quantity, leaves_quantity
                )
            }
            MarketResponses::OrderFullyFilled {
                order_id,
                algo_id,
                symbol,
                execution_status,
                quantity,
                fill_price,
                side,
                executed_quantity,
                cumulative_quantity,
                leaves_quantity,
            } => {
                write!(
                    f,
                    "OrderFullyFilled {{ order_id: {}, algo_id: {}, symbol: {}, execution_status: {}, quantity: {}, fill_price: {}, side: {}, executed_quantity: {}, cumulative_quantity: {}, leaves_quantity: {} }}",
                    order_id, algo_id, symbol, execution_status, quantity, fill_price, side, executed_quantity, cumulative_quantity, leaves_quantity
                )
            }
            MarketResponses::OrderExpired {
                order_id,
                algo_id,
                symbol,
                execution_status,
                quantity,
                side,
                executed_quantity,
                cumulative_quantity,
                leaves_quantity,
            } => {
                write!(
                    f,
                    "OrderExpired {{ order_id: {}, algo_id: {}, symbol: {}, execution_status: {}, quantity: {}, side: {}, executed_quantity: {}, cumulative_quantity: {}, leaves_quantity: {} }}",
                    order_id, algo_id, symbol, execution_status, quantity, side, executed_quantity, cumulative_quantity, leaves_quantity
                )
            }
            MarketResponses::OrderRejected {
                order_id,
                algo_id,
                symbol,
                execution_status,
                order_quantity,
                price,
                side,
                order_type,
                rejection_reason,
                time_in_force,
            } => {
                write!(
                    f,
                    "OrderRejected {{ order_id: {}, algo_id: {}, symbol: {}, execution_status: {}, order_quantity: {}, price: {}, side: {}, time_in_force: {}, order_type: {} , rejection_reason: {} }}",
                    order_id, algo_id, symbol, execution_status, order_quantity, price, side,time_in_force,order_type, rejection_reason
                )
            }
            MarketResponses::OrderCanceled {
                order_id,
                algo_id,
                symbol,
                execution_status,
                quantity,
                side,
                executed_quantity,
                cumulative_quantity,
                leaves_quantity,
            } => {
                write!(
                    f,
                    "OrderCanceled {{ order_id: {}, algo_id: {}, symbol: {}, execution_status: {}, quantity: {}, side: {}, executed_quantity: {}, cumulative_quantity: {}, leaves_quantity: {} }}",
                    order_id, algo_id, symbol, execution_status, quantity, side, executed_quantity, cumulative_quantity, leaves_quantity
                )
            }
        }
    }
}

impl MarketResponses {
    pub fn algo_id(&self) -> &str {
        match self {
            MarketResponses::SymbolInformation { algo_id, .. }
            | MarketResponses::CreateOrderAck { algo_id, .. }
            | MarketResponses::OrderPartiallyFilled { algo_id, .. }
            | MarketResponses::OrderFullyFilled { algo_id, .. }
            | MarketResponses::OrderExpired { algo_id, .. }
            | MarketResponses::OrderRejected { algo_id, .. }
            | MarketResponses::OrderCanceled { algo_id, .. } => algo_id,
        }
    }
}

fn handle_order_trade_event(
    algo: &mpsc::Sender<MarketResponses>,
    algo_id: String,
    event: OrderTradeEvent,
) {
    let execution_type = ExecutionType::from_str(&event.execution_type);

    match execution_type {
        ExecutionType::New => {
            algo.try_send(MarketResponses::CreateOrderAck {
                order_id: event.new_client_order_id,
                algo_id,
                symbol: event.symbol,
                execution_status: execution_type,
                order_quantity: event.qty.parse::<Decimal>().unwrap(),
                side: Side::from_str(&event.side).unwrap_or(Side::Buy),
                order_type: OrderType::from_str(&event.order_type).unwrap_or(OrderType::Limit),
                price: event.price.parse::<Decimal>().unwrap(),
                time_in_force: TIF::from_str(&event.time_in_force).unwrap_or(TIF::GTC),
            })
            .unwrap();
        }
        ExecutionType::Trade => {
            if event.order_status == String::from("FILLED") {
                algo.try_send(MarketResponses::OrderFullyFilled {
                    order_id: event.new_client_order_id,
                    algo_id,
                    symbol: event.symbol,
                    execution_status: execution_type,
                    quantity: event.qty.parse::<Decimal>().unwrap(),
                    fill_price: event.price_last_filled_trade.parse::<Decimal>().unwrap(),
                    side: Side::from_str(&event.side).unwrap(),
                    executed_quantity: event.qty_last_filled_trade.parse::<Decimal>().unwrap(),
                    cumulative_quantity: event
                        .accumulated_qty_filled_trades
                        .parse::<Decimal>()
                        .unwrap(),
                    leaves_quantity: event.qty.parse::<Decimal>().unwrap()
                        - event.qty_last_filled_trade.parse::<Decimal>().unwrap(),
                })
                .unwrap();
            } else {
                algo.try_send(MarketResponses::OrderPartiallyFilled {
                    order_id: event.new_client_order_id,
                    algo_id,
                    symbol: event.symbol,
                    execution_status: execution_type,
                    quantity: event.qty.parse::<Decimal>().unwrap(),
                    fill_price: event.price_last_filled_trade.parse::<Decimal>().unwrap(),
                    side: Side::from_str(&event.side).unwrap(),
                    executed_quantity: event.qty_last_filled_trade.parse::<Decimal>().unwrap(),
                    cumulative_quantity: event
                        .accumulated_qty_filled_trades
                        .parse::<Decimal>()
                        .unwrap(),
                    leaves_quantity: event.qty.parse::<Decimal>().unwrap()
                        - event.qty_last_filled_trade.parse::<Decimal>().unwrap(),
                })
                .unwrap();
            }
        }
        ExecutionType::Expired => {
            algo.try_send(MarketResponses::OrderExpired {
                order_id: event.new_client_order_id,
                algo_id,
                symbol: event.symbol,
                execution_status: execution_type,
                quantity: event.qty.parse::<Decimal>().unwrap(),
                side: Side::from_str(&event.side).unwrap(),
                executed_quantity: event.qty_last_filled_trade.parse::<Decimal>().unwrap(),
                cumulative_quantity: event
                    .accumulated_qty_filled_trades
                    .parse::<Decimal>()
                    .unwrap(),
                leaves_quantity: event.qty.parse::<Decimal>().unwrap()
                    - event
                        .accumulated_qty_filled_trades
                        .parse::<Decimal>()
                        .unwrap(),
            })
            .unwrap();
        }
        ExecutionType::Rejected => {
            algo.try_send(MarketResponses::OrderRejected {
                order_id: event.new_client_order_id,
                algo_id,
                symbol: event.symbol,
                execution_status: execution_type,
                order_quantity: event.qty.parse::<Decimal>().unwrap(),
                side: Side::from_str(&event.side).unwrap_or(Side::Buy),
                order_type: OrderType::from_str(&event.order_type).unwrap_or(OrderType::Limit),
                price: event.price.parse::<Decimal>().unwrap(),
                rejection_reason: event.order_reject_reason,
                time_in_force: TIF::from_str(&event.time_in_force).unwrap_or(TIF::GTC),
            })
            .unwrap();
        }
        ExecutionType::Canceled => {
            algo.try_send(MarketResponses::OrderCanceled {
                order_id: event.new_client_order_id,
                algo_id,
                symbol: event.symbol,
                execution_status: execution_type,
                quantity: event.qty.parse::<Decimal>().unwrap(),
                side: Side::from_str(&event.side).unwrap(),
                executed_quantity: event.qty_last_filled_trade.parse::<Decimal>().unwrap(),
                cumulative_quantity: event
                    .accumulated_qty_filled_trades
                    .parse::<Decimal>()
                    .unwrap(),
                leaves_quantity: event.qty.parse::<Decimal>().unwrap()
                    - event.qty_last_filled_trade.parse::<Decimal>().unwrap(),
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
    pub async fn new(market_config: MarketConfig) -> (Self, JoinHandle<()>) {
        let (sender, receiver) = mpsc::channel(100);

        let actor = MarketSession::new(market_config, receiver, sender.clone()).await;

        let handle = tokio::spawn(run_my_actor(actor));

        (Self { sender }, handle)
    }

    pub fn create_order(
        &self,
        symbol: String,
        price: Decimal,
        quantity: Decimal,
        side: Side,
        order_type: OrderType,
        time_in_force: TIF,
        sender: mpsc::Sender<MarketResponses>,
        order_id: String,
        algo_id: String,
    ) {
        self.sender
            .try_send(MarketMessages::CreateOrder {
                symbol,
                price,
                quantity,
                side,
                order_type,
                time_in_force,
                sender,
                order_id,
                algo_id,
            })
            .unwrap();
    }

    pub fn get_symbol_info(
        &self,
        symbol: String,
        algo_id: String,
        sender: mpsc::Sender<MarketResponses>,
    ) {
        self.sender
            .try_send(MarketMessages::GetSymbolInformation {
                symbol,
                algo_id,
                sender,
            })
            .unwrap();
    }
}

struct MarketSession {
    receiver: mpsc::Receiver<MarketMessages>,
    handles: Vec<task::JoinHandle<()>>,
    account: Account,
    general: General,
    market_config: MarketConfig,
    sender: mpsc::Sender<MarketMessages>,
    algo_contexts: Arc<std::sync::Mutex<HashMap<String, (String, mpsc::Sender<MarketResponses>)>>>,
}

impl MarketSession {
    pub async fn new(
        market_config: MarketConfig,
        receiver: mpsc::Receiver<MarketMessages>,
        sender: mpsc::Sender<MarketMessages>,
    ) -> Self {
        let api_key = Some(market_config.api_key.clone());
        let api_secret = Some(market_config.api_secret.clone());

        let result = task::spawn_blocking(move || {
            // let api_key = Some(market_config.api_key.into());
            // let api_secret = Some(market_config.api_secret.into());
            let config = Config::testnet();
            let user_stream: Account =
                Binance::new_with_config(api_key.clone(), api_secret.clone(), &config);

            let general: General = Binance::new_with_config(api_key, api_secret, &config);

            (user_stream, general) // This should return the actual result
        })
        .await; // Await the result of the blocking tas

        let (account, general) = result.unwrap();

        Self {
            receiver,
            handles: Vec::new(),
            account: account,
            general: general,
            market_config,
            sender: sender,
            algo_contexts: Arc::new(std::sync::Mutex::new(HashMap::new())),
        }
    }

    pub async fn handle(&mut self, market_message: MarketMessages) {
        match market_message {
            MarketMessages::GetSymbolInformation {
                symbol,
                algo_id,
                sender,
            } => {
                let general = self.general.clone();

                task::spawn_blocking(move || match general.get_symbol_info(symbol) {
                    Ok(answer) => {
                        println!("Symbol info siu {:?}", answer);

                        let (mut min_qty, mut max_qty, mut lot_size) = (None, None, None);
                        let (mut min_price, mut max_price, mut tick_size) = (None, None, None);
                        let mut min_amount = None;

                        for filter in &answer.filters {
                            match filter {
                                Filters::LotSize {
                                    min_qty: mq,
                                    max_qty: mxq,
                                    step_size,
                                } => {
                                    min_qty = mq.parse::<Decimal>().ok();
                                    max_qty = mxq.parse::<Decimal>().ok();
                                    lot_size = step_size.parse::<Decimal>().ok();
                                }
                                Filters::PriceFilter {
                                    min_price: mp,
                                    max_price: mxp,
                                    tick_size: ts,
                                } => {
                                    min_price = mp.parse::<Decimal>().ok();
                                    max_price = mxp.parse::<Decimal>().ok();
                                    tick_size = ts.parse::<Decimal>().ok();
                                }
                                Filters::Notional {
                                    notional,
                                    min_notional,
                                    apply_to_market,
                                    avg_price_mins,
                                } => {
                                    min_amount = min_notional
                                        .as_ref()
                                        .and_then(|notional| notional.parse::<Decimal>().ok());
                                }
                                _ => {}
                            }
                        }

                        sender
                            .try_send(MarketResponses::SymbolInformation {
                                algo_id,
                                min_quantity: min_qty,
                                max_quantity: max_qty,
                                lot_size,
                                min_price,
                                max_price,
                                tick_size,
                                min_amount,
                            })
                            .unwrap();
                    }
                    Err(e) => {
                        println!("Didn't receive symbol info: Error: {}", e);
                        sender
                            .try_send(MarketResponses::SymbolInformation {
                                algo_id,
                                min_quantity: None,
                                max_quantity: None,
                                lot_size: None,
                                min_price: None,
                                max_price: None,
                                tick_size: None,
                                min_amount: None,
                            })
                            .unwrap();
                    }
                });
            }

            MarketMessages::CreateOrder {
                symbol,
                price,
                quantity,
                side,
                order_type,
                time_in_force,
                sender,
                order_id,
                algo_id,
            } => {
                let account2 = self.account.clone();
                let order_side = OrderSide::from_int(side.to_int()).unwrap_or(OrderSide::Buy);
                let time_in_force_binance =
                    TimeInForce::from_int(time_in_force.to_int()).unwrap_or(TimeInForce::GTC);
                let order_type_binance = BinanceOrderType::from_int(order_type.to_int())
                    .unwrap_or(BinanceOrderType::Limit);
                let mut algo_contexts = self.algo_contexts.lock().unwrap();
                algo_contexts.insert(order_id.clone(), (algo_id.clone(), sender.clone()));
                task::spawn_blocking(move || {
                    match account2.custom_order(
                        symbol.clone(),
                        quantity.clone().to_f64().unwrap(),
                        price.clone().to_f64().unwrap(),
                        None,
                        order_side,
                        order_type_binance,
                        time_in_force_binance,
                        Some(order_id.clone()),
                    ) {
                        Ok(_) => {}
                        Err(e) => {
                            println!("Error: {}", e);
                            sender
                                .try_send(MarketResponses::OrderRejected {
                                    order_id,
                                    algo_id,
                                    symbol,
                                    execution_status: ExecutionType::Rejected,
                                    order_quantity: quantity,
                                    side,
                                    order_type,
                                    price,
                                    time_in_force,
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

    let api_key = Some(actor.market_config.api_key.clone());
    let api_secret = Some(actor.market_config.api_secret.clone());

    task::spawn_blocking(move || {
        let keep_running = AtomicBool::new(true); // Used to control the event loop
                                                  // let api_key =
                                                  //     Some("8Rz2XDFFItHESqPZ7CUl3Gk4JyQU73jBsqMAEhOKaizD4Fx28ryWNQuOg9OMweND".into());
                                                  // let api_secret =
                                                  //     Some("3Bo4CXMF95vj17hzREUxIzLP5wQfueU8YMplVREKr1pGQqCkBz088XOCw4N8mGV6".into());
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
                        let (algo_id, algo_context) =
                            algo_contexts.get(&trade.new_client_order_id).unwrap();
                        handle_order_trade_event(algo_context, algo_id.clone(), trade);
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
    pub fn new<AlgoId>(
        market_session_handle: &MarketSessionHandle,
        meesage_sender: &mpsc::Sender<MarketResponses>,
        algo_id: AlgoId,
    ) -> Self
    where
        AlgoId: Into<String>,
    {
        Self {
            market_session_handle: market_session_handle.clone(),
            meesage_sender: meesage_sender.clone(),
            algo_id: algo_id.into(),
        }
    }

    pub fn create_ioc_order<Symbol>(
        &self,
        symbol: Symbol,
        price: Decimal,
        quantity: Decimal,
        side: &Side,
    ) where
        Symbol: Into<String>,
    {
        self.create_order(
            symbol.into(),
            price.into(),
            quantity.into(),
            side.clone(),
            OrderType::Limit,
            TIF::IOC,
        );
    }

    pub fn create_order<Symbol>(
        &self,
        symbol: Symbol,
        price: Decimal,
        quantity: Decimal,
        side: Side,
        order_type: OrderType,
        time_inforce: TIF,
    ) where
        Symbol: Into<String>,
    {
        let random_id = Uuid::new_v4().to_string();

        self.market_session_handle.create_order(
            symbol.into(),
            price.into(),
            quantity.into(),
            side,
            order_type,
            time_inforce,
            self.meesage_sender.clone(),
            random_id,
            self.algo_id.clone(),
        );
    }

    pub fn get_symbol_info<Symbol>(&self, symbol: Symbol)
    where
        Symbol: Into<String>,
    {
        self.market_session_handle.get_symbol_info(
            symbol.into(),
            self.algo_id.clone(),
            self.meesage_sender.clone(),
        );
    }
}
