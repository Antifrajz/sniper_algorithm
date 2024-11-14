use dotenv::dotenv;
use rust_decimal::Decimal;
use serde::Deserialize;
use std::collections::HashSet;
use std::env;
use std::fs;

use crate::common_types::algo_type::AlgoType;
use crate::common_types::side::Side;

#[derive(Deserialize, Debug, Clone)]

pub struct AlgoParameters {
    pub base: String,
    pub quote: String,
    pub algo_type: AlgoType,
    pub algo_id: String,
    pub side: Side,
    pub quantity: Decimal,
    pub price: Decimal,
}

impl AlgoParameters {
    pub fn make_symbol(&self) -> String {
        format!("{}{}", self.base, self.quote).to_uppercase()
    }

    pub fn get_trading_pair(&self) -> (&str, &str) {
        (&self.base, &self.quote)
    }
}

#[derive(Deserialize, Debug, Default)]
pub struct AlgorithmConfig {
    pub algorithms: Vec<AlgoParameters>,
}

impl AlgorithmConfig {
    pub fn load_from_file(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let config_content = fs::read_to_string(path)?;
        let config: AlgorithmConfig = toml::from_str(&config_content)?;
        Ok(config)
    }
}

#[derive(Debug, Deserialize)]
pub struct MarketConfig {
    pub api_key: String,
    pub api_secret: String,
}

impl MarketConfig {
    pub fn from_env() -> Result<Self, Box<dyn std::error::Error>> {
        dotenv().ok();

        let api_key = env::var("API_KEY")?;
        let api_secret = env::var("API_SECRET")?;

        Ok(Self {
            api_key,
            api_secret,
        })
    }
}

pub fn extract_trading_pairs(params: &[AlgoParameters]) -> HashSet<(&str, &str)> {
    let mut trading_pairs: HashSet<(&str, &str)> = HashSet::new();

    for param in params {
        trading_pairs.insert(param.get_trading_pair());
    }

    trading_pairs
}
