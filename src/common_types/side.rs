use serde::Deserialize;
use std::fmt::{self, Display};

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
