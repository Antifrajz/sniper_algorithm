use core::fmt;
use std::collections::HashMap;
use std::thread::{self, sleep};
use std::time::Duration;
use std::{str::FromStr, sync::Arc};

use logging::algo_logger::AlgoLogger;

use feed::actor::{Feed, FeedHandle, FeedMessages};
use futures::future::join_all;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::{
    prelude::{FromPrimitive, Zero},
    Decimal,
};

use async_trait::async_trait;
use serde::Deserialize;
use tokio::spawn;
use tokio::{
    sync::{mpsc, Mutex},
    task,
};

use crate::config::{self, AlgoParameters};
use crate::feed::actor::L2Data;
use crate::logging::algo_report::AlgoPdfLogger;
use crate::market::market::TIF;
use crate::{
    feed::{
        self,
        actor::{FeedService, FeedUpdate, L1Data},
    },
    market::market::{ExecutionType, MarketResponses, MarketService, MarketSessionHandle, Side},
};
use crate::{log_debug, log_error, log_info, logging, report};

pub enum AlgoMessages {
    CreateAlgo(AlgoParameters),
}

pub struct AlgoService {
    sender: mpsc::Sender<AlgoMessages>,
}

impl AlgoService {
    pub async fn new(feed_handle: FeedHandle, market_session_handle: MarketSessionHandle) -> Self {
        let (sender, receiver) = mpsc::channel(10);

        let actor = AlgoContext::new(receiver, feed_handle, market_session_handle);

        tokio::spawn(run_my_actor(actor));

        Self { sender }
    }

    pub fn create_algo(&self, params: AlgoParameters) {
        self.sender
            .try_send(AlgoMessages::CreateAlgo(params))
            .unwrap();
    }
}

struct AlgoContext {
    algo_context_id: String,
    algo_messages_receiver: mpsc::Receiver<AlgoMessages>,
    feed_sender: mpsc::Sender<FeedUpdate>,
    feed_receiver: mpsc::Receiver<FeedUpdate>,
    market_sender: mpsc::Sender<MarketResponses>,
    market_receiver: mpsc::Receiver<MarketResponses>,
    handles: Vec<task::JoinHandle<()>>,
    algorithams: HashMap<String, Arc<Mutex<Box<dyn Algorithm + Send>>>>,
    feed_hadnle: FeedHandle,
    market_session_handle: MarketSessionHandle,
}

impl AlgoContext {
    pub fn new(
        algo_messages_receiver: mpsc::Receiver<AlgoMessages>,
        feed_hadnle: FeedHandle,
        market_session_handle: MarketSessionHandle,
    ) -> Self {
        let (feed_sender, feed_receiver) = mpsc::channel(10);
        let (market_sender, market_receiver) = mpsc::channel(10);

        let algo_context_id = String::from("contextId");

        Self {
            algo_context_id,
            algo_messages_receiver,
            feed_sender,
            feed_receiver,
            market_sender,
            market_receiver,
            handles: Vec::new(),
            algorithams: HashMap::new(),
            feed_hadnle,
            market_session_handle,
        }
    }

    pub fn create_algo(&mut self, algo_parameters: AlgoParameters) {
        let market_service = MarketService::new(
            self.market_session_handle.clone(),
            self.market_sender.clone(),
            algo_parameters.algo_id.clone(),
        );

        let feed_service = FeedService::new(
            self.feed_hadnle.clone(),
            self.algo_context_id.clone(),
            algo_parameters.algo_id.clone(),
            self.feed_sender.clone(),
        );

        match algo_parameters.algo_type {
            AlgoType::Sniper => {
                let algo = SniperAlgo::new(algo_parameters.clone(), market_service, feed_service);
                self.algorithams.insert(
                    algo_parameters.algo_id.clone(),
                    Arc::new(Mutex::new(Box::new(algo))),
                );
            }
        }
    }

    pub fn handle_algo_message(&mut self, algo_message: AlgoMessages) {
        match algo_message {
            AlgoMessages::CreateAlgo(params) => {
                self.create_algo(params);
            }
        }
    }

