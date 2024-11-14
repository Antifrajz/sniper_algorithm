use super::algo_context::{run_my_actor, AlgoContext};
use super::messages::messages::AlgoMessages;
use crate::config::AlgoParameters;
use crate::feed::feed_handle::FeedHandle;
use crate::market::market::MarketSessionHandle;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

pub struct AlgoService {
    sender: mpsc::Sender<AlgoMessages>,
}

impl AlgoService {
    pub fn new(
        feed_handle: FeedHandle,
        market_session_handle: MarketSessionHandle,
    ) -> (Self, JoinHandle<()>) {
        let (sender, receiver) = mpsc::channel(10);

        let actor = AlgoContext::new(receiver, feed_handle, market_session_handle);

        let handle = tokio::spawn(run_my_actor(actor));

        (Self { sender }, handle)
    }

    pub fn create_algo(&self, params: AlgoParameters) {
        self.sender
            .try_send(AlgoMessages::CreateAlgo(params))
            .unwrap_or_else(|e| {
                eprintln!(
                    "Failed to send Create Algo message to Algo Context: {:?}",
                    e
                );
            });
    }
}
