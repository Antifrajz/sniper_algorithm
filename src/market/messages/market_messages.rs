use super::market_responses::MarketResponses;
use crate::common_types::{order_types::OrderType, side::Side, time_in_force::TIF};
use rust_decimal::Decimal;
use tokio::sync::mpsc;

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
