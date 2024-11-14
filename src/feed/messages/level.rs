use core::fmt;
use rust_decimal::Decimal;

#[derive(Debug, Clone)]
pub struct Level {
    pub level: i32,
    pub quantity: Decimal,
    pub price: Decimal,
}

impl Level {
    pub fn new(level: i32, quantity: Decimal, price: Decimal) -> Self {
        Level {
            level: level,
            quantity: quantity,
            price: price,
        }
    }
}

impl fmt::Display for Level {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Level: {} {{quantity: {:.6}, price: {:.2} }}",
            self.level, self.quantity, self.price
        )
    }
}
