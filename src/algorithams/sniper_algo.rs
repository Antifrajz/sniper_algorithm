use crate::algorithams::algorithm::Algorithm;
use crate::common_types::order_types::OrderType;
use crate::common_types::side::Side;
use crate::common_types::time_in_force::TIF;
use crate::config::AlgoParameters;
use crate::feed::feed_service::FeedService;
use crate::feed::messages::l1_data::L1Data;
use crate::feed::messages::l2_data::L2Data;
use crate::feed::messages::symbol_information::SymbolInformation;
use crate::logging::algo_report::AlgoPdfLogger;
use crate::market::market_service::MarketService;
use crate::market::messages::execution_type::ExecutionType;
use crate::market::messages::market_responses::MarketResponses;
use crate::{log_debug, log_error, log_info, logging, report};
use async_trait::async_trait;
use core::fmt;
use logging::algo_logger::AlgoLogger;
use rust_decimal::{prelude::Zero, Decimal};

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

#[allow(dead_code)]
#[derive(Debug)]
enum Event {
    SymbolInformation {
        min_quantity: Option<Decimal>,
        max_quantity: Option<Decimal>,
        lot_size: Option<Decimal>,
        min_price: Option<Decimal>,
        max_price: Option<Decimal>,
        tick_size: Option<Decimal>,
        min_amount: Option<Decimal>,
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
        order_type: OrderType,
        time_in_force: TIF,
    },
    CreateOrderRej {
        order_id: String,
        symbol: String,
        execution_status: ExecutionType,
        order_quantity: Decimal,
        price: Decimal,
        side: Side,
        order_type: OrderType,
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

pub struct SniperAlgo {
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
    async fn handle_l1(&mut self, l1_data: &L1Data) {
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

    async fn handle_l2(&mut self, l2_data: &L2Data) {
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
                min_amount,
            } => {
                self.on_event(Event::SymbolInformation {
                    min_quantity,
                    max_quantity,
                    lot_size,
                    min_price,
                    max_price,
                    tick_size,
                    min_amount,
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
                order_type,
                time_in_force,
            } => self.on_event(Event::CreateOrderAck {
                order_id,
                symbol,
                execution_status,
                order_quantity,
                price,
                side,
                order_type,
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
                order_type,
                rejection_reason,
                time_in_force,
            } => self.on_event(Event::CreateOrderRej {
                order_id,
                symbol,
                execution_status,
                order_quantity,
                price,
                side,
                order_type,
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
            logger: AlgoLogger::new(
                &algo_parameters.algo_type.to_string(),
                &algo_parameters.algo_id,
            ),
            algo_parameters: algo_parameters,
            market_sevice,
            feed_service,
            state: State::New,
            symbol_information: SymbolInformation::new(),
            remaining_quantity,
            executed_quantity: Decimal::zero(),
            exposed_quantity: Decimal::zero(),
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
                    min_amount,
                },
            ) => {
                self.symbol_information.set_values(
                    min_quantity,
                    max_quantity,
                    lot_size,
                    min_price,
                    max_price,
                    tick_size,
                    min_amount,
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

                    if let Some(min_price) = self.symbol_information.min_price {
                        if min_price >= price {
                            return;
                        }
                    }

                    if let Some(min_amount) = self.symbol_information.min_amount {
                        if min_amount >= price * quantity {
                            log_info!(
                                self.logger,
                                "onFeedUpdate",
                                "Available quantity {} at price {} does not meet the \
                                minimum amount {} required to send to the exchange.",
                                quantity,
                                price,
                                min_amount
                            );
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
                        self.algo_parameters.side,
                    );

                    self.market_sevice.create_ioc_order(
                        self.algo_parameters.make_symbol(),
                        price,
                        order_quantity,
                        &self.algo_parameters.side,
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
                    order_type,
                    time_in_force,
                },
            ) => {
                log_info!(
                    self.logger,
                    "CreateOrderAckEvent",
                    "Received Order Acknowledgment: Order ID {}, \
                    Symbol {}, Side {}, Time In Force {}, Quantity {}, Price {},OrderType {}, with Status {}",
                    order_id,
                    symbol,
                    side,
                    time_in_force,
                    order_quantity,
                    price,
                    order_type,
                    execution_status
                );

                report!(
                    self.pdf_report,
                    "Algorithm exposed a quantity of {} for Symbol {} at Price {} \
                    with Time In Force {} and Order Type {}",
                    order_quantity,
                    symbol,
                    price,
                    time_in_force,
                    order_type
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
                    order_type,
                    rejection_reason,
                    time_in_force,
                },
            ) => {
                log_info!(
                    self.logger,
                    "CreateOrderRejEvent",
                    "Received a creation rejection while attempting \
                    to place an order: Order ID {}, Symbol {}, Side {}, \
                    Quantity {}, Price {}, Status {}, Oreder Type {} and Rejection Reason {}.",
                    order_id,
                    symbol,
                    side,
                    order_quantity,
                    price,
                    execution_status,
                    order_type,
                    rejection_reason
                );

                report!(
                    self.pdf_report,
                    "The algorithm attempted to expose a quantity of {} for Symbol {} \
                    at Price {} with Time In Force {} and Order Type {}, but the order was rejected due to {}.",
                    order_quantity,
                    symbol,
                    price,
                    time_in_force,
                    order_type,
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

                self.executed_quantity += &executed_quantity;
                self.exposed_quantity -= &executed_quantity;

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
                        &self.algo_parameters.base,
                        &self.algo_parameters.quote,
                    );
                    self.pdf_report.write_to_pdf().unwrap();
                } else {
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

                self.exposed_quantity -= &leaves_quantity;
                self.remaining_quantity += &leaves_quantity;

                report!(
                    self.pdf_report,
                    "An order for symbol {} has expired on the exchange with execution status {}. \
                    During its lifespan, the order executed a quantity of {} \
                    and left an unexecuted quantity of {} that has been revoked. \
                    The algorithm has a remaining quantity of {} \
                    and a cumulative executed quantity of {} until now.",
                    symbol,
                    execution_status,
                    cumulative_quantity,
                    leaves_quantity,
                    self.remaining_quantity,
                    self.executed_quantity
                );

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
