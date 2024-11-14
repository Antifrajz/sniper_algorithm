use super::level::Level;
use core::fmt;

#[derive(Debug, Clone)]

pub struct L2Data {
    pub symbol: String,
    pub bid_side_levels: Vec<Level>,
    pub ask_side_levels: Vec<Level>,
}

impl L2Data {
    pub fn new<Symbol>(
        symbol: Symbol,
        bid_side_levels: Vec<Level>,
        ask_side_levels: Vec<Level>,
    ) -> Self
    where
        Symbol: Into<String>,
    {
        L2Data {
            symbol: symbol.into(),
            bid_side_levels,
            ask_side_levels,
        }
    }
}

impl fmt::Display for L2Data {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "L2Data {{ symbol: {}, bid_side_levels: [", self.symbol)?;
        for (i, level) in self.bid_side_levels.iter().enumerate() {
            if i > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{}", level)?;
        }
        write!(f, "], ask_side_levels: [")?;
        for (i, level) in self.ask_side_levels.iter().enumerate() {
            if i > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{}", level)?;
        }
        write!(f, "] }}")
    }
}
