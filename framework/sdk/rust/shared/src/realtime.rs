use framework_sdk_shared_types::RealtimeEnvelope;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct RealtimeConfig {
    pub ws_url: String,
    pub bearer_token: String,
    pub heartbeat: Duration,
    pub reconnect_base: Duration,
}

impl Default for RealtimeConfig {
    fn default() -> Self {
        Self {
            ws_url: "ws://localhost:8080/graphql".to_string(),
            bearer_token: String::new(),
            heartbeat: Duration::from_secs(20),
            reconnect_base: Duration::from_millis(500),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RealtimeState {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting,
}

pub trait RealtimeSubscriber: Send + Sync {
    fn on_event(&self, envelope: &RealtimeEnvelope);
    fn on_state_changed(&self, _state: RealtimeState) {}
}

pub struct RealtimeClient {
    pub config: RealtimeConfig,
    state: RealtimeState,
    retry_count: u32,
}

impl RealtimeClient {
    pub fn new(config: RealtimeConfig) -> Self {
        Self {
            config,
            state: RealtimeState::Disconnected,
            retry_count: 0,
        }
    }

    pub fn state(&self) -> RealtimeState {
        self.state
    }

    pub fn connect(&mut self) {
        self.state = RealtimeState::Connected;
        self.retry_count = 0;
    }

    pub fn disconnect(&mut self) {
        self.state = RealtimeState::Disconnected;
    }

    pub fn next_reconnect_delay(&mut self) -> Duration {
        self.state = RealtimeState::Reconnecting;
        let factor = 2u32.saturating_pow(self.retry_count.min(8));
        self.retry_count = self.retry_count.saturating_add(1);
        self.config.reconnect_base * factor
    }
}
