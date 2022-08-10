use embassy_executor::time::{Instant, Timer};
use futures::future::pending;

#[derive(Copy, Clone)]
pub enum Expiration {
    Network,
    Publish,
    Ack,
}

pub struct Deadline {
    network: Option<Instant>,
    publish: Option<Instant>,
    ack: Option<Instant>,
}

impl Default for Deadline {
    fn default() -> Self {
        Self {
            network: None,
            publish: None,
            ack: None,
        }
    }
}

impl Deadline {
    pub fn network(&mut self, deadline: Option<Instant>) {
        match (self.network, deadline) {
            (Some(a), Some(b)) if b < a => {
                self.network.replace(b);
            }
            (None, Some(b)) => {
                self.network.replace(b);
            }
            (Some(_), None) => {
                self.network.take();
            }
            _ => {
                // earliest deadline already set
            }
        }
    }

    pub fn publish(&mut self, deadline: Option<Instant>) {
        match (self.publish, deadline) {
            (Some(a), Some(b)) if b < a => {
                self.publish.replace(b);
            }
            (None, Some(b)) => {
                self.publish.replace(b);
            }
            (Some(_), None) => {
                self.publish.take();
            }
            _ => {
                // earliest deadline already set
            }
        }
    }

    pub fn ack(&mut self, deadline: Option<Instant>) {
        match (self.ack, deadline) {
            (Some(a), Some(b)) if b < a => {
                self.ack.replace(b);
            }
            (None, Some(b)) => {
                self.ack.replace(b);
            }
            (Some(_), None) => {
                self.ack.take();
            }
            _ => {
                // earliest deadline already set
            }
        }
    }

    /// Wait for the next earliest deadline, knowing which deadline passed
    /// when this method returns.
    ///
    /// In the event no deadlines are waitable, this method will be Pending
    /// forever, until the future is dropped.
    pub async fn next(&mut self) -> Expiration {
        if let Some(earliest) = self.earliest() {
            Timer::at(earliest.1).await;
            self.clear(earliest.0);
            earliest.0
        } else {
            pending().await
        }
    }

    fn clear(&mut self, expiration: Expiration) {
        match expiration {
            Expiration::Network => {
                self.network.take();
            }
            Expiration::Publish => {
                self.publish.take();
            }
            Expiration::Ack => {
                self.ack.take();
            }
        }
    }

    fn earliest(&self) -> Option<(Expiration, Instant)> {
        let mut result: Option<(Expiration, Instant)> = None;

        if let Some(network) = self.network {
            if let Some(prev) = &result {
                if prev.1 > network {
                    result.replace((Expiration::Network, network));
                }
            } else {
                result.replace((Expiration::Network, network));
            }
        }

        if let Some(publish) = self.publish {
            if let Some(prev) = &result {
                if prev.1 > publish {
                    result.replace((Expiration::Publish, publish));
                }
            } else {
                result.replace((Expiration::Publish, publish));
            }
        }

        if let Some(ack) = self.ack {
            if let Some(prev) = &result {
                if prev.1 > ack {
                    result.replace((Expiration::Ack, ack));
                }
            } else {
                result.replace((Expiration::Ack, ack));
            }
        }

        result
    }
}
