use std::collections::HashMap;

use crate::{
    elements::ChildView, AppContext, Element, Entity, EntityId, TypedActionView, View, ViewContext,
    ViewHandle, WindowId,
};

use super::view_tree_debug_view::ViewTreeDebugView;

/// A root view for a window that provides debugging tools for the UI framework.
pub(crate) struct DebugRootView {
    child: ViewHandle<ViewTreeDebugView>,
}

impl TypedActionView for DebugRootView {
    type Action = ();
}

impl DebugRootView {
    pub fn new(
        target_window_id: WindowId,
        view_parent_map: HashMap<EntityId, EntityId>,
        root_view_id: EntityId,
        ctx: &mut ViewContext<Self>,
    ) -> Self {
        let child = ctx.add_typed_action_view(|ctx| {
            ViewTreeDebugView::new(target_window_id, view_parent_map, root_view_id, ctx)
        });
        Self { child }
    }
}

impl Entity for DebugRootView {
    type Event = ();
}

impl View for DebugRootView {
    fn ui_name() -> &'static str {
        "DebugRootView"
    }

    fn render(&self, _app: &AppContext) -> Box<dyn Element> {
        ChildView::new(&self.child).finish()
    }
}