    pub async fn handle_feed_update(&mut self, feed_update: FeedUpdate) {
        match feed_update {
            FeedUpdate::L1Update(algo_ids, l1_data) => {
                let futures: Vec<_> = algo_ids
                    .into_iter()
                    .filter_map(|algo_id| self.algorithams.get(&algo_id).cloned())
                    .map(|algo| {
                        let l1_data = l1_data.clone();
                        spawn(async move {
                            let mut algo = algo.lock().await;
                            algo.handle_l1(l1_data).await;
                        })
                    })
                    .collect();

                join_all(futures).await;
            }
            FeedUpdate::L2Update(algo_ids, l2_data) => {
                let futures: Vec<_> = algo_ids
                    .into_iter()
                    .filter_map(|algo_id| self.algorithams.get(&algo_id).cloned())
                    .map(|algo| {
                        let l2_data = l2_data.clone();
                        spawn(async move {
                            let mut algo = algo.lock().await;
                            algo.handle_l2(l2_data).await;
                        })
                    })
                    .collect();

                join_all(futures).await;
            }
        }
    }

    pub async fn handle_market_reponse(&mut self, market_response: MarketResponses) {
        let algo_id = market_response.algo_id();

        if let Some(algo) = self.algorithams.get_mut(algo_id) {
            let mut algo = algo.lock().await;
            algo.handle_market_reponse(market_response);
        }
    }
}

async fn run_my_actor(mut actor: AlgoContext) {
    loop {
        tokio::select! {
            Some(algo_message) = actor.algo_messages_receiver.recv() => {
               actor.handle_algo_message(algo_message);
            },
            Some(feed_update) = actor.feed_receiver.recv() => {
                actor.handle_feed_update(feed_update).await;
            },
            Some(market_response) = actor.market_receiver.recv() => {
                actor.handle_market_reponse(market_response).await;
            },
            else => break,
        }
    }
}
#[derive(Debug)]
enum State {
    New,
    WaitingForMarketConditions,
    PendingCreate,
    Working,
    Done,
}

impl fmt::Display for State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let state_name = match self {
            State::New => "New",
            State::WaitingForMarketConditions => "WaitingForMarketConditions",
            State::PendingCreate => "PendingCreate",
            State::Working => "Working",
            State::Done => "Done",
        };
        write!(f, "{}", state_name)
    }
}

#[derive(Debug)]
enum Event {
    SymbolInformation {
        min_quantity: Option<Decimal>,
        max_quantity: Option<Decimal>,
        lot_size: Option<Decimal>,
        min_price: Option<Decimal>,
        max_price: Option<Decimal>,
        tick_size: Option<Decimal>,
    },
    FeedUpdate {
        quantity: Decimal,
        price: Decimal,
    },
    CreateOrderAck {
        order_id: String,
        symbol: String,
        execution_status: ExecutionType,
        order_quantity: Decimal,
        price: Decimal,
        side: Side,
        time_in_force: TIF,
    },
    CreateOrderRej {
        order_id: String,
        symbol: String,
        execution_status: ExecutionType,
        order_quantity: Decimal,
        price: Decimal,
        side: Side,
        rejection_reason: String,
        time_in_force: TIF,
    },
    OrderPartiallyFilled {
        order_id: String,
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
        symbol: String,
        execution_status: ExecutionType,
        quantity: Decimal,
        side: Side,
        executed_quantity: Decimal,
        cumulative_quantity: Decimal,
        leaves_quantity: Decimal,
    },
    OrderCanceled {
        order_id: String,
        symbol: String,
        execution_status: ExecutionType,
        quantity: Decimal,
        side: Side,
        executed_quantity: Decimal,
        cumulative_quantity: Decimal,
        leaves_quantity: Decimal,
    },
}

impl fmt::Display for Event {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let event_name = match self {
            Event::SymbolInformation { .. } => "SymbolInformation",
            Event::FeedUpdate { .. } => "FeedUpdate",
            Event::CreateOrderAck { .. } => "CreateOrderAck",
            Event::CreateOrderRej { .. } => "CreateOrderRej",
            Event::OrderPartiallyFilled { .. } => "OrderPartiallyFilled",
            Event::OrderFullyFilled { .. } => "OrderFullyFilled",
            Event::OrderExpired { .. } => "OrderExpired",
            Event::OrderCanceled { .. } => "OrderCanceled",
        };
        write!(f, "{}", event_name)
    }
}

