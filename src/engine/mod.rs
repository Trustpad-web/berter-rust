pub mod error;
pub mod trader;

use std::collections::HashMap;
use crate::Market;
use crate::engine::error::EngineError;
use crate::engine::trader::Trader;
use crate::data::handler::{Continuer, MarketGenerator};
use crate::strategy::SignalGenerator;
use crate::portfolio::repository::PositionHandler;
use crate::portfolio::{FillUpdater, MarketUpdater, OrderGenerator};
use crate::portfolio::position::Position;
use crate::execution::FillGenerator;
use crate::event::{Event, MessageTransmitter};
use std::fmt::Debug;
use std::sync::{Mutex, Arc};
use std::thread;
use serde::Serialize;
use tokio::sync::{mpsc, oneshot};
use tracing::{info, warn, error};
use uuid::Uuid;
use crate::statistic::summary::trading::TradingSummary;

// Todo - Important:
//  - Roll out consistent use of Market / Exchange / symbol (new types?)
//    '--> Remember (can't use Market instead of Exchange & Symbol for Position due to serde)
//    '--> eg/ portfolio.get_statistics(&self.market.market_id()) -> could market_id() return a ref?
//  - Search for to dos since I found one in /statistic/summary/pnl.rs
//  - Search for unwraps() & fix

// Todo - After Important:
//  - Print summary for each Market, rather than as a total
//  - Add unit test cases for update_from_fill tests (4 of them) which use get & set stats
//  - Write unit tests for Portfolio's new functionality - metrics, etc, etc
//  - Ensure I am eagerly deriving as much as possible - especially enums! Work out the base derive
//  - Extract Portfolio::init util functions to remove code dups? perhaps (fn bootstrap_repository() or similar)
//  - Impl consistent structured logging in Engine & Trader
//   '--> Do I want to spans instead of multiple info logging? eg/ fetch_open_requests logs twice
//   '--> Where do I want to log things like Command::ExitPosition being actioned? In Engine or when we push SignalForceExit on to Q?
//  - Ensure i'm happy with where event Event & Command live (eg/ Balance is in event.rs)
//  - Do I want ad-hoc way to send a SummarySnapshot on top of Event::Metric being emitted all the time?
//     '--> Traders could cache the last metrics for ease (seems dirty?).

// Todo - 0.7.1:
//  - Add Deserialize to Event.
//  - Make as much stuff Copy as can be - start in Statistics!
//  - Cleanup Config passing - seems like there is duplication eg/ Portfolio.starting_cash vs Portfolio.stats_config.starting_equity
//     '--> also can use references to markets to avoid cloning?
//  - If happy with it, impl Initialiser for all stats across the Statistics module.

/// Communicates a String is a message associated with a [`Command`].
pub type Message = String;

#[derive(Debug)]
pub enum Command {
    FetchOpenPositions(oneshot::Sender<Result<Vec<Position>, EngineError>>), // Engine
    // SendSummary(oneshot::Sender<Result<TradingSummary, EngineError>>),    // Engine
    Terminate(Message),                                                      // All Traders
    ExitAllPositions,                                                        // All Traders
    ExitPosition(Market),                                                    // Single Trader
}

/// Lego components for constructing an [`Engine`] via the new() constructor method.
#[derive(Debug)]
pub struct EngineLego<EventTx, Statistic, Portfolio, Data, Strategy, Execution>
where
    EventTx: MessageTransmitter<Event<Statistic>>  + Send,
    Statistic: Serialize + Send,
    Portfolio: MarketUpdater + OrderGenerator + FillUpdater<Statistic> + Send,
    Data: Continuer + MarketGenerator + Send,
    Strategy: SignalGenerator + Send,
    Execution: FillGenerator + Send,
{
    /// Unique identifier for an [`Engine`] in Uuid v4 format. Used as a unique identifier seed for
    /// the Portfolio, Trader & Positions associated with this [`Engine`].
    pub engine_id: Uuid,
    /// mpsc::Receiver for receiving [`Command`]s from a remote source.
    pub command_rx: mpsc::Receiver<Command>,
    /// Shared-access to a global Portfolio instance.
    pub portfolio: Arc<Mutex<Portfolio>>,
    /// Collection of [`Trader`] instances that can concurrently trade a market pair on it's own thread.
    pub traders: Vec<Trader<EventTx, Statistic, Portfolio, Data, Strategy, Execution>>,
    /// Todo:
    pub trader_command_txs: HashMap<Market, mpsc::Sender<Command>>,
}

