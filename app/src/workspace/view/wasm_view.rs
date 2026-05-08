//! WASM-only view functions for the Workspace.

use warpui::{ViewContext, ViewHandle};

use crate::uri::browser_url_handler::parse_current_url;

use crate::view_components::action_button::{ActionButton, PrimaryTheme};
use crate::wasm_nux_dialog::{WasmNUXDialog, WasmNUXDialogEvent};
use crate::workspace::action::WorkspaceAction;
use crate::workspace::view::Workspace;

impl Workspace {
    pub(super) fn build_wasm_nux_dialog(ctx: &mut ViewContext<Self>) -> ViewHandle<WasmNUXDialog> {
        let wasm_nux_dialog = ctx.add_typed_action_view(|_| WasmNUXDialog::new());
        ctx.subscribe_to_view(&wasm_nux_dialog, |me, _, event, ctx| match event {
            WasmNUXDialogEvent::Close => {
                me.show_wasm_nux_dialog = false;
                ctx.notify();
            }
        });
        wasm_nux_dialog
    }

    pub(super) fn build_open_in_warp_button(
        ctx: &mut ViewContext<Self>,
    ) -> ViewHandle<ActionButton> {
        ctx.add_typed_action_view(|_ctx| {
            ActionButton::new("Open in Warper", PrimaryTheme).on_click(move |ctx| {
                // Get the current URL and dispatch action to open it on desktop
                if let Some(url) = parse_current_url() {
                    ctx.dispatch_typed_action(WorkspaceAction::OpenLinkOnDesktop(url));
                } else {
                    log::warn!("Could not get URL for Open in Warper button");
                }
            })
        })
    }
}