struct SniperAlgo {
    algo_parameters: AlgoParameters,
    market_sevice: MarketService,
    feed_service: FeedService,
    state: State,
    symbol_information: SymbolInformation,
    remaining_quantity: Decimal,
    executed_quantity: Decimal,
    exposed_quantity: Decimal,
    logger: AlgoLogger,
    pdf_report: AlgoPdfLogger,
}

#[async_trait]
impl Algorithm for SniperAlgo {
    async fn handle_l1(&mut self, l1_data: L1Data) {
        log_debug!(self.logger, "handle_l1", "Handling L1 update {}", l1_data);
        match self.algo_parameters.side {
            Side::Buy => {
                self.on_event(Event::FeedUpdate {
                    quantity: l1_data.best_ask_level.quantity,
                    price: l1_data.best_ask_level.price,
                });
            }
            Side::Sell => {
                self.on_event(Event::FeedUpdate {
                    quantity: l1_data.best_bid_level.quantity,
                    price: l1_data.best_bid_level.price,
                });
            }
        }
    }

    async fn handle_l2(&mut self, l2_data: L2Data) {
        log_debug!(self.logger, "handle_l2", "Handling L2 update {}", l2_data);
    }

    fn handle_market_reponse(&mut self, market_response: MarketResponses) {
        log_debug!(
            self.logger,
            "handle_market_reponse",
            "Handling Market Response {}",
            market_response
        );
        match market_response {
            MarketResponses::SymbolInformation {
                algo_id: _,
                min_quantity,
                max_quantity,
                lot_size,
                min_price,
                max_price,
                tick_size,
            } => {
                self.on_event(Event::SymbolInformation {
                    min_quantity,
                    max_quantity,
                    lot_size,
                    min_price,
                    max_price,
                    tick_size,
                });
            }

            MarketResponses::CreateOrderAck {
                order_id,
                algo_id: _,
                symbol,
                execution_status,
                order_quantity,
                price,
                side,
                time_in_force,
            } => self.on_event(Event::CreateOrderAck {
                order_id,
                symbol,
                execution_status,
                order_quantity,
                price,
                side,
                time_in_force,
            }),
            MarketResponses::OrderPartiallyFilled {
                order_id,
                algo_id: _,
                symbol,
                execution_status,
                quantity,
                fill_price,
                side,
                executed_quantity,
                cumulative_quantity,
                leaves_quantity,
            } => self.on_event(Event::OrderPartiallyFilled {
                order_id,
                symbol,
                execution_status,
                quantity,
                fill_price,
                side,
                executed_quantity,
                cumulative_quantity,
                leaves_quantity,
            }),
            MarketResponses::OrderFullyFilled {
                order_id,
                algo_id: _,
                symbol,
                execution_status,
                quantity,
                fill_price,
                side,
                executed_quantity,
                cumulative_quantity,
                leaves_quantity,
            } => self.on_event(Event::OrderFullyFilled {
                order_id,
                symbol,
                execution_status,
                quantity,
                fill_price,
                side,
                executed_quantity,
                cumulative_quantity,
                leaves_quantity,
            }),
            MarketResponses::OrderExpired {
                order_id,
                algo_id: _,
                symbol,
                execution_status,
                quantity,
                side,
                executed_quantity,
                cumulative_quantity,
                leaves_quantity,
            } => self.on_event(Event::OrderExpired {
                order_id,
                symbol,
                execution_status,
                quantity,
                side,
                executed_quantity,
                cumulative_quantity,
                leaves_quantity,
            }),
            MarketResponses::OrderRejected {
                order_id,
                algo_id: _,
                symbol,
                execution_status,
                order_quantity,
                price,
                side,
                rejection_reason,
                time_in_force,
            } => self.on_event(Event::CreateOrderRej {
                order_id,
                symbol,
                execution_status,
                order_quantity,
                price,
                side,
                rejection_reason,
                time_in_force,
            }),
            MarketResponses::OrderCanceled {
                order_id,
                algo_id: _,
                symbol,
                execution_status,
                quantity,
                side,
                executed_quantity,
                cumulative_quantity,
                leaves_quantity,
            } => self.on_event(Event::OrderCanceled {
                order_id,
                symbol,
                execution_status,
                quantity,
                side,
                executed_quantity,
                cumulative_quantity,
                leaves_quantity,
            }),
        }
    }
}

