use super::messages::messages::AlgoMessages;
use crate::algorithams::algorithm::Algorithm;
use crate::algorithams::sniper_algo::SniperAlgo;
use crate::common_types::algo_type::AlgoType;
use crate::config::AlgoParameters;
use crate::feed::feed_handle::FeedHandle;
use crate::feed::feed_service::FeedService;
use crate::feed::messages::messages::FeedUpdate;
use crate::market::market_handle::MarketHandle;
use crate::market::market_service::MarketService;
use crate::market::messages::market_responses::MarketResponses;
use probe::probe_lazy;
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use uuid::Uuid;

macro_rules! probe {
    ($name:ident) => {
        probe_lazy!(l1_updates, $name, { std::ptr::null::<()>() })
    };
}

pub(super) struct AlgoContext {
    algo_context_id: String,
    algo_messages_receiver: mpsc::Receiver<AlgoMessages>,
    feed_sender: mpsc::Sender<FeedUpdate>,
    feed_receiver: mpsc::Receiver<FeedUpdate>,
    market_sender: mpsc::Sender<MarketResponses>,
    market_receiver: mpsc::Receiver<MarketResponses>,
    algorithams: HashMap<String, Arc<Mutex<Box<dyn Algorithm + Send>>>>,
    feed_hadnle: FeedHandle,
    market_handle: MarketHandle,
}

impl AlgoContext {
    pub fn new(
        algo_messages_receiver: mpsc::Receiver<AlgoMessages>,
        feed_hadnle: FeedHandle,
        market_handle: MarketHandle,
    ) -> Self {
        let (feed_sender, feed_receiver) = mpsc::channel(1000);
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
            market_handle,
        }
    }

    pub fn create_algo(&mut self, algo_parameters: AlgoParameters) {
        let algo_id = algo_parameters.algo_id.clone();

        let market_service = MarketService::new(&self.market_handle, &self.market_sender, &algo_id);

        let feed_service = FeedService::new(
            &self.feed_hadnle,
            &self.algo_context_id,
            &algo_id,
            &self.feed_sender,
        );

        match algo_parameters.algo_type {
            AlgoType::Sniper => {
                let algo = SniperAlgo::new(algo_parameters, market_service, feed_service);
                self.algorithams
                    .insert(algo_id, Arc::new(Mutex::new(Box::new(algo))));
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

    pub fn handle_feed_update(&mut self, feed_update: FeedUpdate) {
        match feed_update {
            FeedUpdate::L1Update(algo_ids, l1_data) => {
                probe!(start_processing_l1_update);
                algo_ids
                    .into_par_iter()
                    .filter_map(|algo_id| self.algorithams.get(&algo_id).cloned())
                    .for_each(move |algo| {
                        let mut algo = algo.lock().unwrap();
                        algo.handle_l1(&l1_data);
                    });
                probe!(finish_processing_l1_update);
            }
            FeedUpdate::L2Update(algo_ids, l2_data) => {
                algo_ids
                    .into_par_iter()
                    .filter_map(|algo_id| self.algorithams.get(&algo_id).cloned())
                    .for_each(move |algo| {
                        let mut algo = algo.lock().unwrap();
                        algo.handle_l2(&l2_data);
                    });
            }
        }
    }

    pub fn handle_market_reponse(&mut self, market_response: MarketResponses) {
        let algo_id = market_response.algo_id();

        if let Some(algo) = self.algorithams.get_mut(algo_id) {
            let mut algo = algo.lock().unwrap();
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
                actor.handle_feed_update(feed_update);
            },
            Some(market_response) = actor.market_receiver.recv() => {
                actor.handle_market_reponse(market_response);
            },
            else => break,
        }
    }
}
