use super::execution_type::ExecutionType;
use crate::common_types::{order_types::OrderType, side::Side, time_in_force::TIF};
use core::fmt;
use rust_decimal::Decimal;

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
            Some(ref v) => format!("{}", v),
            None => "None".to_string(),
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