impl SniperAlgo {
    pub fn new(
        algo_parameters: AlgoParameters,
        market_sevice: MarketService,
        feed_service: FeedService,
    ) -> Self {
        let remaining_quantity = algo_parameters.quantity;
        market_sevice.get_symbol_info(algo_parameters.make_symbol());
        let mut pdf_report = AlgoPdfLogger::new(
            &algo_parameters.algo_id,
            &algo_parameters.algo_type.to_string(),
        );

        report!(
            pdf_report,
            "{} Algorithm successfully created \
        for Symbol {} on Side {} with Quantity {} and Limit Price {}.",
            algo_parameters.algo_type,
            algo_parameters.make_symbol(),
            algo_parameters.side,
            algo_parameters.quantity,
            algo_parameters.price
        );

        Self {
            algo_parameters: algo_parameters.clone(),
            market_sevice,
            feed_service,
            state: State::New,
            symbol_information: SymbolInformation::new(),
            remaining_quantity,
            executed_quantity: Decimal::zero(),
            exposed_quantity: Decimal::zero(),
            logger: AlgoLogger::new(&algo_parameters.algo_id),
            pdf_report,
        }
    }

    fn should_react(side: &Side, price: &Decimal, limit_price: &Decimal) -> bool {
        match side {
            Side::Buy => price < limit_price,
            Side::Sell => price > limit_price,
        }
    }

    fn round_quantity_to_lot_size(&self, value: &mut Decimal) {
        let lot_size = &self.symbol_information.lot_size;
        if let Some(lot_size) = lot_size {
            *value = (*value / lot_size).floor() * lot_size;
        }
    }

    fn round_price_to_tick_size(&self, value: &mut Decimal) {
        let tick_size = &self.symbol_information.tick_size;
        if let Some(tick_size) = tick_size {
            *value = (*value / tick_size).floor() * tick_size;
        }
    }

