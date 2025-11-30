//! Event handling
//!
//! Defines the Event enum and EventHandler for async event processing.
//! Uses tokio channels to decouple input from processing.

use std::time::Duration;

use crossterm::event::{self, Event as CrosstermEvent, KeyEvent};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

use crate::ai_client::AIError;
use crate::executor::CommandResult;

/// Events that can occur in the application
#[derive(Debug)]
pub enum Event {
    /// Keyboard input event
    Key(KeyEvent),
    /// Periodic tick for animations (spinner, etc.)
    Tick,
    /// Terminal resize event
    Resize(u16, u16),
    /// API response received (success or error)
    ApiResponse(Result<String, AIError>),
    /// Command execution completed
    CommandComplete(CommandResult),
    /// Command was cancelled
    CommandCancelled,
    /// Models list response (models, optional model to switch to)
    ModelsResponse(Result<Vec<String>, AIError>, Option<String>),
}

/// Handles async event collection and distribution
pub struct EventHandler {
    /// Receiver for events
    rx: UnboundedReceiver<Event>,
    /// Sender for events (kept for spawning tasks)
    tx: UnboundedSender<Event>,
}

impl EventHandler {
    /// Create a new EventHandler with the specified tick rate
    ///
    /// Spawns a background task that polls for terminal events and sends
    /// tick events at the specified interval.
    pub fn new(tick_rate: Duration) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        let event_tx = tx.clone();

        // Spawn the event polling task
        tokio::spawn(async move {
            loop {
                // Poll for crossterm events with timeout
                if event::poll(tick_rate).unwrap_or(false) {
                    if let Ok(evt) = event::read() {
                        let event = match evt {
                            CrosstermEvent::Key(key) => Event::Key(key),
                            CrosstermEvent::Resize(w, h) => Event::Resize(w, h),
                            _ => continue, // Ignore other events
                        };
                        if event_tx.send(event).is_err() {
                            break; // Channel closed, exit loop
                        }
                    }
                } else {
                    // Timeout - send tick event
                    if event_tx.send(Event::Tick).is_err() {
                        break; // Channel closed, exit loop
                    }
                }
            }
        });

        Self { rx, tx }
    }

    /// Get the next event asynchronously
    ///
    /// Returns None if the channel is closed.
    pub async fn next(&mut self) -> Option<Event> {
        self.rx.recv().await
    }

    /// Get a sender for sending events from other tasks
    ///
    /// This is useful for sending ApiResponse and CommandComplete events
    /// from async operations.
    pub fn sender(&self) -> UnboundedSender<Event> {
        self.tx.clone()
    }
}