/// Multi-threaded Trading Engine capable of trading with an arbitrary number of [`Trader`] market
/// pairs. Each [`Trader`] operates on it's own thread and has it's own Data Handler, Strategy &
/// Execution Handler, as well as shared access to a global Portfolio instance. A graceful remote
/// shutdown is made possible by sending a [`Message`] to the Engine's broadcast::Receiver
/// termination_rx.
#[derive(Debug)]
pub struct Engine<EventTx, Statistic, Portfolio, Data, Strategy, Execution>
where
    EventTx: MessageTransmitter<Event<Statistic>>,
    Statistic: Serialize + Send,
    Portfolio: PositionHandler + MarketUpdater + OrderGenerator + FillUpdater<Statistic> + Send,
    Data: Continuer + MarketGenerator + Send + 'static,
    Strategy: SignalGenerator + Send,
    Execution: FillGenerator + Send,
{
    /// Unique identifier for an [`Engine`] in Uuid v4 format. Used as a unique identifier seed for
    /// the Portfolio, Trader & Positions associated with this [`Engine`].
    engine_id: Uuid,
    /// mpsc::Receiver for receiving [`Command`]s from a remote source.
    command_rx: mpsc::Receiver<Command>,
    /// Shared-access to a global Portfolio instance that implements [`MarketUpdater`],
    /// [`OrderGenerator`] & [`FillUpdater`].
    portfolio: Arc<Mutex<Portfolio>>,
    /// Collection of [`Trader`] instances that can concurrently trade a market pair on it's own thread.
    traders: Vec<Trader<EventTx, Statistic, Portfolio, Data, Strategy, Execution>>,
    /// Todo:
    trader_command_txs: HashMap<Market, mpsc::Sender<Command>>,
}

