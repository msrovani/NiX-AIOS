//! Fila Sistema 2 — worker dedicado; tick S1 não partilha o lock do HotSwap.
//! Onda 4.5: `submit` fire-and-forget + `poll`; `submit_wait` para chat sync.
//! Honesty: 1 thread; CAP=8; results CAP=32 FIFO; sem guarantee TTFT.

use parking_lot::Mutex;
use std::collections::VecDeque;
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::sync::Arc;
use std::thread;
use tracing::{info, warn};

const CAP: usize = 8;
const RESULT_CAP: usize = 32;

#[derive(Debug, Clone)]
pub struct S2Job {
    pub id: u64,
    pub prompt: String,
}

#[derive(Debug, Clone)]
pub struct S2Result {
    pub id: u64,
    pub ok: bool,
    pub text: String,
}

type InferFn = Arc<dyn Fn(&str) -> Result<String, String> + Send + Sync>;

struct JobMsg {
    job: S2Job,
    /// Se Some, também notifica o waiter (submit_wait).
    reply: Option<SyncSender<S2Result>>,
}

#[derive(Clone)]
pub struct S2Queue {
    tx: SyncSender<JobMsg>,
    next_id: Arc<Mutex<u64>>,
    pending: Arc<Mutex<usize>>,
    results: Arc<Mutex<VecDeque<S2Result>>>,
}

impl S2Queue {
    pub fn start(infer: InferFn) -> Self {
        let (tx, rx) = mpsc::sync_channel::<JobMsg>(CAP);
        let pending = Arc::new(Mutex::new(0usize));
        let results = Arc::new(Mutex::new(VecDeque::with_capacity(RESULT_CAP)));
        let pending_w = pending.clone();
        let results_w = results.clone();
        thread::Builder::new()
            .name("nosd-s2".into())
            .spawn(move || worker(rx, infer, pending_w, results_w))
            .expect("s2 worker");
        Self {
            tx,
            next_id: Arc::new(Mutex::new(1)),
            pending,
            results,
        }
    }

    pub fn pending(&self) -> usize {
        *self.pending.lock()
    }

    fn alloc_id(&self) -> u64 {
        let mut g = self.next_id.lock();
        let id = *g;
        *g = g.wrapping_add(1);
        id
    }

    fn enqueue(&self, prompt: String, reply: Option<SyncSender<S2Result>>) -> Result<u64, String> {
        let id = self.alloc_id();
        {
            let mut p = self.pending.lock();
            *p = p.saturating_add(1);
        }
        match self.tx.try_send(JobMsg {
            job: S2Job { id, prompt },
            reply,
        }) {
            Ok(()) => Ok(id),
            Err(mpsc::TrySendError::Full(_)) => {
                let mut p = self.pending.lock();
                *p = p.saturating_sub(1);
                Err("s2 queue full (CAP=8) — retry later".into())
            }
            Err(mpsc::TrySendError::Disconnected(_)) => {
                let mut p = self.pending.lock();
                *p = p.saturating_sub(1);
                Err("s2 worker dead".into())
            }
        }
    }

    /// Fire-and-forget — resultado via `poll` / `take_ready`.
    pub fn submit(&self, prompt: String) -> Result<u64, String> {
        self.enqueue(prompt, None)
    }

    /// Enfileira e espera (bloqueia o caller do chat, NÃO o tick).
    pub fn submit_wait(&self, prompt: String) -> Result<S2Result, String> {
        let (rtx, rrx) = mpsc::sync_channel(1);
        let id = self.enqueue(prompt, Some(rtx))?;
        let r = rrx.recv().map_err(|_| "s2 worker dropped".to_string())?;
        debug_assert_eq!(r.id, id);
        Ok(r)
    }

    /// Consome resultado pronto por id (None = ainda a correr / desconhecido).
    pub fn poll(&self, id: u64) -> Option<S2Result> {
        let mut g = self.results.lock();
        if let Some(pos) = g.iter().position(|r| r.id == id) {
            return g.remove(pos);
        }
        None
    }

    /// Drena até `max` resultados prontos (FIFO).
    pub fn take_ready(&self, max: usize) -> Vec<S2Result> {
        let mut g = self.results.lock();
        let n = max.min(g.len());
        g.drain(..n).collect()
    }

    /// Parqueia resultado já pronto (S1 sync → mesmo contrato poll que S2).
    pub fn park_done(&self, text: String, ok: bool) -> u64 {
        let id = self.alloc_id();
        let mut g = self.results.lock();
        if g.len() >= RESULT_CAP {
            g.pop_front();
        }
        g.push_back(S2Result { id, ok, text });
        id
    }
}

fn worker(
    rx: Receiver<JobMsg>,
    infer: InferFn,
    pending: Arc<Mutex<usize>>,
    results: Arc<Mutex<VecDeque<S2Result>>>,
) {
    while let Ok(msg) = rx.recv() {
        let job = msg.job;
        info!(target: "nosd", "s2 worker start id={} len={}", job.id, job.prompt.len());
        let out = match infer(&job.prompt) {
            Ok(t) => S2Result {
                id: job.id,
                ok: true,
                text: t,
            },
            Err(e) => {
                warn!(target: "nosd", "s2 worker fail id={} err={e}", job.id);
                S2Result {
                    id: job.id,
                    ok: false,
                    text: e,
                }
            }
        };
        if let Some(tx) = msg.reply {
            let _ = tx.send(out.clone());
        }
        {
            let mut g = results.lock();
            if g.len() >= RESULT_CAP {
                g.pop_front();
            }
            g.push_back(out);
        }
        let mut p = pending.lock();
        *p = p.saturating_sub(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    #[test]
    fn submit_echo() {
        let n = Arc::new(AtomicUsize::new(0));
        let n2 = n.clone();
        let q = S2Queue::start(Arc::new(move |p| {
            n2.fetch_add(1, Ordering::SeqCst);
            Ok(format!("s2:{p}"))
        }));
        let r = q.submit_wait("hi".into()).unwrap();
        assert!(r.ok);
        assert_eq!(r.text, "s2:hi");
        assert_eq!(n.load(Ordering::SeqCst), 1);
        // pending pode ainda ser 1 até o worker decrementar pós-result — bounded wait.
        for _ in 0..50 {
            if q.pending() == 0 {
                break;
            }
            thread::sleep(Duration::from_millis(2));
        }
        assert_eq!(q.pending(), 0);
    }

    #[test]
    fn submit_nowait_poll() {
        let q = S2Queue::start(Arc::new(|p| {
            thread::sleep(Duration::from_millis(30));
            Ok(format!("async:{p}"))
        }));
        let id = q.submit("x".into()).unwrap();
        assert!(q.poll(id).is_none() || q.pending() <= 1);
        let mut got = None;
        for _ in 0..50 {
            if let Some(r) = q.poll(id) {
                got = Some(r);
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }
        let r = got.expect("result");
        assert!(r.ok);
        assert_eq!(r.text, "async:x");
        assert_eq!(q.pending(), 0);
    }
}
