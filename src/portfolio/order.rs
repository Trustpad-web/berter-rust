use uuid::Uuid;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use crate::strategy::signal::Decision;
use crate::portfolio::error::PortfolioError;
use crate::portfolio::error::PortfolioError::BuilderIncomplete;

// Todo: Add rust docs etc
// OrderEvent contains work to be done by an Execution to execute a trade
#[derive(Debug, PartialOrd, PartialEq, Serialize, Deserialize)]
pub struct OrderEvent {
    pub trace_id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub exchange: String,
    pub symbol: String,
    pub close: f64,
    pub decision: Decision,    // LONG, CloseLong, SHORT or CloseShort
    pub quantity: f64,         // +ve or -ve Quantity depending on Decision
    pub order_type: OrderType, // MARKET, LIMIT etc
}

impl Default for OrderEvent {
    fn default() -> Self {
        Self {
            trace_id: Uuid::new_v4(),
            timestamp: Utc::now(),
            exchange: String::from("BINANCE"),
            symbol: String::from("ETH-USD"),
            close: 1050.0,
            decision: Decision::default(),
            quantity: 10.0,
            order_type: OrderType::default(),
        }
    }
}

impl OrderEvent {
    /// Returns a OrderEventBuilder instance.
    pub fn builder() -> OrderEventBuilder {
        OrderEventBuilder::new()
    }
}

#[derive(Debug, Clone, PartialOrd, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OrderType {
    Market,
    Limit,
    Bracket,
}

impl Default for OrderType {
    fn default() -> Self {
        Self::Market
    }
}

pub struct OrderEventBuilder {
    pub trace_id: Option<Uuid>,
    pub timestamp: Option<DateTime<Utc>>,
    pub exchange: Option<String>,
    pub symbol: Option<String>,
    pub close: Option<f64>,
    pub decision: Option<Decision>,
    pub quantity: Option<f64>,
    pub order_type: Option<OrderType>,
}

impl OrderEventBuilder {
    pub fn new() -> Self {
        Self {
            trace_id: None,
            timestamp: None,
            exchange: None,
            symbol: None,
            close: None,
            decision: None,
            quantity: None,
            order_type: None,
        }
    }

    pub fn trace_id(mut self, value: Uuid) -> Self {
        self.trace_id = Some(value);
        self
    }

    pub fn timestamp(mut self, value: DateTime<Utc>) -> Self {
        self.timestamp = Some(value);
        self
    }

    pub fn exchange(mut self, value: String) -> Self {
        self.exchange = Some(value);
        self
    }

    pub fn symbol(mut self, value: String) -> Self {
        self.symbol = Some(value);
        self
    }

    pub fn close(mut self, value: f64) -> Self {
        self.close = Some(value);
        self
    }

    pub fn decision(mut self, value: Decision) -> Self {
        self.decision = Some(value);
        self
    }

    pub fn quantity(mut self, value: f64) -> Self {
        self.quantity = Some(value);
        self
    }

    pub fn order_type(mut self, value: OrderType) -> Self {
        self.order_type = Some(value);
        self
    }

    pub fn build(self) -> Result<OrderEvent, PortfolioError> {
        if let (
            Some(trace_id),
            Some(timestamp),
            Some(exchange),
            Some(symbol),
            Some(close),
            Some(decision),
            Some(quantity),
            Some(order_type),
        ) = (
            self.trace_id,
            self.timestamp,
            self.exchange,
            self.symbol,
            self.close,
            self.decision,
            self.quantity,
            self.order_type,
        ) {
            Ok(OrderEvent {
                trace_id,
                timestamp,
                exchange,
                symbol,
                close,
                decision,
                quantity,
                order_type,
            })
        } else {
            Err(BuilderIncomplete())
        }
    }
}