impl<EventTx, Statistic, Portfolio, Data, Strategy, Execution> Engine<EventTx, Statistic, Portfolio, Data, Strategy, Execution>
where
    EventTx: MessageTransmitter<Event<Statistic>>  + Send + 'static,
    Statistic: Serialize + Send + 'static,
    Portfolio: PositionHandler + MarketUpdater + OrderGenerator + FillUpdater<Statistic> + Send + 'static,
    Data: Continuer + MarketGenerator + Send,
    Strategy: SignalGenerator + Send + 'static,
    Execution: FillGenerator + Send + 'static,
{
    /// Constructs a new trading [`Engine`] instance using the provided [`EngineLego`].
    pub fn new(lego: EngineLego<EventTx, Statistic, Portfolio, Data, Strategy, Execution>) -> Self {
        info!(engine_id = &*format!("{}", lego.engine_id), "constructed new Engine instance");
        Self {
            engine_id: lego.engine_id,
            command_rx: lego.command_rx,
            portfolio: lego.portfolio,
            traders: lego.traders,
            trader_command_txs: lego.trader_command_txs
        }
    }

    /// Builder to construct [`Engine`] instances.
    pub fn builder() -> EngineBuilder<EventTx, Statistic, Portfolio, Data, Strategy, Execution> {
        EngineBuilder::new()
    }

    /// Run the trading [`Engine`]. Spawns a thread for each [`Trader`] instance in the [`Engine`] and run
    /// the [`Trader`] event-loop. Asynchronously awaits a remote shutdown [`Message`]
    /// via the [`Engine`]'s termination_rx. After remote shutdown has been initiated, the trading
    /// period's statistics are generated & printed with the provided Statistic component.
    pub async fn run(mut self) {
        // Run Traders on threads & send notification when they have stopped organically
        let mut notify_traders_stopped = self.run_traders().await;

        loop {
            // Action received commands from remote, or wait for all Traders to stop organically
            tokio::select! {
                _ = notify_traders_stopped.recv() => {
                    break;
                },

                command = self.command_rx.recv() => {
                    if let Some(command) = command {
                        match command {
                            Command::FetchOpenPositions(positions_tx) => {
                                self.fetch_open_positions(positions_tx).await;
                            },
                            // Command::SendSummary(summary_tx) => {
                            //     self.send_summary(summary_tx).await;
                            // }
                            Command::Terminate(message) => {
                                self.terminate_traders(message).await;
                                break;
                            },
                            Command::ExitPosition(market) => {
                                self.exit_position(market).await;
                            },
                            Command::ExitAllPositions => {
                                self.exit_all_positions().await;
                            },
                        }
                    } else {
                        // Terminate traders due to dropped receiver
                        break;
                    }
                }
            }
        };

        // // Unlock Portfolio Mutex to access backtest information
        // let mut portfolio = match self.portfolio.lock() {
        //     Ok(portfolio) => portfolio,
        //     Err(err) => {
        //         warn!("Mutex poisoned with error: {}", err);
        //         err.into_inner()
        //     }
        // };
        //
        // self.trader_command_txs
        //     .into_keys()
        //     .map(|market| {
        //         let market_stats = portfolio.get_statistics();
        //         market_stats.print();
        //     });
        //
        // // Generate TradingSummary
        // match portfolio.get_exited_positions(&Uuid::new_v4()).unwrap() {
        //     None => info!("Backtest yielded no closed Positions - no TradingSummary available"),
        //     Some(closed_positions) => {
        //         self.statistics.generate_summary(&closed_positions);
        //         self.statistics.print();
        //     }
        // }
    }

    /// Todo: Also deal w/ unwraps
    async fn run_traders(&mut self) -> mpsc::Receiver<bool> {
        // Extract Traders out of the Engine so we can move them into threads
        let traders = std::mem::replace(
            &mut self.traders, Vec::with_capacity(0)
        );

        // Run each Trader instance on it's own thread
        let mut thread_handles = Vec::with_capacity(traders.len());
        for trader in traders.into_iter() {
            let handle = thread::spawn(move || trader.run());
            thread_handles.push(handle);
        }

        // Create channel to notify the Engine when the Traders have stopped organically
        let (notify_tx, notify_rx) = mpsc::channel(1);

        // Create Task that notifies Engine when the Traders have stopped organically
        tokio::spawn(async move {
            for handle in thread_handles {
                handle.join().unwrap()
            }

            let _ = notify_tx.send(true).await;
        });

        notify_rx
    }

    /// Todo:
    async fn fetch_open_positions(&self, positions_tx: oneshot::Sender<Result<Vec<Position>, EngineError>>) {
        let open_positions = self
            .portfolio
            .lock().unwrap()
            .get_open_positions(&self.engine_id, self.trader_command_txs.keys())
            .map_err(EngineError::from);

        if positions_tx.send(open_positions).is_err() {
            warn!(why = "oneshot receiver dropped", "cannot action Command::SendOpenPositions");
        }
    }

    /// Todo:
    fn send_summary(&self, summary_tx: oneshot::Sender<Result<TradingSummary, EngineError>>) {
        todo!()
    }
    /// Todo:
    async fn terminate_traders(&self, message: Message) {
        for (market, command_tx) in self.trader_command_txs.iter() {
            if command_tx.send(Command::Terminate(message.clone())).await.is_err() {
                error!(
                        market = &*format!("{:?}", market),
                        why = "dropped receiver",
                        "failed to send Command::Terminate to Trader command_rx"
                );
            }
        }
    }

    /// Todo:
    async fn exit_position(&self, market: Market) {
        if let Some((market_ref, command_tx)) = self.trader_command_txs.get_key_value(&market) {
            if command_tx.send(Command::ExitPosition(market)).await.is_err() {
                error!(
                    market = &*format!("{:?}", market_ref),
                    why = "dropped receiver",
                    "failed to send Command::Terminate to Trader command_rx"
                );
            }
        } else {
            warn!(
                market = &*format!("{:?}", market),
                why = "Engine has no trader_command_tx associated with provided Market",
                "failed to exit Position"
            );
        }
    }

    /// Todo:
    async fn exit_all_positions(&self) {
        for (market, command_tx) in self.trader_command_txs.iter() {
            if command_tx.send(Command::ExitPosition(market.clone())).await.is_err() {
                error!(
                    market = &*format!("{:?}", market),
                    why = "dropped receiver",
                    "failed to send Command::Terminate to Trader command_rx"
                );
            }
        }
    }
}

