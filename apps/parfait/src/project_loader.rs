//! One bounded worker. The UI submits the next request only after consuming this result.
use parfait_core::Document;
use std::{path::PathBuf, sync::mpsc, thread};

pub(crate) type RequestId = u64;
pub(crate) struct LoadResult {
    pub id: RequestId,
    pub result: Result<Document, String>,
    pub source_head: Option<parfait_storage::SnapshotHead>,
    pub digest: Option<[u8; 32]>,
    pub fell_back: bool,
    #[cfg(test)]
    pub thread: thread::ThreadId,
}
pub(crate) struct ProjectLoader {
    requests: Option<mpsc::SyncSender<(RequestId, PathBuf)>>,
    results: mpsc::Receiver<LoadResult>,
    worker: Option<thread::JoinHandle<()>>,
}
impl ProjectLoader {
    pub fn new() -> anyhow::Result<Self> {
        let (requests, receive) = mpsc::sync_channel::<(RequestId, PathBuf)>(1);
        let (send, results) = mpsc::sync_channel(1);
        let worker = thread::Builder::new()
            .name("parfait-project-loader".into())
            .spawn(move || {
                while let Ok((id, path)) = receive.recv() {
                    let mut source_head = None;
                    let mut digest = None;
                    let mut fell_back = false;
                    let result =
                        std::panic::catch_unwind(std::panic::AssertUnwindSafe(
                            || -> anyhow::Result<Document> {
                                let loaded =
                                    parfait_storage::Store::open_read_only(
                                        &path,
                                    )?
                                    .load()?
                                    .ok_or_else(|| {
                                        anyhow::anyhow!(
                                            "Project contains no snapshots"
                                        )
                                    })?;
                                source_head = Some(loaded.head);
                                fell_back = loaded.fell_back;
                                let document = loaded.document;
                                anyhow::ensure!(
                                    document.logical_pixel_bytes()
                                        <= 512 * 1024 * 1024,
                                    "Drawing exceeds pixel budget"
                                );
                                digest = document.art_digest_v1().map(|d| d.0);
                                Ok(document)
                            },
                        ))
                        .map_err(|_| "Project reader panicked".to_owned())
                        .and_then(|r| r.map_err(|e| format!("{e:#}")));
                    if send
                        .send(LoadResult {
                            id,
                            result,
                            source_head,
                            digest,
                            fell_back,
                            #[cfg(test)]
                            thread: thread::current().id(),
                        })
                        .is_err()
                    {
                        break;
                    }
                }
            })?;
        Ok(Self { requests: Some(requests), results, worker: Some(worker) })
    }
    pub fn submit(&self, id: RequestId, path: PathBuf) -> anyhow::Result<()> {
        self.requests
            .as_ref()
            .unwrap()
            .try_send((id, path))
            .map_err(|e| anyhow::anyhow!("Project reader unavailable: {e}"))
    }
    #[cfg(test)]
    pub fn wait(&self) -> LoadResult {
        self.results.recv_timeout(std::time::Duration::from_secs(10)).unwrap()
    }
    pub fn poll(&self) -> Result<Option<LoadResult>, mpsc::TryRecvError> {
        match self.results.try_recv() {
            Ok(result) => Ok(Some(result)),
            Err(mpsc::TryRecvError::Empty) => Ok(None),
            Err(e) => Err(e),
        }
    }
}
impl Drop for ProjectLoader {
    fn drop(&mut self) {
        self.requests.take();
        // Drain before joining so a result cannot keep the bounded worker blocked.
        while self.results.recv().is_ok() {}
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
