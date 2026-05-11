use super::HandoffComposeState;
use crate::server::ids::{ClientId, SyncId};
use warpui::App;

#[test]
fn preserves_explicit_environment_selection() {
    App::test((), |mut app| async move {
        let state = app.add_model(|_| HandoffComposeState::default());
        let default_environment_id = SyncId::ClientId(ClientId::new());
        let explicit_environment_id = SyncId::ClientId(ClientId::new());

        state.update(&mut app, |state, ctx| {
            state.activate(ctx);
            state.ensure_default_environment_id(default_environment_id, ctx);
        });
        state.read(&app, |state, _| {
            assert_eq!(
                state.selected_environment_id(),
                Some(&default_environment_id)
            );
            assert_eq!(state.explicit_environment_id(), None);
        });

        state.update(&mut app, |state, ctx| {
            state.set_environment_id(Some(explicit_environment_id), true, ctx);
            state.ensure_default_environment_id(default_environment_id, ctx);
        });
        state.read(&app, |state, _| {
            assert_eq!(
                state.selected_environment_id(),
                Some(&explicit_environment_id)
            );
            assert_eq!(
                state.explicit_environment_id(),
                Some(explicit_environment_id)
            );
        });
    });
}
