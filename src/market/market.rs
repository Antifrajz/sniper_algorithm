use super::messages::{
    execution_type::ExecutionType, market_messages::MarketMessages,
    market_responses::MarketResponses,
};
use crate::{
    common_types::{order_types::OrderType, side::Side, time_in_force::TIF},
    config::MarketConfig,
};
use binance::{
    account::{Account, OrderSide, OrderType as BinanceOrderType, TimeInForce},
    general::General,
    model::Filters,
    websockets::*,
};
use binance::{api::*, config::Config};
use binance::{model::OrderTradeEvent, userstream::*};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::{prelude::Zero, Decimal};
use std::sync::atomic::AtomicBool;
use std::{collections::HashMap, str::FromStr, sync::Arc};
use tokio::{
    sync::mpsc,
    task::{self},
};

pub(super) struct MarketActor {
    receiver: mpsc::Receiver<MarketMessages>,
    account: Account,
    general: General,
    market_config: MarketConfig,
    algo_contexts: Arc<std::sync::Mutex<HashMap<String, (String, mpsc::Sender<MarketResponses>)>>>,
}

impl MarketActor {
    pub async fn new(
        market_config: MarketConfig,
        receiver: mpsc::Receiver<MarketMessages>,
    ) -> Self {
        let api_key = Some(market_config.api_key.clone());
        let api_secret = Some(market_config.api_secret.clone());

        let result = task::spawn_blocking(move || {
            let config = Config::testnet();
            let user_stream: Account =
                Binance::new_with_config(api_key.clone(), api_secret.clone(), &config);

            let general: General = Binance::new_with_config(api_key, api_secret, &config);

            (user_stream, general)
        })
        .await;

        let (account, general) = result.unwrap();

        Self {
            receiver,
            account: account,
            general: general,
            market_config,
            algo_contexts: Arc::new(std::sync::Mutex::new(HashMap::new())),
        }
    }

    pub async fn handle(&mut self, market_message: MarketMessages) {
        match market_message {
            MarketMessages::GetSymbolInformation {
                symbol,
                algo_id,
                sender,
            } => {
                let general = self.general.clone();

                task::spawn_blocking(move || match general.get_symbol_info(symbol) {
                    Ok(answer) => {
                        println!("Symbol info siu {:?}", answer);

                        let (mut min_qty, mut max_qty, mut lot_size) = (None, None, None);
                        let (mut min_price, mut max_price, mut tick_size) = (None, None, None);
                        let mut min_amount = None;

                        for filter in &answer.filters {
                            match filter {
                                Filters::LotSize {
                                    min_qty: mq,
                                    max_qty: mxq,
                                    step_size,
                                } => {
                                    min_qty = mq.parse::<Decimal>().ok();
                                    max_qty = mxq.parse::<Decimal>().ok();
                                    lot_size = step_size.parse::<Decimal>().ok();
                                }
                                Filters::PriceFilter {
                                    min_price: mp,
                                    max_price: mxp,
                                    tick_size: ts,
                                } => {
                                    min_price = mp.parse::<Decimal>().ok();
                                    max_price = mxp.parse::<Decimal>().ok();
                                    tick_size = ts.parse::<Decimal>().ok();
                                }
                                Filters::Notional {
                                    notional: _,
                                    min_notional,
                                    apply_to_market: _,
                                    avg_price_mins: _,
                                } => {
                                    min_amount = min_notional
                                        .as_ref()
                                        .and_then(|notional| notional.parse::<Decimal>().ok());
                                }
                                _ => {}
                            }
                        }

                        sender
                            .try_send(MarketResponses::SymbolInformation {
                                algo_id,
                                min_quantity: min_qty,
                                max_quantity: max_qty,
                                lot_size,
                                min_price,
                                max_price,
                                tick_size,
                                min_amount,
                            })
                            .unwrap_or_else(|err| eprintln!("Failed to send message: {:?}", err));
                    }
                    Err(e) => {
                        eprintln!("Didn't receive symbol info: Error: {}", e);
                        sender
                            .try_send(MarketResponses::SymbolInformation {
                                algo_id,
                                min_quantity: None,
                                max_quantity: None,
                                lot_size: None,
                                min_price: None,
                                max_price: None,
                                tick_size: None,
                                min_amount: None,
                            })
                            .unwrap_or_else(|err| eprintln!("Failed to send message: {:?}", err));
                    }
                });
            }

            MarketMessages::CreateOrder {
                symbol,
                price,
                quantity,
                side,
                order_type,
                time_in_force,
                sender,
                order_id,
                algo_id,
            } => {
                let account_clone = self.account.clone();
                let order_side = OrderSide::from_int(side.to_int()).unwrap_or(OrderSide::Buy);
                let time_in_force_binance =
                    TimeInForce::from_int(time_in_force.to_int()).unwrap_or(TimeInForce::GTC);
                let order_type_binance = BinanceOrderType::from_int(order_type.to_int())
                    .unwrap_or(BinanceOrderType::Limit);
                let mut algo_contexts = self.algo_contexts.lock().unwrap();
                algo_contexts.insert(order_id.clone(), (algo_id.clone(), sender.clone()));
                task::spawn_blocking(move || {
                    match account_clone.custom_order(
                        symbol.clone(),
                        quantity.clone().to_f64().unwrap_or(0.),
                        price.clone().to_f64().unwrap_or(0.),
                        None,
                        order_side,
                        order_type_binance,
                        time_in_force_binance,
                        Some(order_id.clone()),
                    ) {
                        Ok(_) => {}
                        Err(e) => {
                            println!("Error: {}", e);
                            sender
                                .try_send(MarketResponses::OrderRejected {
                                    order_id,
                                    algo_id,
                                    symbol,
                                    execution_status: ExecutionType::Rejected,
                                    order_quantity: quantity,
                                    side,
                                    order_type,
                                    price,
                                    time_in_force,
                                    rejection_reason: e.to_string(),
                                })
                                .unwrap_or_else(|err| {
                                    eprintln!("Failed to send message: {:?}", err)
                                });
                        }
                    }
                });
            }
        }
    }
}

