//! Shared UI-thread publication of fully preflighted native buffer edits.
//! This function does not decide permission, ownership, freshness or persistence.
//! Callers must finish those checks and staging before entering this callback.
use std::{collections::BTreeMap, rc::Rc};
use floem::reactive::{SignalUpdate, SignalWith};
use crate::doc::Doc;
use lapce_core::buffer::{Buffer, InvalLines};
use lapce_xi_rope::{Rope, RopeDelta};

pub(crate) type StagedBuffers<K> = BTreeMap<K, (Buffer, (Rope, RopeDelta, InvalLines))>;

pub(crate) fn commit_buffers<K: Ord>(docs: &BTreeMap<K, Rc<Doc>>, mut staged: StagedBuffers<K>) {
    floem::reactive::batch(|| {
        // Publish every source before notifying dependent views or the proxy.
        // There is no await, save or file creation in this publication step.
        for (path, (buffer, _)) in &mut staged {
            docs[path].buffer.with_untracked(|live| buffer.adopt_revision_clock(live));
        }
        for (path, (buffer, _)) in &staged {
            docs[path].buffer.set(buffer.clone());
        }
        for (path, (_, delta)) in staged {
            docs[&path].apply_deltas(&[delta]);
        }
    });
}
