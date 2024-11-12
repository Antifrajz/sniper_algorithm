use core::fmt;

use ::serde::Deserialize;
use async_trait::async_trait;

use crate::{
    feed::actor::{L1Data, L2Data},
    market::market::MarketResponses,
};

#[async_trait]
pub trait Algorithm {
    async fn handle_l1(&mut self, l1_data: &L1Data);
    async fn handle_l2(&mut self, l2_data: &L2Data);
    fn handle_market_reponse(&mut self, market_response: MarketResponses);
}

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
