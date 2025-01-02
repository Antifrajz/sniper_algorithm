use crate::{
    feed::messages::{l1_data::L1Data, l2_data::L2Data},
    market::messages::market_responses::MarketResponses,
};

pub trait Algorithm {
    fn handle_l1(&mut self, l1_data: &L1Data);
    fn handle_l2(&mut self, l2_data: &L2Data);
    fn handle_market_reponse(&mut self, market_response: MarketResponses);
}
