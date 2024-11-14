use core::fmt;

#[derive(Debug)]
pub enum ExecutionType {
    New,
    Canceled,
    Replaced,
    Rejected,
    Trade,
    Expired,
    TradePrevention,
    Unknown,
}

impl ExecutionType {
    pub fn from_str(input: &str) -> Self {
        match input {
            "NEW" => ExecutionType::New,
            "CANCELED" => ExecutionType::Canceled,
            "REPLACED" => ExecutionType::Replaced,
            "REJECTED" => ExecutionType::Rejected,
            "TRADE" => ExecutionType::Trade,
            "EXPIRED" => ExecutionType::Expired,
            "TRADE_PREVENTION" => ExecutionType::TradePrevention,
            _ => ExecutionType::Unknown,
        }
    }
}

impl fmt::Display for ExecutionType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let variant_str = match self {
            ExecutionType::New => "NEW",
            ExecutionType::Canceled => "CANCELED",
            ExecutionType::Replaced => "REPLACED",
            ExecutionType::Rejected => "REJECTED",
            ExecutionType::Trade => "TRADE",
            ExecutionType::Expired => "EXPIRED",
            ExecutionType::TradePrevention => "TRADE_PREVENTION",
            ExecutionType::Unknown => "UNKNOWN",
        };
        write!(f, "{}", variant_str)
    }
}