    fn on_event(&mut self, event: Event) {
        match (&self.state, event) {
            (
                State::New,
                Event::SymbolInformation {
                    min_quantity,
                    max_quantity,
                    lot_size,
                    min_price,
                    max_price,
                    tick_size,
                },
            ) => {
                self.symbol_information.set_values(
                    min_quantity,
                    max_quantity,
                    lot_size,
                    min_price,
                    max_price,
                    tick_size,
                );

                if let Some(min_quantity) = self.symbol_information.min_quantity {
                    if self.algo_parameters.quantity < min_quantity {
                        log_error!(
                            self.logger,
                            "SymbolInformation",
                            "The algorithm's quantity {} is below the minimum required \
                            for exposure on the exchange {}, so the algorithm will not \
                            be initiated.",
                            self.algo_parameters.quantity,
                            min_quantity
                        );

                        report!(
                            self.pdf_report,
                            "The algorithm's quantity {} is below the minimum required \
                            for exposure on the exchange {}, so the algorithm will be REJECTED.",
                            self.algo_parameters.quantity,
                            min_quantity
                        );

                        report!(self.pdf_report, "ALGORITHM REJECTED",);

                        self.pdf_report.write_to_pdf().unwrap();
                        self.state = State::Done;
                        return;
                    }
                }
                self.feed_service
                    .subscribe_to_l1(&self.algo_parameters.base, &self.algo_parameters.quote);
                self.state = State::WaitingForMarketConditions;
            }

            (state @ State::New, event) => {
                #[cfg(debug_assertions)]
                log_error!(
                    self.logger,
                    "unsupportedEvent",
                    "Ignoring event {} as it is not supported in the current state {}.",
                    event,
                    state
                );
            }

            (
                State::WaitingForMarketConditions,
                Event::FeedUpdate {
                    quantity,
                    mut price,
                },
            ) => {
                if Self::should_react(
                    &self.algo_parameters.side,
                    &price,
                    &self.algo_parameters.price,
                ) {
                    let mut order_quantity;
                    if quantity < self.remaining_quantity {
                        order_quantity = quantity;
                    } else {
                        order_quantity = self.remaining_quantity;
                    }

                    self.round_quantity_to_lot_size(&mut order_quantity);
                    self.round_price_to_tick_size(&mut price);

                    if let Some(min_quantity) = self.symbol_information.min_quantity {
                        if min_quantity >= order_quantity {
                            return;
                        }
                    }

                    if let Some(min_price) = self.symbol_information.min_price {
                        if min_price >= price {
                            return;
                        }
                    }

                    log_info!(
                        self.logger,
                        "onFeedUpdate",
                        "Attempting to place an order on the market: \
                        Symbol: {}, Side: {}, Price: {}, Quantity: {}.",
                        self.algo_parameters.make_symbol(),
                        price,
                        order_quantity,
                        self.algo_parameters.side.clone(),
                    );

                    self.market_sevice.create_ioc_order(
                        self.algo_parameters.make_symbol(),
                        price,
                        order_quantity,
                        self.algo_parameters.side.clone(),
                    );
                    self.state = State::PendingCreate;
                } else {
                    log_debug!(
                        self.logger,
                        "FeedUpdateEvent",
                        "Disregarding the update due to the Price {} \
                        being above the acceptable threshold.",
                        price,
                    );
                }
            }

            (state @ State::WaitingForMarketConditions, event) => {
                #[cfg(debug_assertions)]
                log_error!(
                    self.logger,
                    "unsupportedEvent",
                    "Ignoring event {} as it is not supported in the current state {}.",
                    event,
                    state
                );
            }
            (
                State::PendingCreate,
                Event::CreateOrderAck {
                    order_id,
                    symbol,
                    execution_status,
                    order_quantity,
                    price,
                    side,
                    time_in_force,
                },
            ) => {
                log_info!(
                    self.logger,
                    "CreateOrderAckEvent",
                    "Received Order Acknowledgment: Order ID {}, \
                    Symbol {}, Side {}, Time In Force {}, Quantity {}, Price {}, with Status {}",
                    order_id,
                    symbol,
                    side,
                    time_in_force,
                    order_quantity,
                    price,
                    execution_status
                );

                report!(
                    self.pdf_report,
                    "Algorithm exposed a quantity of {} for Symbol {} at Price {} \
                    with Time In Force {}",
                    order_quantity,
                    symbol,
                    price,
                    time_in_force
                );

                self.exposed_quantity += &order_quantity;
                self.remaining_quantity -= &order_quantity;

                self.state = State::Working;
            }
            (
                State::PendingCreate,
                Event::CreateOrderRej {
                    order_id,
                    symbol,
                    execution_status,
                    order_quantity,
                    price,
                    side,
                    rejection_reason,
                    time_in_force,
                },
            ) => {
                log_info!(
                    self.logger,
                    "CreateOrderRejEvent",
                    "Received a creation rejection while attempting \
                    to place an order: Order ID {}, Symbol {}, Side {}, \
                    Quantity {}, Price {}, Status {}, Rejection Reason {}.",
                    order_id,
                    symbol,
                    side,
                    order_quantity,
                    price,
                    execution_status,
                    rejection_reason
                );

                report!(
                    self.pdf_report,
                    "The algorithm attempted to expose a quantity of {} for Symbol {} \
                    at Price {} with Time In Force {}, but the order was rejected due to {}.",
                    order_quantity,
                    symbol,
                    price,
                    time_in_force,
                    rejection_reason
                );

                self.state = State::WaitingForMarketConditions;
            }

            (state @ State::PendingCreate, event) => {
                #[cfg(debug_assertions)]
                log_error!(
                    self.logger,
                    "unsupportedEvent",
                    "Ignoring event {} as it is not supported in the current state {}.",
                    event,
                    state
                );
            }

            (
                State::Working,
                Event::OrderPartiallyFilled {
                    order_id,
                    symbol,
                    execution_status,
                    quantity,
                    fill_price,
                    side,
                    executed_quantity,
                    cumulative_quantity,
                    leaves_quantity,
                },
            ) => {
                log_info!(
                    self.logger,
                    "OrderPartiallyFilledEvent",
                    "Order {} for Symbol {}, Side {}, has been \
                    partially filled with Quantity {} at Price {}. \
                    The total order quantity is {}, cumulative \
                    executed quantity is {}, remaining unexecuted   \
                    quantity is {}, and the execution status of the order is {}.",
                    order_id,
                    symbol,
                    side,
                    executed_quantity,
                    fill_price,
                    quantity,
                    cumulative_quantity,
                    leaves_quantity,
                    execution_status
                );

                self.executed_quantity += &executed_quantity;
                self.exposed_quantity -= &executed_quantity;

                report!(
                    self.pdf_report,
                    "An order exposed on the exchange for Symbol {} was partially executed, \
                    with an executed quantity of {} at Price {}. Currently, \
                    there is an exposed quantity of {} on the exchange, a \
                    remaining quantity of {}, and a total executed quantity of {} so far.",
                    symbol,
                    executed_quantity,
                    fill_price,
                    self.exposed_quantity,
                    self.remaining_quantity,
                    self.executed_quantity
                );
            }
            (
                State::Working,
                Event::OrderFullyFilled {
                    order_id,
                    symbol,
                    execution_status,
                    quantity,
                    fill_price,
                    side,
                    executed_quantity,
                    cumulative_quantity,
                    leaves_quantity,
                },
            ) => {
                log_info!(
                    self.logger,
                    "OrderFullyFilledEvent",
                    "Order {} for Symbol {}, Side {}, has been fully \
                    filled with execution Quantity {} at Price {}. \
                    The total order quantity is {}, cumulative \
                    executed quantity is {}, remaining unexecuted \
                    quantity is {}, and the execution status of the order is {}.",
                    order_id,
                    symbol,
                    side,
                    executed_quantity,
                    fill_price,
                    quantity,
                    cumulative_quantity,
                    leaves_quantity,
                    execution_status
                );

                self.executed_quantity += executed_quantity;
                self.exposed_quantity -= executed_quantity;

                if match self.symbol_information.min_quantity {
                    Some(min_quantity) => self.remaining_quantity < min_quantity,
                    None => self.remaining_quantity.is_zero(),
                } {
                    if self.remaining_quantity.is_zero() {
                        log_info!(
                            self.logger,
                            "AlgoDone",
                            "The algorithm has been fully executed. \
                            Unsubscribing from feed updates for Symbol {}.",
                            symbol
                        );

                        report!(
                            self.pdf_report,
                            "An order exposed on the exchange for Symbol {} has \
                            been fully filled, with an executed quantity of {} at \
                            Price {}. This completes the algorithm, achieving a \
                            total executed quantity of {} across its lifetime, \
                            with a remaining quantity of {}.",
                            symbol,
                            executed_quantity,
                            fill_price,
                            self.executed_quantity,
                            self.remaining_quantity
                        );
                        report!(self.pdf_report, "ALGORITHM EXECUTED",);
                    } else {
                        log_info!(
                            self.logger,
                            "AlgoDone",
                            "The remaining quantity is below the minimum limit, \
                            preventing further order exposure. Therefore, \
                            this algorithm will be marked as complete. \
                            Unsubscribing from feed updates for Symbol {}.",
                            symbol
                        );
                        report!(
                            self.pdf_report,
                            "An order exposed on the exchange for Symbol {} has been \
                            fully filled with an executed quantity of {} at \
                            Price {}. This completes the algorithm, as the \
                            remaining quantity {} is below the minimum threshold \
                            required for exposure {}. The algorithm achieved a total \
                            executed quantity of {} over its lifetime.",
                            symbol,
                            executed_quantity,
                            fill_price,
                            self.remaining_quantity,
                            self.symbol_information
                                .min_quantity
                                .unwrap_or(Decimal::zero()),
                            self.executed_quantity
                        );
                        report!(self.pdf_report, "ALGORITHM DONE",);
                    }

                    self.state = State::Done;
                    self.feed_service.unsubscribe_from_l1(
                        self.algo_parameters.base.clone(),
                        self.algo_parameters.quote.clone(),
                    );
                    self.pdf_report.write_to_pdf().unwrap();
                } else {
                    println!("Remaining quantity: {}", self.remaining_quantity);
                    log_info!(
                        self.logger,
                        "transitionToWaitingForMarketConditions",
                        "The algorithm's remaining quantity is {}, \
                        so we are awaiting further feed updates.",
                        self.remaining_quantity
                    );

                    report!(
                        self.pdf_report,
                        "An order exposed on the exchange for Symbol {} was fully executed, \
                        with an executed quantity of {} at Price {}. The algorithm has a \
                        remaining quantity of {}, with a total executed quantity of {} so far.",
                        symbol,
                        executed_quantity,
                        fill_price,
                        self.remaining_quantity,
                        self.executed_quantity
                    );

                    self.state = State::WaitingForMarketConditions;
                }
            }
            (
                State::Working,
                Event::OrderExpired {
                    order_id,
                    symbol,
                    execution_status,
                    quantity,
                    side,
                    executed_quantity: _,
                    cumulative_quantity,
                    leaves_quantity,
                },
            ) => {
                log_info!(
                    self.logger,
                    "OrderExpiredEvent",
                    "Order {} for Symbol {}, Side {}, has expired. \
                    The total order quantity was {}, with a cumulative \
                    executed quantity of {}, a remaining unexecuted quantity of {}, \
                    and an execution status of {}.",
                    order_id,
                    symbol,
                    side,
                    quantity,
                    cumulative_quantity,
                    leaves_quantity,
                    execution_status
                );

                self.exposed_quantity -= leaves_quantity;
                self.remaining_quantity += leaves_quantity;

                log_info!(
                    self.logger,
                    "transitionToWaitingForMarketConditions",
                    "The algorithm's remaining quantity is {}, \
                    so we are awaiting further feed updates.",
                    self.remaining_quantity
                );

                self.state = State::WaitingForMarketConditions;
            }

            (state @ State::Working, event) => {
                #[cfg(debug_assertions)]
                log_error!(
                    self.logger,
                    "unsupportedEvent",
                    "Ignoring event {} as it is not supported in the current state {}.",
                    event,
                    state
                );
            }

            (state @ State::Done, event) => {
                #[cfg(debug_assertions)]
                log_error!(
                    self.logger,
                    "unsupportedEvent",
                    "Ignoring event {} as it is not supported in the current state {}.",
                    event,
                    state
                );
            }
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AlgoType {
    Sniper,
}

impl fmt::Display for AlgoType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AlgoType::Sniper => write!(f, "Sniper"),
        }
    }
}

