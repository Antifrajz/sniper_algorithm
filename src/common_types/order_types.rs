use core::fmt;
use std::{fmt::Display, str::FromStr};

#[derive(Debug)]
pub enum OrderType {
    Limit,
    Market,
    StopLossLimit,
}

impl OrderType {
    pub fn from_int(value: i32) -> Option<Self> {
        match value {
            1 => Some(OrderType::Limit),
            2 => Some(OrderType::Market),
            3 => Some(OrderType::StopLossLimit),
            _ => None,
        }
    }

    pub fn to_int(&self) -> i32 {
        match self {
            OrderType::Limit => 1,
            OrderType::Market => 2,
            OrderType::StopLossLimit => 3,
        }
    }
}

impl Display for OrderType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Limit => write!(f, "LIMIT"),
            Self::Market => write!(f, "MARKET"),
            Self::StopLossLimit => write!(f, "STOP_LOSS_LIMIT"),
        }
    }
}

impl FromStr for OrderType {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "LIMIT" => Ok(OrderType::Limit),
            "MARKET" => Ok(OrderType::Market),
            "STOP_LOSS_LIMIT" => Ok(OrderType::StopLossLimit),
            _ => Err(()),
        }
    }
}
