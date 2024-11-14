use super::level::Level;
use core::fmt;
use rust_decimal::Decimal;

#[derive(Debug, Clone)]
pub struct L1Data {
    pub symbol: String,
    pub best_bid_level: Level,
    pub best_ask_level: Level,
}

impl L1Data {
    pub fn new<Symbol>(
        symbol: Symbol,
        best_bid_quantity: Decimal,
        best_bid_price: Decimal,
        best_ask_quantity: Decimal,
        best_ask_price: Decimal,
    ) -> Self
    where
        Symbol: Into<String>,
    {
        L1Data {
            symbol: symbol.into(),
            best_bid_level: Level::new(1, best_bid_quantity, best_bid_price),
            best_ask_level: Level::new(1, best_ask_quantity, best_ask_price),
        }
    }
}

impl fmt::Display for L1Data {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "L1Data {{ symbol: {}, best_bid: {}, best_ask: {} }}",
            self.symbol, self.best_bid_level, self.best_ask_level
        )
    }
}
