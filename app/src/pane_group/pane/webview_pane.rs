use std::cell::RefCell;

use warpui::{AppContext, ModelHandle, View, ViewContext, ViewHandle};

use crate::app_state::{LeafContents, WebViewPaneSnapshot};
use crate::pane_group::pane::webview_view::WebViewView;

use super::{
    view::PaneView, BackingView, DetachType, PaneConfiguration, PaneContent, PaneGroup, PaneId, ShareableLink,
    ShareableLinkError,
};

pub struct WebViewPane {
    view: ViewHandle<PaneView<WebViewView>>,
    pane_configuration: ModelHandle<PaneConfiguration>,
    url: String,
    title: String,
    #[cfg(target_os = "macos")]
    native_webview: RefCell<Option<warpui::platform::mac::webview::HeliosWebView>>,
}

impl WebViewPane {
    pub fn new<V: View>(url: String, title: String, ctx: &mut ViewContext<V>) -> Self {
        let webview_view = ctx.add_typed_action_view(|ctx| WebViewView::new(title.clone(), url.clone(), ctx));
        let pane_configuration = webview_view.as_ref(ctx).pane_configuration();
        let pane_view = ctx.add_typed_action_view(|ctx| {
            let pane_id = PaneId::from_webview_pane_ctx(ctx);
            PaneView::new(
                pane_id,
                webview_view,
                (),
                pane_configuration.clone(),
                ctx,
            )
        });
        Self {
            view: pane_view,
            pane_configuration,
            url,
            title,
            #[cfg(target_os = "macos")]
            native_webview: RefCell::new(None),
        }
    }
}

impl PaneContent for WebViewPane {
    fn id(&self) -> PaneId {
        PaneId::from_webview_pane_view(&self.view)
    }

    fn attach(
        &self,
        _group: &PaneGroup,
        focus_handle: crate::pane_group::focus_state::PaneFocusHandle,
        ctx: &mut ViewContext<PaneGroup>,
    ) {
        self.view
            .update(ctx, |view, ctx| view.set_focus_handle(focus_handle, ctx));
        let child = self.view.as_ref(ctx).child(ctx);
        let pane_id = self.id();
        ctx.subscribe_to_view(&child, move |pane_group, _, event, ctx| {
            pane_group.handle_pane_event(pane_id, event, ctx);
        });

        // Create native WKWebView and add to window content view
        #[cfg(target_os = "macos")]
        {
            use cocoa::appkit::NSView;
            use cocoa::foundation::NSRect;
            use warpui::platform::mac::webview::HeliosWebView;
            use warpui::platform::mac::content_view_from_platform_window;

            let window_id = ctx.window_id();
            if let Some(platform_window) = ctx.windows().platform_window(window_id) {
                if let Some(content_view) = content_view_from_platform_window(platform_window.as_ref()) {
                    // Use the content view's full bounds as the initial frame so the
                    // webview fills the window immediately (autoresize handles future resizes).
                    let frame: NSRect = unsafe { NSView::bounds(content_view) };
                    let webview = HeliosWebView::new(frame, Some(&self.url), None, std::ptr::null_mut());
                    webview.add_to_view(content_view);
                    webview.set_autoresize();
                    *self.native_webview.borrow_mut() = Some(webview);
                }
            }
        }
    }

    fn detach(
        &self,
        _group: &PaneGroup,
        _detach_type: DetachType,
        ctx: &mut ViewContext<PaneGroup>,
    ) {
        let child = self.view.as_ref(ctx).child(ctx);
        ctx.unsubscribe_to_view(&child);

        // Remove native webview (Drop calls helios_webview_release)
        #[cfg(target_os = "macos")]
        {
            *self.native_webview.borrow_mut() = None;
        }
    }

    fn snapshot(&self, _ctx: &AppContext) -> LeafContents {
        LeafContents::WebView(WebViewPaneSnapshot {
            url: self.url.clone(),
            title: self.title.clone(),
        })
    }

    fn has_application_focus(&self, ctx: &mut ViewContext<PaneGroup>) -> bool {
        self.view.is_self_or_child_focused(ctx)
    }

    fn focus(&self, ctx: &mut ViewContext<PaneGroup>) {
        self.view.as_ref(ctx).child(ctx)
            .update(ctx, |view, ctx| view.focus_contents(ctx));

        // Also make the native WKWebView first responder so it receives keyboard input
        #[cfg(target_os = "macos")]
        {
            if let Some(ref webview) = *self.native_webview.borrow() {
                let native = webview.native_id();
                unsafe {
                    use objc::{msg_send, sel, sel_impl};
                    let window: cocoa::base::id = msg_send![native, window];
                    if !window.is_null() {
                        let _: () = msg_send![window, makeFirstResponder: native];
                    }
                }
            }
        }
    }

    fn shareable_link(
        &self,
        _ctx: &mut ViewContext<PaneGroup>,
    ) -> Result<ShareableLink, ShareableLinkError> {
        Ok(ShareableLink::Base)
    }

    fn pane_configuration(&self) -> ModelHandle<PaneConfiguration> {
        self.pane_configuration.clone()
    }

    fn is_pane_being_dragged(&self, ctx: &AppContext) -> bool {
        self.view.as_ref(ctx).is_being_dragged()
    }
}

impl WebViewPane {
    #[cfg(target_os = "macos")]
    pub fn evaluate_javascript(&self, js: &str) {
        if let Some(ref webview) = *self.native_webview.borrow() {
            webview.eval_js(js);
        }
    }
}
