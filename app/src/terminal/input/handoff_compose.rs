//! Tracks the `&` prefix mode drafting state in the local input while the user
//! writes a cloud handoff prompt, before a cloud pane/model exists.

use std::sync::atomic::{AtomicU64, Ordering};

use crate::server::ids::SyncId;
use warpui::{Entity, ModelContext};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HandoffLaunchRequestId(u64);

impl HandoffLaunchRequestId {
    pub(crate) fn new() -> Self {
        static NEXT_ID: AtomicU64 = AtomicU64::new(1);
        Self(NEXT_ID.fetch_add(1, Ordering::Relaxed))
    }
}

#[derive(Clone)]
pub enum HandoffComposeStateEvent {
    ActiveChanged,
    EnvironmentSelected,
    RequestChanged,
}

/// Transient state owned by the local input while drafting a cloud handoff
/// prompt (the `&` prefix mode), before a cloud pane exists.
#[derive(Default)]
pub struct HandoffComposeState {
    active: bool,
    selected_environment_id: Option<SyncId>,
    has_explicit_environment_selection: bool,
    active_request_id: Option<HandoffLaunchRequestId>,
}

impl HandoffComposeState {
    pub(crate) fn is_active(&self) -> bool {
        self.active
    }

    pub(crate) fn activate(&mut self, ctx: &mut ModelContext<Self>) {
        self.active = true;
        self.active_request_id = None;
        self.has_explicit_environment_selection = false;
        ctx.emit(HandoffComposeStateEvent::ActiveChanged);
    }

    pub(crate) fn exit(&mut self, ctx: &mut ModelContext<Self>) {
        if !self.active
            && self.active_request_id.is_none()
            && !self.has_explicit_environment_selection
        {
            return;
        }

        self.active = false;
        self.active_request_id = None;
        self.has_explicit_environment_selection = false;
        ctx.emit(HandoffComposeStateEvent::ActiveChanged);
    }

    pub(crate) fn selected_environment_id(&self) -> Option<&SyncId> {
        self.selected_environment_id.as_ref()
    }

    pub(crate) fn explicit_environment_id(&self) -> Option<SyncId> {
        self.has_explicit_environment_selection
            .then_some(self.selected_environment_id)
            .flatten()
    }

    pub(crate) fn set_environment_id(
        &mut self,
        environment_id: Option<SyncId>,
        is_explicit: bool,
        ctx: &mut ModelContext<Self>,
    ) {
        if self.selected_environment_id == environment_id
            && (!is_explicit || self.has_explicit_environment_selection)
        {
            return;
        }

        self.selected_environment_id = environment_id;
        if is_explicit {
            self.has_explicit_environment_selection = true;
        }
        ctx.emit(HandoffComposeStateEvent::EnvironmentSelected);
    }

    pub(crate) fn ensure_default_environment_id(
        &mut self,
        environment_id: SyncId,
        ctx: &mut ModelContext<Self>,
    ) {
        if self.selected_environment_id.is_none() {
            self.set_environment_id(Some(environment_id), false, ctx);
        }
    }

    pub(crate) fn set_active_request_id(
        &mut self,
        request_id: HandoffLaunchRequestId,
        ctx: &mut ModelContext<Self>,
    ) {
        self.active_request_id = Some(request_id);
        ctx.emit(HandoffComposeStateEvent::RequestChanged);
    }

    pub(crate) fn take_request(
        &mut self,
        request_id: HandoffLaunchRequestId,
        ctx: &mut ModelContext<Self>,
    ) -> bool {
        if self.active_request_id != Some(request_id) {
            return false;
        }

        self.exit(ctx);
        true
    }
}

impl Entity for HandoffComposeState {
    type Event = HandoffComposeStateEvent;
}

#[cfg(test)]
#[path = "handoff_compose_tests.rs"]
mod tests;