impl AlgoType {
    pub fn from_string(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "sniper" => Some(AlgoType::Sniper),
            _ => None,
        }
    }

    pub fn to_string(&self) -> &'static str {
        match self {
            AlgoType::Sniper => "Sniper",
        }
    }
}

struct SymbolInformation {
    min_quantity: Option<Decimal>,
    max_quantity: Option<Decimal>,
    lot_size: Option<Decimal>,
    min_price: Option<Decimal>,
    max_price: Option<Decimal>,
    tick_size: Option<Decimal>,
}

impl SymbolInformation {
    pub fn new() -> Self {
        SymbolInformation {
            min_quantity: None,
            max_quantity: None,
            lot_size: None,
            min_price: None,
            max_price: None,
            tick_size: None,
        }
    }

    pub fn set_values(
        &mut self,
        min_quantity: Option<Decimal>,
        max_quantity: Option<Decimal>,
        lot_size: Option<Decimal>,
        min_price: Option<Decimal>,
        max_price: Option<Decimal>,
        tick_size: Option<Decimal>,
    ) {
        if let Some(value) = min_quantity {
            self.min_quantity = Some(value);
        }
        if let Some(value) = max_quantity {
            self.max_quantity = Some(value);
        }
        if let Some(value) = lot_size {
            self.lot_size = Some(value);
        }
        if let Some(value) = min_price {
            self.min_price = Some(value);
        }
        if let Some(value) = max_price {
            self.max_price = Some(value);
        }
        if let Some(value) = tick_size {
            self.tick_size = Some(value);
        }
    }
}

#[async_trait]
trait Algorithm {
    async fn handle_l1(&mut self, l1_data: L1Data);
    async fn handle_l2(&mut self, l2_data: L2Data);
    fn handle_market_reponse(&mut self, market_response: MarketResponses);
}
