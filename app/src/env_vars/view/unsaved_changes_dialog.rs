use warp_core::ui::appearance::Appearance;
use warp_i18n::tr;
use warpui::{
    elements::{Container, MouseStateHandle},
    fonts::Weight,
    platform::Cursor,
    ui_components::{
        button::ButtonVariant,
        components::{Coords, UiComponent, UiComponentStyles},
    },
    Element,
};

use crate::ui_components::dialog::{dialog_styles, Dialog};

use super::env_var_collection::{EnvVarCollectionAction, EnvVarCollectionView};

const BUTTON_FONT_SIZE: f32 = 14.;
const BUTTON_PADDING: f32 = 12.;
const MODAL_HORIZONTAL_MARGIN: f32 = 28.;
const DIALOG_WIDTH: f32 = 460.;

impl EnvVarCollectionView {
    pub fn render_unsaved_changes_dialog_button(
        &self,
        appearance: &Appearance,
        button_mouse_state: MouseStateHandle,
        action: EnvVarCollectionAction,
        text: &str,
    ) -> Box<dyn Element> {
        appearance
            .ui_builder()
            .button(ButtonVariant::Secondary, button_mouse_state)
            .with_style(UiComponentStyles {
                font_size: Some(BUTTON_FONT_SIZE),
                font_weight: Some(Weight::Bold),
                padding: Some(Coords::uniform(BUTTON_PADDING)),
                ..Default::default()
            })
            .with_text_label(text.into())
            .build()
            .with_cursor(Cursor::PointingHand)
            .on_click(move |ctx, _, _| ctx.dispatch_typed_action(action.clone()))
            .finish()
    }

    pub fn render_unsaved_changes_dialog(&self, appearance: &Appearance) -> Box<dyn Element> {
        let keep_editing_button = self.render_unsaved_changes_dialog_button(
            appearance,
            self.button_mouse_states.keep_editing_state.clone(),
            EnvVarCollectionAction::CloseUnsavedChangesDialog,
            tr!("dialog-button-keep-editing").as_ref(),
        );

        let discard_changes_button = self.render_unsaved_changes_dialog_button(
            appearance,
            self.button_mouse_states.discard_changes_state.clone(),
            EnvVarCollectionAction::ForceClose,
            tr!("dialog-button-discard-changes").as_ref(),
        );

        Container::new(
            Dialog::new(
                tr!("dialog-unsaved-changes-title").to_string(),
                None,
                dialog_styles(appearance),
            )
            .with_bottom_row_child(keep_editing_button)
            .with_bottom_row_child(discard_changes_button)
            .with_width(DIALOG_WIDTH)
            .build()
            .finish(),
        )
        .with_margin_left(MODAL_HORIZONTAL_MARGIN)
        .with_margin_right(MODAL_HORIZONTAL_MARGIN)
        .finish()
    }
}
