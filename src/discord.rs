//! discord rich presence: what the pilot is doing, on their discord profile,
//! through the discord app's local ipc. it needs the game's application id
//! from discord's developer portal (`APP_ID`, or `FABLE_DISCORD_APP` to try
//! one); without one, or without discord running, it does nothing.

use crossbeam_channel::{Receiver, RecvTimeoutError, Sender};
use std::time::Duration;

/// the game's application on discord's developer portal (empty: off).
pub const APP_ID: &str = "";

/// what the profile says: the top line, the one under it, and since when (unix s).
#[derive(Clone, Debug, PartialEq)]
pub struct Status {
    pub details: String,
    pub state: String,
    pub since: i64,
}

/// the link to discord, on its own thread so a slow or missing discord never stalls a frame.
pub struct Presence {
    tx: Sender<Option<Status>>,
    last: Option<Option<Status>>,
}

impl Presence {
    /// none without an application id.
    pub fn start() -> Option<Presence> {
        let id = std::env::var("FABLE_DISCORD_APP").ok().filter(|s| !s.trim().is_empty()).or_else(|| (!APP_ID.is_empty()).then(|| APP_ID.to_string()))?;
        let (tx, rx) = crossbeam_channel::unbounded();
        std::thread::Builder::new().name("discord".into()).spawn(move || run(id, rx)).ok()?;
        Some(Presence { tx, last: None })
    }

    /// what to show now (none: nothing); sent on only when it changes.
    pub fn set(&mut self, s: Option<Status>) {
        if self.last.as_ref() != Some(&s) {
            let _ = self.tx.send(s.clone());
            self.last = Some(s);
        }
    }
}

fn run(id: String, rx: Receiver<Option<Status>>) {
    use discord_rich_presence::{activity, DiscordIpc, DiscordIpcClient};
    let mut client: Option<DiscordIpcClient> = None;
    let mut want: Option<Status> = None;
    let mut dirty = false;
    // discord takes about five updates in twenty seconds: one every four at most.
    let gap = Duration::from_secs(4);
    loop {
        match rx.recv_timeout(gap) {
            Ok(s) => {
                want = s;
                dirty = true;
                // only the newest of a burst matters.
                while let Ok(s) = rx.try_recv() {
                    want = s;
                }
            }
            Err(RecvTimeoutError::Disconnected) => {
                if let Some(c) = client.as_mut() {
                    let _ = c.close();
                }
                return;
            }
            Err(RecvTimeoutError::Timeout) => {}
        }
        if !dirty {
            continue;
        }
        // (re)connect while discord is not there yet, or has gone and come back.
        if client.is_none() {
            let mut c = DiscordIpcClient::new(&id);
            if c.connect().is_err() {
                continue;
            }
            log::info!("discord: connected");
            client = Some(c);
        }
        let Some(c) = client.as_mut() else { continue };
        let r = match &want {
            Some(s) => c.set_activity(
                activity::Activity::new()
                    .details(s.details.as_str())
                    .state(s.state.as_str())
                    .timestamps(activity::Timestamps::new().start(s.since * 1000))
                    .assets(activity::Assets::new().large_image("logo").large_text("FableFlight")),
            ),
            None => c.clear_activity(),
        };
        match r {
            Ok(()) => dirty = false,
            Err(e) => {
                log::info!("discord: {e}");
                client = None;
            }
        }
    }
}

/// now, in unix seconds.
pub fn now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
}
