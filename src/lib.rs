// Copyright 2024 System76 <info@system76.com>
// SPDX-License-Identifier: MPL-2.0

//! ```no_run
//! use futures_util::StreamExt;
//!
//! #[tokio::main(flavor = "current_thread")]
//! async fn main() -> std::io::Result<()> {
//!     let mut ac_plug_events = acpid_plug::connect().await?;
//!
//!     while let Some(event) = ac_plug_events.next().await {
//!         match event {
//!             Ok(event) => println!("{:?}", event),
//!             Err(why) => eprintln!("error: {}", why),
//!         }
//!     }
//!
//!     Ok(())
//! }
//! ```

use std::path::Path;
use std::task::Poll;

use futures_util::FutureExt;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::net::UnixStream;

const DEFAULT_SOCKET: &str = "/var/run/acpid.socket";
const BAT0_STATUS: &str = "/sys/class/power_supply/BAT0/status";
const BAT1_STATUS: &str = "/sys/class/power_supply/BAT1/status";

/// Listens for AC plug events from `/var/run/acpid.socket`.
pub async fn connect() -> std::io::Result<AcPlugEvents> {
    AcPlugEvents::connect().await
}

/// Listens for AC plug events from a custom socket.
pub async fn with_socket<P: AsRef<Path>>(socket: P) -> std::io::Result<AcPlugEvents> {
    AcPlugEvents::with_socket(socket).await
}

/// Whether the power adapter has been plugged or unplugged.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum Event {
    Plugged,
    Unplugged,
}

/// A stream of power adapter plug events.
pub struct AcPlugEvents {
    reader: BufReader<UnixStream>,
    line: String,
    plugged: bool,
}

impl AcPlugEvents {
    /// Listens for AC plug events from `/var/run/acpid.socket`.
    pub async fn connect() -> std::io::Result<Self> {
        Self::with_socket(DEFAULT_SOCKET).await
    }

    /// Listens for AC plug events from a custom socket.
    pub async fn with_socket<P: AsRef<Path>>(socket: P) -> std::io::Result<Self> {
        let stream = UnixStream::connect(socket).await?;

        Ok(Self {
            reader: BufReader::new(stream),
            line: String::new(),
            plugged: {
                let status = match tokio::fs::read_to_string(BAT1_STATUS).await {
                    Ok(string) => string,
                    Err(_) => tokio::fs::read_to_string(BAT0_STATUS).await?,
                };

                status.trim() != "Discharging"
            },
        })
    }
}

impl futures_util::Stream for AcPlugEvents {
    type Item = std::io::Result<Event>;

    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();

        loop {
            let mut line_read = Box::pin(this.reader.read_line(&mut this.line));

            match line_read.poll_unpin(cx) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(Ok(read)) => {
                    if read == 0 {
                        return Poll::Ready(None);
                    }

                    let read_line = &this.line[..read].trim();

                    if read_line.starts_with("ac_adapter") {
                        if this.plugged {
                            if read_line.ends_with('0') {
                                this.plugged = false;
                                this.line.clear();
                                return Poll::Ready(Some(Ok(Event::Unplugged)));
                            }
                        } else if read_line.ends_with('1') {
                            this.plugged = true;
                            this.line.clear();
                            return Poll::Ready(Some(Ok(Event::Plugged)));
                        }
                    }

                    this.line.clear();
                }
                Poll::Ready(Err(why)) => return Poll::Ready(Some(Err(why))),
            }
        }
    }
}
