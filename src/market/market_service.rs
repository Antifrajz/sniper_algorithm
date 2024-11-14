use super::{market_handle::MarketHandle, messages::market_responses::MarketResponses};
use crate::common_types::{order_types::OrderType, side::Side, time_in_force::TIF};
use rust_decimal::Decimal;
use tokio::sync::mpsc;
use uuid::Uuid;

#[derive(Clone)]
pub struct MarketService {
    market_handle: MarketHandle,
    meesage_sender: mpsc::Sender<MarketResponses>,
    algo_id: String,
}

impl MarketService {
    pub fn new<AlgoId>(
        market_handle: &MarketHandle,
        meesage_sender: &mpsc::Sender<MarketResponses>,
        algo_id: AlgoId,
    ) -> Self
    where
        AlgoId: Into<String>,
    {
        Self {
            market_handle: market_handle.clone(),
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

        self.market_handle.create_order(
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
        self.market_handle.get_symbol_info(
            symbol.into(),
            self.algo_id.clone(),
            self.meesage_sender.clone(),
        );
    }
}
