use std::time::Duration;

#[derive(Debug, Clone)]
pub struct Config {
    pub max_concurrent_streams: usize,
    pub send_recv_timeout: Duration,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            max_concurrent_streams: 3,
            send_recv_timeout: Duration::from_secs(10),
        }
    }
}
