use crate::Store;
use anyhow::Result;
use parfait_core::Document;
use std::{
    path::PathBuf,
    sync::{Arc, Condvar, Mutex, mpsc},
    thread::{self, JoinHandle},
};

pub enum SaveEvent {
    Saved(u64),
    Failed(String),
}
#[derive(Default)]
struct Queue {
    latest: Option<Document>,
    closing: bool,
}
pub struct Autosave {
    queue: Arc<(Mutex<Queue>, Condvar)>,
    events: mpsc::Receiver<SaveEvent>,
    thread: Option<JoinHandle<()>>,
}
impl Autosave {
    pub fn start(path: PathBuf) -> Result<Self> {
        let store = Store::open(&path)?;
        store.mark_open()?;
        let queue = Arc::new((Mutex::new(Queue::default()), Condvar::new()));
        let q = queue.clone();
        let (send, events) = mpsc::channel();
        let thread = thread::spawn(move || {
            let mut store = store;
            let mut healthy = true;
            loop {
                let (lock, ready) = &*q;
                let mut state = lock.lock().unwrap();
                while state.latest.is_none() && !state.closing {
                    state = ready.wait(state).unwrap();
                }
                let doc = state.latest.take();
                let closing = state.closing;
                drop(state);
                if let Some(doc) = doc {
                    match store.save(&doc) {
                        Ok(()) => {
                            healthy = true;
                            let _ = send.send(SaveEvent::Saved(doc.revision));
                        }
                        Err(e) => {
                            healthy = false;
                            let _ = send.send(SaveEvent::Failed(format!("{e:#}")));
                        }
                    }
                }
                if closing {
                    if healthy {
                        let _ = store.mark_clean();
                        let _ = store.checkpoint();
                    }
                    break;
                }
            }
        });
        Ok(Self {
            queue,
            events,
            thread: Some(thread),
        })
    }
    /// Coalesces pending snapshots instead of allowing an unbounded memory queue.
    pub fn submit(&self, doc: Document) {
        let (lock, ready) = &*self.queue;
        lock.lock().unwrap().latest = Some(doc);
        ready.notify_one();
    }
    pub fn poll(&self) -> Option<SaveEvent> {
        self.events.try_recv().ok()
    }
    pub fn finish(&mut self, doc: Document) {
        self.submit(doc);
        self.close();
    }
    fn close(&mut self) {
        let (lock, ready) = &*self.queue;
        lock.lock().unwrap().closing = true;
        ready.notify_one();
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}
impl Drop for Autosave {
    fn drop(&mut self) {
        self.close();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn final_flush_keeps_latest_coalesced_revision() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session.parfait");
        let mut worker = Autosave::start(path.clone()).unwrap();
        let mut doc = Document::new(8, 8);
        for n in 0..100 {
            doc.name = format!("revision {n}");
            doc.touch();
            worker.submit(doc.clone());
        }
        worker.finish(doc.clone());
        let store = Store::open(&path).unwrap();
        assert!(!store.unclean().unwrap());
        assert_eq!(store.load().unwrap().unwrap().document, doc);
    }
}
