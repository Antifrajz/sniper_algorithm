use ::serde::Deserialize;
use std::fmt;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AlgoType {
    Sniper,
}

impl fmt::Display for AlgoType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AlgoType::Sniper => write!(f, "Sniper"),
        }
    }
}

impl AlgoType {
    pub fn from_string(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "sniper" => Some(AlgoType::Sniper),
            _ => None,
        }
    }

    pub fn to_string(&self) -> &'static str {
        match self {
            AlgoType::Sniper => "Sniper",
        }
    }
}
