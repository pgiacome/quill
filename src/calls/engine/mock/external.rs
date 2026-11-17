//! External frames on the mock engine: a sender per connected call that
//! records what a feeder pushed, for capture and driver tests.
use super::*;

/// One recorded external frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MockExternalFrame {
    pub target: CaptureTarget,
    pub device: ExternalDevice,
    pub bytes: Vec<u8>,
    pub meta: ExternalFrameMeta,
}

struct MockFrameSender {
    inner: Arc<Mutex<MockInner>>,
    frame_pushed: Arc<std::sync::Condvar>,
    target: CaptureTarget,
}

impl ExternalFrameSender for MockFrameSender {
    fn send(
        &self,
        device: ExternalDevice,
        data: &[u8],
        meta: ExternalFrameMeta,
    ) -> Result<(), EngineError> {
        if !external_frame_valid(device, data.len(), meta) {
            return Err(EngineError::Engine {
                op: "qc_external_frame",
                code: -1,
            });
        }
        let mut inner = self.inner.lock_or_recover();
        if !inner.available {
            return Err(EngineError::Unavailable);
        }
        inner.external_frames.push(MockExternalFrame {
            target: self.target,
            device,
            bytes: data.to_vec(),
            meta,
        });
        drop(inner);
        self.frame_pushed.notify_all();
        Ok(())
    }
}

impl MockEngine {
    /// Every external frame pushed so far, oldest first.
    pub fn external_frames(&self) -> Vec<MockExternalFrame> {
        self.inner.lock_or_recover().external_frames.clone()
    }

    /// Block until at least `count` external frames were pushed, then
    /// return them all. Tests: the feeder thread is the one pushing.
    pub fn wait_external_frames(&self, count: usize) -> Vec<MockExternalFrame> {
        let inner = self.inner.lock_or_recover();
        let inner = self
            .frame_pushed
            .wait_while(inner, |inner| inner.external_frames.len() < count)
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        inner.external_frames.clone()
    }

    /// A sender once the call's sources exist: a connected 1:1 call or a
    /// connected group call, as on the native engine.
    pub(super) fn mock_external_sender(
        &self,
        target: CaptureTarget,
    ) -> Option<Arc<dyn ExternalFrameSender>> {
        let inner = self.inner.lock_or_recover();
        if !inner.available {
            return None;
        }
        let ready = match target {
            CaptureTarget::Private(call_id) => {
                inner.calls.contains_key(&call_id)
                    && inner.connects.iter().any(|(id, _)| *id == call_id)
            }
            CaptureTarget::Group(group_call_id) => {
                inner
                    .group_connects
                    .iter()
                    .any(|(id, _, _)| *id == group_call_id)
                    && !inner.group_leaves.contains(&group_call_id)
            }
        };
        drop(inner);
        ready.then(|| {
            Arc::new(MockFrameSender {
                inner: self.inner.clone(),
                frame_pushed: self.frame_pushed.clone(),
                target,
            }) as Arc<dyn ExternalFrameSender>
        })
    }
}
