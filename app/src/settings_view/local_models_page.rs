//! Local Models Settings Page UI Component
//!
//! Provides the settings interface for configuring local LLM providers.
//! Users can:
//! - Enable/disable local models
//! - Select provider (Ollama or LMStudio)
//! - Configure API endpoints
//! - Select active model
//! - Test connection

use warpui::{
    elements::{Container, Flex, ParentElement, Text},
    ViewContext, ViewHandle, Action,
};

use crate::menu::{MenuItem, MenuItemFields};
use crate::view_components::{ActionButton, Dropdown, DropdownItem, SingleLineEditor};
use crate::appearance::Appearance;

use warp_ai::local_models::{LocalModelProvider, ModelInfo};

/// Connection status indicator
#[derive(Clone, Debug, PartialEq)]
pub enum ConnectionStatus {
    Disconnected,
    Testing,
    Connected,
    Failed(String),
}

impl ConnectionStatus {
    pub fn display_text(&self) -> String {
        match self {
            Self::Disconnected => "Disconnected".to_string(),
            Self::Testing => "Testing...".to_string(),
            Self::Connected => "✓ Connected".to_string(),
            Self::Failed(err) => format!("✗ Failed: {}", err),
        }
    }

    pub fn is_connected(&self) -> bool {
        matches!(self, Self::Connected)
    }
}

/// Local Models Settings Page View
pub struct LocalModelsSettingsPageView {
    // Provider selection
    provider_dropdown: ViewHandle<Dropdown<LocalModelsAction>>,
    
    // URL configuration
    ollama_url_input: ViewHandle<SingleLineEditor>,
    lmstudio_url_input: ViewHandle<SingleLineEditor>,
    
    // Model selection
    model_dropdown: ViewHandle<Dropdown<LocalModelsAction>>,
    
    // Action buttons
    test_connection_button: ViewHandle<ActionButton>,
    refresh_models_button: ViewHandle<ActionButton>,
    
    // State
    connection_status: ConnectionStatus,
    available_models: Vec<ModelInfo>,
    selected_provider: LocalModelProvider,
    selected_model: Option<String>,
    
    // UI state
    is_testing: bool,
    show_advanced: bool,
}

/// Actions for Local Models Settings
#[derive(Clone, Debug, PartialEq)]
pub enum LocalModelsAction {
    SetProvider(LocalModelProvider),
    UpdateOllamaUrl(String),
    UpdateLMStudioUrl(String),
    SelectModel(String),
    TestConnection,
    RefreshModels,
    ToggleAdvanced,
}

impl Action for LocalModelsAction {}

impl LocalModelsSettingsPageView {
    /// Create a new Local Models Settings page view
    pub fn new(ctx: &mut ViewContext<Self>) -> Self {
        let provider_dropdown = ctx.add_typed_action_view(|ctx| {
            let mut dropdown = Dropdown::new(ctx);
            
            // Add provider options
            let items = vec![
                DropdownItem::new(
                    "Disabled".to_string(),
                    LocalModelsAction::SetProvider(LocalModelProvider::None),
                ),
                DropdownItem::new(
                    "Ollama".to_string(),
                    LocalModelsAction::SetProvider(LocalModelProvider::Ollama),
                ),
                DropdownItem::new(
                    "LMStudio".to_string(),
                    LocalModelsAction::SetProvider(LocalModelProvider::LMStudio),
                ),
            ];
            
            dropdown.add_items(items, ctx);
            dropdown
        });

        let ollama_url_input = ctx.add_typed_action_view(|ctx| {
            let editor = SingleLineEditor::new(ctx);
            editor
        });

        let lmstudio_url_input = ctx.add_typed_action_view(|ctx| {
            let editor = SingleLineEditor::new(ctx);
            editor
        });

        let model_dropdown = ctx.add_typed_action_view(|ctx| {
            Dropdown::new(ctx)
        });

        let test_connection_button = ctx.add_typed_action_view(|ctx| {
            ActionButton::new(
                "Test Connection",
                LocalModelsAction::TestConnection,
            )
        });

        let refresh_models_button = ctx.add_typed_action_view(|ctx| {
            ActionButton::new(
                "Refresh Models",
                LocalModelsAction::RefreshModels,
            )
        });

        Self {
            provider_dropdown,
            ollama_url_input,
            lmstudio_url_input,
            model_dropdown,
            test_connection_button,
            refresh_models_button,
            connection_status: ConnectionStatus::Disconnected,
            available_models: Vec::new(),
            selected_provider: LocalModelProvider::None,
            selected_model: None,
            is_testing: false,
            show_advanced: false,
        }
    }

    /// Update the model list dropdown
    pub fn update_model_dropdown(&self, models: &[ModelInfo], ctx: &mut ViewContext<Self>) {
        let items: Vec<DropdownItem<LocalModelsAction>> = models
            .iter()
            .map(|model| {
                DropdownItem::new(
                    model.name.clone(),
                    LocalModelsAction::SelectModel(model.name.clone()),
                )
            })
            .collect();

        self.model_dropdown.as_ref(ctx).update(ctx, |dropdown, ctx| {
            dropdown.set_items(items, ctx);
        });
    }

    /// Handle provider change
    pub fn handle_provider_change(&mut self, provider: LocalModelProvider) {
        self.selected_provider = provider;
        self.connection_status = ConnectionStatus::Disconnected;
        self.available_models.clear();
    }

    /// Handle model selection
    pub fn handle_model_selection(&mut self, model: String) {
        self.selected_model = Some(model);
    }

    /// Update connection status
    pub fn set_connection_status(&mut self, status: ConnectionStatus) {
        self.connection_status = status;
    }

    /// Get current provider configuration
    pub fn get_provider_config(&self) -> Option<(LocalModelProvider, String)> {
        match self.selected_provider {
            LocalModelProvider::None => None,
            LocalModelProvider::Ollama => {
                Some((LocalModelProvider::Ollama, "http://localhost:11434".to_string()))
            }
            LocalModelProvider::LMStudio => {
                Some((LocalModelProvider::LMStudio, "http://localhost:1234".to_string()))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_connection_status_display() {
        assert_eq!(ConnectionStatus::Disconnected.display_text(), "Disconnected");
        assert_eq!(ConnectionStatus::Testing.display_text(), "Testing...");
        assert_eq!(ConnectionStatus::Connected.display_text(), "✓ Connected");
        assert!(ConnectionStatus::Failed("timeout".to_string())
            .display_text()
            .contains("Failed"));
    }

    #[test]
    fn test_connection_status_is_connected() {
        assert!(!ConnectionStatus::Disconnected.is_connected());
        assert!(ConnectionStatus::Connected.is_connected());
        assert!(!ConnectionStatus::Testing.is_connected());
    }

    #[test]
    fn test_local_models_action_clone() {
        let action = LocalModelsAction::SetProvider(LocalModelProvider::Ollama);
        let cloned = action.clone();
        assert_eq!(action, cloned);
    }
}