pub(super) async fn run_my_actor(mut actor: MarketActor) {
    let algo_contexts = actor.algo_contexts.clone();

    let api_key = Some(actor.market_config.api_key.clone());
    let api_secret = Some(actor.market_config.api_secret.clone());

    task::spawn_blocking(move || {
        let keep_running = AtomicBool::new(true);
        let config = Config::testnet();
        let user_stream: UserStream = Binance::new_with_config(api_key, api_secret, &config);

        if let Ok(answer) = user_stream.start() {
            let listen_key = answer.listen_key;

            let mut web_socket: WebSockets<'_> = WebSockets::new(|event: WebsocketEvent| {
                match event {
                    WebsocketEvent::OrderTrade(trade) => {
                        let algo_contexts = algo_contexts.lock().unwrap();
                        match algo_contexts.get(&trade.new_client_order_id) {
                            Some((algo_id, algo_context)) => {
                                handle_order_trade_event(algo_context, algo_id, &trade);
                            }
                            None => {
                                eprintln!(
                                    "No algorithm context found for new_client_order_id: {}",
                                    trade.new_client_order_id
                                );
                            }
                        }
                    }
                    _ => (),
                };

                Ok(())
            });

            web_socket
                .connect_with_config(&listen_key, &config)
                .unwrap();
            if let Err(e) = web_socket.event_loop(&keep_running) {
                println!("Error: {}", e);
            }
            user_stream.close(&listen_key).unwrap();
            web_socket.disconnect().unwrap();
            println!("Userstrem closed and disconnected");
        }
    });

    while let Some(msg) = actor.receiver.recv().await {
        actor.handle(msg).await;
    }
}

