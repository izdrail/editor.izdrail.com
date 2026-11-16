use floem::{
    peniko::kurbo::Rect,
    reactive::{RwSignal, Scope, SignalGet, SignalUpdate},
    views::editor::id::EditorId,
};

use crate::markdown::MarkdownContent;

#[derive(Clone)]
pub struct HoverData {
    pub active: RwSignal<bool>,
    pub offset: RwSignal<usize>,
    pub editor_id: RwSignal<EditorId>,
    pub content: RwSignal<Vec<MarkdownContent>>,
    pub layout_rect: RwSignal<Rect>,
    /// Id of the newest hover request issued. A response carrying an older
    /// id is a stale result and must not replace the visible hover.
    pub request_id: RwSignal<usize>,
}

impl HoverData {
    pub fn new(cx: Scope) -> Self {
        Self {
            active: cx.create_rw_signal(false),
            offset: cx.create_rw_signal(0),
            content: cx.create_rw_signal(Vec::new()),
            editor_id: cx.create_rw_signal(EditorId::next()),
            layout_rect: cx.create_rw_signal(Rect::ZERO),
            request_id: cx.create_rw_signal(0),
        }
    }

    /// Issue the id for a new hover request, superseding every in-flight
    /// request so their late responses are dropped.
    pub fn next_request_id(&self) -> usize {
        let id = self.request_id.get_untracked() + 1;
        self.request_id.set(id);
        id
    }

    /// Hide the current tooltip and reject late responses from its source.
    pub fn dismiss(&self) {
        self.active.set(false);
        self.invalidate_requests();
    }

    /// Supersede every in-flight request without issuing a new one, so a
    /// late response cannot bring back a hover the user already left.
    pub fn invalidate_requests(&self) {
        self.request_id.update(|id| *id += 1);
    }
}

/// Whether the response to request `request_id` may be shown when
/// `latest_id` is the newest request issued: only the newest request's own
/// response. Anything older is a stale result from a superseded hover.
pub fn response_is_current(request_id: usize, latest_id: usize) -> bool {
    request_id == latest_id
}

#[cfg(test)]
mod tests {
    use floem::reactive::{Scope, SignalGet, SignalUpdate};

    use super::{response_is_current, HoverData};

    #[test]
    fn newest_response_is_current() {
        assert!(response_is_current(3, 3));
        assert!(!response_is_current(2, 3));
        assert!(!response_is_current(4, 3));
    }

    #[test]
    fn superseded_request_is_stale() {
        let cx = Scope::new();
        let hover = HoverData::new(cx);
        let first = hover.next_request_id();
        let second = hover.next_request_id();
        assert_ne!(first, second);
        assert!(!response_is_current(first, hover.request_id.get_untracked()));
        assert!(response_is_current(second, hover.request_id.get_untracked()));
        cx.dispose();
    }

    #[test]
    fn dismiss_hides_tooltip_and_rejects_its_pending_response() {
        let cx = Scope::new();
        let hover = HoverData::new(cx);
        hover.active.set(true);
        let pending = hover.next_request_id();
        hover.dismiss();
        assert!(!hover.active.get_untracked());
        assert!(!response_is_current(pending, hover.request_id.get_untracked()));
        let fresh = hover.next_request_id();
        assert!(response_is_current(fresh, hover.request_id.get_untracked()));
        cx.dispose();
    }

    #[test]
    fn invalidation_drops_in_flight_requests() {
        let cx = Scope::new();
        let hover = HoverData::new(cx);
        let pending = hover.next_request_id();
        hover.invalidate_requests();
        assert!(!response_is_current(pending, hover.request_id.get_untracked()));
        cx.dispose();
    }
}

/// Freeze both coordinate spaces for a hover response. Tooltip placement is
/// display-relative; the requested text and LSP position remain source-relative.
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub(crate) struct SourceHoverStamp {
    pub origin_id:u64,
    pub origin_revision:u64,
    pub source_id:u64,
    pub source_revision:u64,
    pub source_offset:usize,
    pub cursor_offset:usize,
}
#[cfg(test)]
mod source_stamp_tests {
    use super::*;
    #[test] fn source_origin_and_caret_changes_each_invalidate_response() {
        let current=SourceHoverStamp{origin_id:1,origin_revision:2,source_id:3,source_revision:4,source_offset:12345,cursor_offset:80};
        assert_eq!(current,current);
        for changed in [SourceHoverStamp{origin_id:9,..current},SourceHoverStamp{origin_revision:9,..current},SourceHoverStamp{source_id:9,..current},SourceHoverStamp{source_revision:9,..current},SourceHoverStamp{source_offset:9,..current},SourceHoverStamp{cursor_offset:9,..current}] {assert_ne!(current,changed);}
    }
}
