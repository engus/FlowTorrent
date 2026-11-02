use std::{
    collections::BTreeMap,
    num::NonZeroU32,
    sync::Mutex,
    time::{Duration, Instant},
};
use tokio::sync::Notify;

struct Request {
    torrent: usize,
    size: u32,
    finish: u128,
}
struct State {
    bps: u32,
    tokens: f64,
    updated: Instant,
    clock: u128,
    ticket: u64,
    requests: BTreeMap<u64, Request>,
    finishes: BTreeMap<usize, u128>,
}
impl State {
    fn remove(&mut self, ticket: u64) {
        match self.requests.remove(&ticket) {
            Some(request) if !self.requests.values().any(|r| r.torrent == request.torrent) => {
                self.finishes.remove(&request.torrent);
            }
            _ => {}
        }
    }
}

/// Packetized weighted fair admission at the shared download cap. Peers of the
/// same torrent share one virtual clock; idle/cancelled flows retain no credit.
pub(crate) struct DownloadQueue {
    state: Mutex<State>,
    changed: Notify,
}
impl DownloadQueue {
    pub(crate) fn new(bps: Option<NonZeroU32>) -> Self {
        let bps = bps.map_or(0, NonZeroU32::get);
        Self {
            state: Mutex::new(State {
                bps,
                tokens: f64::from(bps),
                updated: Instant::now(),
                clock: 0,
                ticket: 0,
                requests: BTreeMap::new(),
                finishes: BTreeMap::new(),
            }),
            changed: Notify::new(),
        }
    }
    pub(crate) fn set_rate(&self, bps: Option<NonZeroU32>) {
        let mut state = self.state.lock().unwrap();
        state.bps = bps.map_or(0, NonZeroU32::get);
        state.tokens = f64::from(state.bps);
        state.updated = Instant::now();
        drop(state);
        self.changed.notify_waiters();
    }
    pub(crate) async fn reserve(&self, torrent: usize, weight: u8, size: NonZeroU32) {
        let ticket = {
            let mut state = self.state.lock().unwrap();
            if state.bps == 0 {
                return;
            }
            state.ticket += 1;
            let ticket = state.ticket;
            let finish = state
                .finishes
                .get(&torrent)
                .copied()
                .unwrap_or(state.clock)
                .max(state.clock)
                + u128::from(size.get()) * 256 / u128::from(weight.max(1));
            state.finishes.insert(torrent, finish);
            state.requests.insert(
                ticket,
                Request {
                    torrent,
                    size: size.get(),
                    finish,
                },
            );
            ticket
        };
        let _pending = Pending {
            queue: self,
            ticket,
        };
        self.changed.notify_waiters();
        loop {
            let changed = self.changed.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            let delay = {
                let mut state = self.state.lock().unwrap();
                if state.bps == 0 {
                    return;
                }
                let now = Instant::now();
                state.tokens = (state.tokens
                    + now.duration_since(state.updated).as_secs_f64() * f64::from(state.bps))
                .min(f64::from(state.bps.max(size.get())));
                state.updated = now;
                let (&next, request) = state
                    .requests
                    .iter()
                    .min_by_key(|(id, r)| (r.finish, **id))
                    .unwrap();
                let finish = request.finish;
                let needed = f64::from(request.size) - state.tokens;
                if needed <= 0.0 && next == ticket {
                    state.tokens -= f64::from(size.get());
                    state.clock = state.clock.max(finish);
                    state.remove(ticket);
                    return;
                }
                if needed > 0.0 {
                    Some(Duration::from_secs_f64(needed / f64::from(state.bps)))
                } else {
                    None
                }
            };
            match delay {
                Some(delay) => {
                    tokio::select! { _ = changed.as_mut() => {}, _ = tokio::time::sleep(delay) => {} }
                }
                None => changed.await,
            }
        }
    }
}
struct Pending<'a> {
    queue: &'a DownloadQueue,
    ticket: u64,
}
impl Drop for Pending<'_> {
    fn drop(&mut self) {
        self.queue.state.lock().unwrap().remove(self.ticket);
        self.queue.changed.notify_waiters();
    }
}