fn handle_order_trade_event(
    algo: &mpsc::Sender<MarketResponses>,
    algo_id: &String,
    event: &OrderTradeEvent,
) {
    let execution_type = ExecutionType::from_str(&event.execution_type);

    match execution_type {
        ExecutionType::New => {
            algo.try_send(MarketResponses::CreateOrderAck {
                order_id: event.new_client_order_id.clone(),
                algo_id: algo_id.clone(),
                symbol: event.symbol.clone(),
                execution_status: execution_type,
                order_quantity: event.qty.parse::<Decimal>().unwrap_or(Decimal::zero()),
                side: Side::from_str(&event.side).unwrap_or(Side::Buy),
                order_type: OrderType::from_str(&event.order_type).unwrap_or(OrderType::Limit),
                price: event.price.parse::<Decimal>().unwrap_or(Decimal::ZERO),
                time_in_force: TIF::from_str(&event.time_in_force).unwrap_or(TIF::GTC),
            })
            .unwrap_or_else(|err| eprintln!("Failed to send message: {:?}", err));
        }
        ExecutionType::Trade => {
            if event.order_status == String::from("FILLED") {
                algo.try_send(MarketResponses::OrderFullyFilled {
                    order_id: event.new_client_order_id.clone(),
                    algo_id: algo_id.clone(),
                    symbol: event.symbol.clone(),
                    execution_status: execution_type,
                    quantity: event.qty.parse::<Decimal>().unwrap_or(Decimal::zero()),
                    fill_price: event
                        .price_last_filled_trade
                        .parse::<Decimal>()
                        .unwrap_or(Decimal::zero()),
                    side: Side::from_str(&event.side).unwrap_or(Side::Buy),
                    executed_quantity: event
                        .qty_last_filled_trade
                        .parse::<Decimal>()
                        .unwrap_or(Decimal::zero()),
                    cumulative_quantity: event
                        .accumulated_qty_filled_trades
                        .parse::<Decimal>()
                        .unwrap_or(Decimal::zero()),
                    leaves_quantity: event.qty.parse::<Decimal>().unwrap_or(Decimal::zero())
                        - event
                            .qty_last_filled_trade
                            .parse::<Decimal>()
                            .unwrap_or(Decimal::zero()),
                })
                .unwrap_or_else(|err| eprintln!("Failed to send message: {:?}", err));
            } else {
                algo.try_send(MarketResponses::OrderPartiallyFilled {
                    order_id: event.new_client_order_id.clone(),
                    algo_id: algo_id.clone(),
                    symbol: event.symbol.clone(),
                    execution_status: execution_type,
                    quantity: event.qty.parse::<Decimal>().unwrap_or(Decimal::zero()),
                    fill_price: event
                        .price_last_filled_trade
                        .parse::<Decimal>()
                        .unwrap_or(Decimal::zero()),
                    side: Side::from_str(&event.side).unwrap_or(Side::Buy),
                    executed_quantity: event
                        .qty_last_filled_trade
                        .parse::<Decimal>()
                        .unwrap_or(Decimal::zero()),
                    cumulative_quantity: event
                        .accumulated_qty_filled_trades
                        .parse::<Decimal>()
                        .unwrap_or(Decimal::zero()),
                    leaves_quantity: event.qty.parse::<Decimal>().unwrap_or(Decimal::zero())
                        - event
                            .qty_last_filled_trade
                            .parse::<Decimal>()
                            .unwrap_or(Decimal::zero()),
                })
                .unwrap_or_else(|err| eprintln!("Failed to send message: {:?}", err));
            }
        }
        ExecutionType::Expired => {
            algo.try_send(MarketResponses::OrderExpired {
                order_id: event.new_client_order_id.clone(),
                algo_id: algo_id.clone(),
                symbol: event.symbol.clone(),
                execution_status: execution_type,
                quantity: event.qty.parse::<Decimal>().unwrap_or(Decimal::zero()),
                side: Side::from_str(&event.side).unwrap_or(Side::Buy),
                executed_quantity: event
                    .qty_last_filled_trade
                    .parse::<Decimal>()
                    .unwrap_or(Decimal::ZERO),
                cumulative_quantity: event
                    .accumulated_qty_filled_trades
                    .parse::<Decimal>()
                    .unwrap_or(Decimal::zero()),
                leaves_quantity: event.qty.parse::<Decimal>().unwrap_or(Decimal::ZERO)
                    - event
                        .accumulated_qty_filled_trades
                        .parse::<Decimal>()
                        .unwrap_or(Decimal::ZERO),
            })
            .unwrap_or_else(|err| eprintln!("Failed to send message: {:?}", err));
        }
        ExecutionType::Rejected => {
            algo.try_send(MarketResponses::OrderRejected {
                order_id: event.new_client_order_id.clone(),
                algo_id: algo_id.clone(),
                symbol: event.symbol.clone(),
                execution_status: execution_type,
                order_quantity: event.qty.parse::<Decimal>().unwrap_or(Decimal::ZERO),
                side: Side::from_str(&event.side).unwrap_or(Side::Buy),
                order_type: OrderType::from_str(&event.order_type).unwrap_or(OrderType::Limit),
                price: event.price.parse::<Decimal>().unwrap_or(Decimal::ZERO),
                rejection_reason: event.order_reject_reason.clone(),
                time_in_force: TIF::from_str(&event.time_in_force).unwrap_or(TIF::GTC),
            })
            .unwrap_or_else(|err| eprintln!("Failed to send message: {:?}", err));
        }
        ExecutionType::Canceled => {
            algo.try_send(MarketResponses::OrderCanceled {
                order_id: event.new_client_order_id.clone(),
                algo_id: algo_id.clone(),
                symbol: event.symbol.clone(),
                execution_status: execution_type,
                quantity: event.qty.parse::<Decimal>().unwrap_or(Decimal::ZERO),
                side: Side::from_str(&event.side).unwrap(),
                executed_quantity: event.qty_last_filled_trade.parse::<Decimal>().unwrap(),
                cumulative_quantity: event
                    .accumulated_qty_filled_trades
                    .parse::<Decimal>()
                    .unwrap_or(Decimal::ZERO),
                leaves_quantity: event.qty.parse::<Decimal>().unwrap_or(Decimal::ZERO)
                    - event
                        .qty_last_filled_trade
                        .parse::<Decimal>()
                        .unwrap_or(Decimal::ZERO),
            })
            .unwrap_or_else(|err| eprintln!("Failed to send message: {:?}", err));
        }
        _ => (),
    }
}