/// Builder to construct [`Engine`] instances.
#[derive(Debug)]
pub struct EngineBuilder<EventTx, Statistic, Portfolio, Data, Strategy, Execution>
where
    EventTx: MessageTransmitter<Event<Statistic>>,
    Statistic: Serialize + Send,
    Portfolio: MarketUpdater + OrderGenerator + FillUpdater<Statistic> + Send,
    Data: Continuer + MarketGenerator + Send,
    Strategy: SignalGenerator + Send,
    Execution: FillGenerator + Send,
{
    engine_id: Option<Uuid>,
    command_rx: Option<mpsc::Receiver<Command>>,
    portfolio: Option<Arc<Mutex<Portfolio>>>,
    traders: Option<Vec<Trader<EventTx, Statistic, Portfolio, Data, Strategy, Execution>>>,
    trader_command_txs: Option<HashMap<Market, mpsc::Sender<Command>>>,
}

impl<EventTx, Statistic, Portfolio, Data, Strategy, Execution> EngineBuilder<EventTx, Statistic, Portfolio, Data, Strategy, Execution>
where
    EventTx: MessageTransmitter<Event<Statistic>>,
    Statistic: Serialize + Send,
    Portfolio: PositionHandler + MarketUpdater + OrderGenerator + FillUpdater<Statistic> + Send,
    Data: Continuer + MarketGenerator + Send,
    Strategy: SignalGenerator + Send,
    Execution: FillGenerator + Send,
{
    fn new() -> Self {
        Self {
            engine_id: None,
            command_rx: None,
            portfolio: None,
            traders: None,
            trader_command_txs: None,
        }
    }

    pub fn engine_id(self, value: Uuid) -> Self {
        Self {
            engine_id: Some(value),
            ..self
        }
    }

    pub fn command_rx(self, value: mpsc::Receiver<Command>) -> Self {
        Self {
            command_rx: Some(value),
            ..self
        }
    }

    pub fn portfolio(self, value: Arc<Mutex<Portfolio>>) -> Self {
        Self {
            portfolio: Some(value),
            ..self
        }
    }

    pub fn traders(self, value: Vec<Trader<EventTx, Statistic, Portfolio, Data, Strategy, Execution>>) -> Self {
        Self {
            traders: Some(value),
            ..self
        }
    }

    pub fn trader_command_txs(self, value: HashMap<Market, mpsc::Sender<Command>>) -> Self {
        Self {
            trader_command_txs: Some(value),
            ..self
        }
    }


    pub fn build(self) -> Result<Engine<EventTx, Statistic, Portfolio, Data, Strategy, Execution>, EngineError> {
        let engine_id = self.engine_id.ok_or(EngineError::BuilderIncomplete)?;
        let command_rx = self.command_rx.ok_or(EngineError::BuilderIncomplete)?;
        let portfolio = self.portfolio.ok_or(EngineError::BuilderIncomplete)?;
        let traders = self.traders.ok_or(EngineError::BuilderIncomplete)?;
        let trader_command_txs = self.trader_command_txs.ok_or(EngineError::BuilderIncomplete)?;

        Ok(Engine {
            engine_id,
            command_rx,
            portfolio,
            traders,
            trader_command_txs,
        })
    }
}