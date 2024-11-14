use super::{
    market::MarketActor,
    messages::{market_messages::MarketMessages, market_responses::MarketResponses},
};
use crate::{
    common_types::{order_types::OrderType, side::Side, time_in_force::TIF},
    config::MarketConfig,
};
use rust_decimal::Decimal;
use tokio::{sync::mpsc, task::JoinHandle};

#[derive(Clone)]

pub struct MarketHandle {
    sender: mpsc::Sender<MarketMessages>,
}

impl MarketHandle {
    pub async fn new(market_config: MarketConfig) -> (Self, JoinHandle<()>) {
        let (sender, receiver) = mpsc::channel(100);

        let actor = MarketActor::new(market_config, receiver).await;

        let handle = tokio::spawn(super::market::run_my_actor(actor));

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
            .unwrap_or_else(|err| eprintln!("Failed to send message: {:?}", err));
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
            .unwrap_or_else(|err| eprintln!("Failed to send message: {:?}", err));
    }
}
