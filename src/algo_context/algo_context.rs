use super::messages::messages::AlgoMessages;
use crate::algorithams::algorithm::Algorithm;
use crate::algorithams::sniper_algo::SniperAlgo;
use crate::common_types::algo_type::AlgoType;
use crate::config::AlgoParameters;
use crate::{
    feed::{
        self,
        actor::{FeedService, FeedUpdate},
    },
    market::market::{MarketResponses, MarketService, MarketSessionHandle},
};
use feed::actor::FeedHandle;
use futures::future::join_all;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::spawn;
use tokio::sync::{mpsc, Mutex};
use uuid::Uuid;

pub(super) struct AlgoContext {
    algo_context_id: String,
    algo_messages_receiver: mpsc::Receiver<AlgoMessages>,
    feed_sender: mpsc::Sender<FeedUpdate>,
    feed_receiver: mpsc::Receiver<FeedUpdate>,
    market_sender: mpsc::Sender<MarketResponses>,
    market_receiver: mpsc::Receiver<MarketResponses>,
    algorithams: HashMap<String, Arc<Mutex<Box<dyn Algorithm + Send>>>>,
    feed_hadnle: FeedHandle,
    market_session_handle: MarketSessionHandle,
}

impl AlgoContext {
    pub fn new(
        algo_messages_receiver: mpsc::Receiver<AlgoMessages>,
        feed_hadnle: FeedHandle,
        market_session_handle: MarketSessionHandle,
    ) -> Self {
        let (feed_sender, feed_receiver) = mpsc::channel(10);
        let (market_sender, market_receiver) = mpsc::channel(10);

        let algo_context_id = Uuid::new_v4().to_string();

        Self {
            algo_context_id,
            algo_messages_receiver,
            feed_sender,
            feed_receiver,
            market_sender,
            market_receiver,
            algorithams: HashMap::new(),
            feed_hadnle,
            market_session_handle,
        }
    }

    pub fn create_algo(&mut self, algo_parameters: AlgoParameters) {
        let market_service = MarketService::new(
            self.market_session_handle.clone(),
            self.market_sender.clone(),
            algo_parameters.algo_id.clone(),
        );

        let feed_service = FeedService::new(
            self.feed_hadnle.clone(),
            self.algo_context_id.clone(),
            algo_parameters.algo_id.clone(),
            self.feed_sender.clone(),
        );

        match algo_parameters.algo_type {
            AlgoType::Sniper => {
                let algo = SniperAlgo::new(algo_parameters.clone(), market_service, feed_service);
                self.algorithams.insert(
                    algo_parameters.algo_id.clone(),
                    Arc::new(Mutex::new(Box::new(algo))),
                );
            }
        }
    }

    pub fn handle_algo_message(&mut self, algo_message: AlgoMessages) {
        match algo_message {
            AlgoMessages::CreateAlgo(params) => {
                self.create_algo(params);
            }
        }
    }

    pub async fn handle_feed_update(&mut self, feed_update: FeedUpdate) {
        match feed_update {
            FeedUpdate::L1Update(algo_ids, l1_data) => {
                let l1_data = Arc::new(l1_data);
                let futures: Vec<_> = algo_ids
                    .into_iter()
                    .filter_map(|algo_id| self.algorithams.get(&algo_id).cloned())
                    .map(|algo| {
                        let l1_data = Arc::clone(&l1_data);
                        spawn(async move {
                            let mut algo = algo.lock().await;
                            algo.handle_l1(&l1_data).await;
                        })
                    })
                    .collect();

                join_all(futures).await;
            }
            FeedUpdate::L2Update(algo_ids, l2_data) => {
                let l2_data = Arc::new(l2_data);
                let futures: Vec<_> = algo_ids
                    .into_iter()
                    .filter_map(|algo_id| self.algorithams.get(&algo_id).cloned())
                    .map(|algo| {
                        let l2_data = Arc::clone(&l2_data);
                        spawn(async move {
                            let mut algo = algo.lock().await;
                            algo.handle_l2(&l2_data).await;
                        })
                    })
                    .collect();

                join_all(futures).await;
            }
        }
    }

    pub async fn handle_market_reponse(&mut self, market_response: MarketResponses) {
        let algo_id = market_response.algo_id();

        if let Some(algo) = self.algorithams.get_mut(algo_id) {
            let mut algo = algo.lock().await;
            algo.handle_market_reponse(market_response);
        }
    }
}

pub(super) async fn run_my_actor(mut actor: AlgoContext) {
    loop {
        tokio::select! {
            Some(algo_message) = actor.algo_messages_receiver.recv() => {
               actor.handle_algo_message(algo_message);
            },
            Some(feed_update) = actor.feed_receiver.recv() => {
                actor.handle_feed_update(feed_update).await;
            },
            Some(market_response) = actor.market_receiver.recv() => {
                actor.handle_market_reponse(market_response).await;
            },
            else => break,
        }
    }
}
