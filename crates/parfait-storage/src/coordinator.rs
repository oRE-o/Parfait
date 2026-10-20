//! One bounded storage worker. Original jobs have bounded priority over fair recovery.
use crate::{
    NativePath, RecoveryMetadata, SnapshotHead, SourceEvent, SourceFailure,
    SourceRequest, Store, recovery_path,
};
use anyhow::{Result, ensure};
use parfait_core::{Document, EditStateId};
use std::{
    collections::{HashMap, VecDeque},
    path::PathBuf,
    sync::{Arc, Condvar, Mutex},
    thread::{self, JoinHandle},
};
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct JobKey {
    pub document_id: [u8; 16],
    pub session_epoch: [u8; 16],
    pub request_id: u64,
}
#[derive(Clone)]
pub struct RecoveryRequest {
    pub key: JobKey,
    pub edit_state_id: EditStateId,
    pub snapshot: Document,
    pub source_path: Option<NativePath>,
    pub source_saved_state_id: Option<[u8; 16]>,
    pub source_saved_digest: Option<[u8; 32]>,
    pub source_head: Option<SnapshotHead>,
}
pub struct RecoveryEvent {
    pub key: JobKey,
    pub revision: u64,
    pub edit_state_id: EditStateId,
    pub result: Result<SnapshotHead, String>,
}
#[derive(Default)]
struct Queue {
    pending: HashMap<[u8; 16], RecoveryRequest>,
    order: VecDeque<[u8; 16]>,
    owners: HashMap<[u8; 16], [u8; 16]>,
    running: Option<RecoveryRequest>,
    events: HashMap<[u8; 16], RecoveryEvent>,
    last_request: HashMap<[u8; 16], u64>,
    closing: bool,
    alive: bool,
    sources: VecDeque<SourceRequest>,
    running_source: Option<SourceRequest>,
    source_events: VecDeque<SourceEvent>,
    priority_streak: usize,
    #[cfg(test)]
    digest_calculations: HashMap<[u8; 16], usize>,
}
impl Queue {
    fn owner(&mut self, key: JobKey) -> Result<()> {
        ensure!(
            self.owners.contains_key(&key.document_id) || self.owners.len() < 8,
            "Storage owner limit reached"
        );
        ensure!(
            self.owners
                .get(&key.document_id)
                .is_none_or(|epoch| *epoch == key.session_epoch),
            "Storage owner epoch changed without a barrier"
        );
        self.owners.insert(key.document_id, key.session_epoch);
        Ok(())
    }
    fn enqueue(&mut self, request: RecoveryRequest) -> Result<()> {
        self.owner(request.key)?;
        let id = request.key.document_id;
        ensure!(
            self.last_request
                .get(&id)
                .is_none_or(|last| request.key.request_id > *last),
            "Obsolete recovery request"
        );
        self.last_request.insert(id, request.key.request_id);
        if self.pending.insert(id, request).is_none() {
            self.order.push_back(id);
        }
        Ok(())
    }
    fn take_next(&mut self) -> Option<RecoveryRequest> {
        let id = self.order.pop_front()?;
        self.pending.remove(&id)
    }
    fn busy(&self) -> bool {
        !self.pending.is_empty()
            || self.running.is_some()
            || !self.sources.is_empty()
            || self.running_source.is_some()
    }
    fn choose_source(&self) -> bool {
        !self.sources.is_empty()
            && (self.priority_streak < 2 || self.pending.is_empty())
    }
}
struct WorkerGuard(Arc<(Mutex<Queue>, Condvar)>);
impl Drop for WorkerGuard {
    fn drop(&mut self) {
        let (lock, ready) = &*self.0;
        let mut q = lock.lock().unwrap_or_else(|p| p.into_inner());
        let mut abandoned: Vec<_> = q.pending.drain().map(|(_, r)| r).collect();
        if let Some(r) = q.running.take() {
            abandoned.push(r);
        }
        for r in abandoned {
            if q.events
                .get(&r.key.document_id)
                .is_none_or(|e| e.key.request_id < r.key.request_id)
            {
                q.events.insert(
                    r.key.document_id,
                    RecoveryEvent {
                        key: r.key,
                        revision: r.snapshot.revision,
                        edit_state_id: r.edit_state_id,
                        result: Err("Storage worker disconnected".into()),
                    },
                );
            }
        }
        let mut sources: Vec<_> = q.sources.drain(..).collect();
        if let Some(r) = q.running_source.take() {
            sources.push(r);
        }
        for request in sources {
            q.source_events.push_back(SourceEvent {
                request,
                result: Err(SourceFailure::Failed(
                    "Storage worker disconnected".into(),
                )),
            });
        }
        q.order.clear();
        q.alive = false;
        ready.notify_all();
    }
}
enum Work {
    Recovery(RecoveryRequest),
    Source(SourceRequest),
}
type DigestCache = HashMap<([u8; 16], [u8; 16]), (EditStateId, [u8; 32])>;
fn digest(
    cache: &mut DigestCache,
    key: JobKey,
    state: EditStateId,
    document: &Document,
) -> Result<([u8; 32], bool)> {
    let owner = (key.document_id, key.session_epoch);
    if let Some((_, digest)) = cache.get(&owner).filter(|(id, _)| *id == state)
    {
        return Ok((*digest, false));
    }
    let digest = document
        .art_digest_v1()
        .ok_or_else(|| anyhow::anyhow!("Invalid drawing"))?
        .0;
    cache.insert(owner, (state, digest));
    Ok((digest, true))
}
pub struct StorageCoordinator {
    queue: Arc<(Mutex<Queue>, Condvar)>,
    thread: Option<JoinHandle<()>>,
}
impl StorageCoordinator {
    pub fn start(directory: PathBuf) -> Result<Self> {
        let queue = Arc::new((
            Mutex::new(Queue { alive: true, ..Default::default() }),
            Condvar::new(),
        ));
        let worker_queue = queue.clone();
        let thread = thread::Builder::new()
            .name("parfait-storage".into())
            .spawn(move || {
                let _guard = WorkerGuard(worker_queue.clone());
                let mut cache = DigestCache::new();
                loop {
                    let (lock, ready) = &*worker_queue;
                    let mut q = lock.lock().unwrap_or_else(|p| p.into_inner());
                    while !q.busy() && !q.closing {
                        q = ready.wait(q).unwrap_or_else(|p| p.into_inner());
                    }
                    if !q.busy() && q.closing {
                        break;
                    }
                    let work = if q.choose_source() {
                        q.priority_streak += 1;
                        let r = q.sources.pop_front().unwrap();
                        q.running_source = Some(r.clone());
                        Work::Source(r)
                    } else {
                        q.priority_streak = 0;
                        let r = q.take_next().unwrap();
                        q.running = Some(r.clone());
                        Work::Recovery(r)
                    };
                    cache.retain(|(id, epoch), _| {
                        q.owners.get(id) == Some(epoch)
                    });
                    drop(q);
                    match work {
                        Work::Recovery(r) => {
                            #[cfg(test)]
                            if r.snapshot.name == "stage3-outer-panic-fixture" {
                                panic!("injected terminal worker failure");
                            }
                            let result = std::panic::catch_unwind(
                                std::panic::AssertUnwindSafe(
                                    || -> Result<SnapshotHead> {
                                        #[cfg(test)]
                                        if r.snapshot.name
                                            == "stage3-panic-fixture"
                                        {
                                            panic!("injected recovery panic");
                                        }
                                        let (hash, _computed) = digest(
                                            &mut cache,
                                            r.key,
                                            r.edit_state_id,
                                            &r.snapshot,
                                        )?;
                                        #[cfg(test)]
                                        if _computed {
                                            *lock
                                                .lock()
                                                .unwrap()
                                                .digest_calculations
                                                .entry(r.key.document_id)
                                                .or_default() += 1;
                                        }
                                        let metadata = RecoveryMetadata {
                                            document_id: r.key.document_id,
                                            edit_state_id: r
                                                .edit_state_id
                                                .bytes(),
                                            logical_digest_version: 1,
                                            logical_digest: hash,
                                            source_path: r.source_path.clone(),
                                            source_saved_state_id: r
                                                .source_saved_state_id,
                                            source_saved_digest: r
                                                .source_saved_digest,
                                            source_head: r.source_head.clone(),
                                        };
                                        let mut store =
                                            Store::open(&recovery_path(
                                                &directory,
                                                r.key.document_id,
                                            ))?;
                                        store.mark_open()?;
                                        store.save_recovery(
                                            &r.snapshot,
                                            &metadata,
                                        )
                                    },
                                ),
                            )
                            .map_err(|_| "Recovery worker panicked".to_owned())
                            .and_then(|r| r.map_err(|e| format!("{e:#}")));
                            let mut q =
                                lock.lock().unwrap_or_else(|p| p.into_inner());
                            q.running = None;
                            if q.owners.get(&r.key.document_id)
                                == Some(&r.key.session_epoch)
                            {
                                q.events.insert(
                                    r.key.document_id,
                                    RecoveryEvent {
                                        key: r.key,
                                        revision: r.snapshot.revision,
                                        edit_state_id: r.edit_state_id,
                                        result,
                                    },
                                );
                            }
                        }
                        Work::Source(r) => {
                            let result = std::panic::catch_unwind(
                                std::panic::AssertUnwindSafe(|| {
                                    let (hash, _) = digest(
                                        &mut cache,
                                        r.key,
                                        r.edit_state_id,
                                        &r.snapshot,
                                    )
                                    .map_err(|e| {
                                        SourceFailure::Failed(e.to_string())
                                    })?;
                                    crate::source::write_source(&r, hash)
                                }),
                            )
                            .unwrap_or_else(|_| {
                                Err(SourceFailure::Failed(
                                    "Original-save worker panicked".into(),
                                ))
                            });
                            let mut q =
                                lock.lock().unwrap_or_else(|p| p.into_inner());
                            q.running_source = None;
                            q.source_events
                                .push_back(SourceEvent { request: r, result });
                        }
                    }
                    ready.notify_all();
                }
            })?;
        Ok(Self { queue, thread: Some(thread) })
    }
    pub fn submit(&self, r: RecoveryRequest) -> Result<()> {
        let (lock, ready) = &*self.queue;
        let mut q = lock.lock().unwrap_or_else(|p| p.into_inner());
        ensure!(q.alive && !q.closing, "Storage worker is unavailable");
        q.enqueue(r)?;
        ready.notify_all();
        Ok(())
    }
    pub fn submit_source(&self, r: SourceRequest) -> Result<()> {
        let (lock, ready) = &*self.queue;
        let mut q = lock.lock().unwrap_or_else(|p| p.into_inner());
        ensure!(q.alive && !q.closing, "Storage worker is unavailable");
        ensure!(
            q.sources.len()
                + usize::from(q.running_source.is_some())
                + q.source_events.len()
                < 8,
            "Original-save queue is full"
        );
        ensure!(
            !q.sources
                .iter()
                .chain(q.running_source.iter())
                .any(|old| old.key.document_id == r.key.document_id),
            "An original save is already running for this document"
        );
        q.owner(r.key)?;
        q.sources.push_back(r);
        ready.notify_all();
        Ok(())
    }
    pub fn poll_source(&self) -> Option<SourceEvent> {
        self.queue
            .0
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .source_events
            .pop_front()
    }
    pub fn poll(&self) -> Option<RecoveryEvent> {
        let mut q = self.queue.0.lock().unwrap_or_else(|p| p.into_inner());
        let id = *q.events.keys().next()?;
        q.events.remove(&id)
    }
    pub fn snapshots(&self) -> Vec<Document> {
        let q = self.queue.0.lock().unwrap_or_else(|p| p.into_inner());
        q.pending
            .values()
            .chain(q.running.iter())
            .map(|r| r.snapshot.clone())
            .chain(
                q.sources
                    .iter()
                    .chain(q.running_source.iter())
                    .map(|r| r.snapshot.clone()),
            )
            .chain(q.source_events.iter().map(|e| e.request.snapshot.clone()))
            .collect()
    }
    pub fn available(&self) -> bool {
        let q = self.queue.0.lock().unwrap_or_else(|p| p.into_inner());
        q.alive && !q.closing
    }
    pub fn pending(&self) -> bool {
        self.queue.0.lock().unwrap_or_else(|p| p.into_inner()).busy()
    }
    pub fn barrier(&self) {
        let (lock, ready) = &*self.queue;
        let mut q = lock.lock().unwrap_or_else(|p| p.into_inner());
        while q.alive && q.busy() {
            q = ready.wait(q).unwrap_or_else(|p| p.into_inner());
        }
    }
    pub fn forget(&self, id: [u8; 16], epoch: [u8; 16]) {
        let (lock, ready) = &*self.queue;
        let mut q = lock.lock().unwrap_or_else(|p| p.into_inner());
        if q.owners.get(&id) != Some(&epoch) {
            return;
        }
        q.pending.remove(&id);
        q.order.retain(|other| *other != id);
        while q.alive
            && (q.running.as_ref().is_some_and(|r| r.key.document_id == id)
                || q.running_source
                    .as_ref()
                    .is_some_and(|r| r.key.document_id == id))
        {
            q = ready.wait(q).unwrap_or_else(|p| p.into_inner());
        }
        q.sources.retain(|r| r.key.document_id != id);
        q.source_events.retain(|e| e.request.key.document_id != id);
        q.events.remove(&id);
        q.owners.remove(&id);
        q.last_request.remove(&id);
    }
    pub fn finish(&mut self) {
        let (lock, ready) = &*self.queue;
        lock.lock().unwrap_or_else(|p| p.into_inner()).closing = true;
        ready.notify_all();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
impl Drop for StorageCoordinator {
    fn drop(&mut self) {
        self.finish();
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn request(id: u8, n: u64) -> RecoveryRequest {
        RecoveryRequest {
            key: JobKey {
                document_id: [id; 16],
                session_epoch: [1; 16],
                request_id: n,
            },
            edit_state_id: EditStateId::try_new().unwrap(),
            snapshot: Document::new(1, 1),
            source_path: None,
            source_saved_state_id: None,
            source_saved_digest: None,
            source_head: None,
        }
    }
    #[test]
    fn stage3_round_robin_bounds_selection_count_despite_repeated_a_edits() {
        let mut q = Queue::default();
        for id in 1..=8 {
            q.enqueue(request(id, 1)).unwrap();
        }
        for n in 2..=100 {
            q.enqueue(request(1, n)).unwrap();
        }
        assert_eq!(q.pending.len(), 8);
        assert_eq!(q.order.len(), 8);
        assert_eq!(q.take_next().unwrap().key.document_id, [1; 16]);
        q.enqueue(request(1, 101)).unwrap();
        for id in 2..=8 {
            assert_eq!(q.take_next().unwrap().key.document_id, [id; 16]);
            q.enqueue(request(1, 101 + u64::from(id))).unwrap();
        }
        assert_eq!(q.take_next().unwrap().key.document_id, [1; 16]);
        assert!(q.pending.is_empty());
    }
    #[test]
    fn stage3_worker_panic_is_a_routed_failure_and_other_documents_continue() {
        let dir = tempfile::tempdir().unwrap();
        let worker = StorageCoordinator::start(dir.path().to_owned()).unwrap();
        let mut bad = request(1, 1);
        bad.snapshot.name = "stage3-panic-fixture".into();
        worker.submit(bad).unwrap();
        worker.submit(request(2, 1)).unwrap();
        worker.barrier();
        let events = [worker.poll().unwrap(), worker.poll().unwrap()];
        assert!(events.iter().any(|e| e.key.document_id == [1; 16]
            && e.result.as_ref().is_err_and(|e| e.contains("panicked"))));
        assert!(
            events
                .iter()
                .any(|e| e.key.document_id == [2; 16] && e.result.is_ok())
        );
    }
}

#[cfg(test)]
mod cache_tests {
    use super::*;
    #[test]
    fn stage3_digest_is_reused_only_for_the_same_committed_owner_state() {
        let dir = tempfile::tempdir().unwrap();
        let worker = StorageCoordinator::start(dir.path().to_owned()).unwrap();
        let mut request = RecoveryRequest {
            key: JobKey {
                document_id: [1; 16],
                session_epoch: [1; 16],
                request_id: 1,
            },
            edit_state_id: EditStateId::try_new().unwrap(),
            snapshot: Document::new(1, 1),
            source_path: None,
            source_saved_state_id: None,
            source_saved_digest: None,
            source_head: None,
        };
        worker.submit(request.clone()).unwrap();
        worker.barrier();
        assert_eq!(
            worker.queue.0.lock().unwrap().digest_calculations[&[1; 16]],
            1
        );
        request.key.request_id += 1;
        request.snapshot.name = "New filename".into();
        request.snapshot.touch();
        worker.submit(request.clone()).unwrap();
        worker.barrier();
        assert_eq!(
            worker.queue.0.lock().unwrap().digest_calculations[&[1; 16]],
            1
        );
        request.key.request_id += 1;
        request.edit_state_id = EditStateId::try_new().unwrap();
        request.snapshot.layers[1].opacity = 0.5;
        request.snapshot.touch();
        worker.submit(request).unwrap();
        worker.barrier();
        assert_eq!(
            worker.queue.0.lock().unwrap().digest_calculations[&[1; 16]],
            2
        );
    }
}

#[cfg(test)]
mod terminal_tests {
    use super::*;
    #[test]
    fn stage3_terminal_worker_exit_releases_barriers_and_reports_abandoned_owner()
     {
        let dir = tempfile::tempdir().unwrap();
        let worker = StorageCoordinator::start(dir.path().to_owned()).unwrap();
        let mut document = Document::new(1, 1);
        document.name = "stage3-outer-panic-fixture".into();
        let request = RecoveryRequest {
            key: JobKey {
                document_id: [1; 16],
                session_epoch: [1; 16],
                request_id: 1,
            },
            edit_state_id: EditStateId::try_new().unwrap(),
            snapshot: document,
            source_path: None,
            source_saved_state_id: None,
            source_saved_digest: None,
            source_head: None,
        };
        worker.submit(request.clone()).unwrap();
        worker.barrier();
        assert!(!worker.available());
        assert!(!worker.pending());
        assert!(worker.snapshots().is_empty());
        assert!(worker.poll().unwrap().result.is_err());
        assert!(worker.submit(request).is_err());
    }
}

#[cfg(test)]
mod source_priority_tests {
    use super::*;
    #[test]
    fn stage4_original_priority_is_limited_to_two_before_waiting_recovery() {
        let mut q = Queue::default();
        for id in 1..=8u8 {
            q.enqueue(RecoveryRequest {
                key: JobKey {
                    document_id: [id; 16],
                    session_epoch: [1; 16],
                    request_id: 1,
                },
                edit_state_id: EditStateId::try_new().unwrap(),
                snapshot: Document::new(1, 1),
                source_path: None,
                source_saved_state_id: None,
                source_saved_digest: None,
                source_head: None,
            })
            .unwrap();
        }
        let source = SourceRequest {
            key: JobKey {
                document_id: [1; 16],
                session_epoch: [1; 16],
                request_id: 2,
            },
            binding_epoch: 0,
            path: PathBuf::from("unused"),
            snapshot: Document::new(1, 1),
            edit_state_id: EditStateId::try_new().unwrap(),
            expected_head: None,
        };
        q.sources.push_back(source);
        let mut choices = 0;
        for id in 1..=8u8 {
            while q.choose_source() {
                q.priority_streak += 1;
                choices += 1;
            }
            assert!(q.priority_streak <= 2);
            q.priority_streak = 0;
            assert_eq!(q.take_next().unwrap().key.document_id, [id; 16]);
            choices += 1;
        }
        assert!(choices <= 8 * 3);
    }
}
