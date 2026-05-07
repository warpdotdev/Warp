# Integration Checklist for Phase 2+

## Phase 2: Settings Integration

### Files to Modify

1. **`app/src/settings/mod.rs`**
   - Export new settings module
   ```rust
   pub mod local_models;
   pub use local_models::*;
   ```

2. **`app/src/settings/local_models.rs`** (NEW)
   ```rust
   use warp_core::settings::macros::define_settings_group;
   use warp_ai::local_models::{LocalModelConfig, LocalModelProvider};

   #[derive(Clone, Debug, Serialize, Deserialize)]
   pub struct LocalModelSettings {
       pub enabled: bool,
       pub provider: LocalModelProvider,
       // ... individual provider configs
   }
   ```

3. **`app/src/settings/init.rs`**
   - Register settings:
   ```rust
   LocalModelSettings::register(ctx);
   ```

### Settings UI Component

Create `app/src/settings_view/local_models_page.rs`:

```rust
pub struct LocalModelsSettingsPageView {
    provider_dropdown: ViewHandle<Dropdown<LocalModelsAction>>,
    url_input: ViewHandle<SingleLineEditor>,
    model_selector: ViewHandle<FilterableDropdown<ModelInfo>>,
    test_button: ViewHandle<ActionButton>,
    connection_status: ConnectionStatus,
}

pub enum LocalModelsAction {
    SetProvider(LocalModelProvider),
    UpdateUrl(String),
    SelectModel(String),
    TestConnection,
    RefreshModels,
}
```

## Phase 3: Agent Integration

### Files to Modify

1. **`app/src/ai/execution_profiles/model_provider_router.rs`**
   - Add local models to model selection
   - Route to appropriate client based on selection

2. **`app/src/ai/llms.rs`**
   - Add `LocalModel` to LLM enum
   - Implement routing logic

3. **`app/src/ai/agent/api.rs`**
   - Support local models in requests
   - Add provider detection

### Key Integration Points

```rust
// Model selection with local models
pub enum ModelProvider {
    Cloud(CloudLLM),
    Local(LocalModelProvider),
}

// Agent request with local model support
pub struct RequestParams {
    pub model_provider: ModelProvider,
    // ... other fields
}

// Factory for creating model clients
pub async fn get_model_client(
    model_provider: &ModelProvider,
    ctx: &AppContext,
) -> Result<Arc<dyn LocalModelClient>> {
    // Get config from settings
    // Create client via ProviderFactory
}
```

## Testing Strategy

### Unit Tests (Completed ✅)
- Config serialization/deserialization
- Provider factory
- Error handling

### Integration Tests (Next)
- Settings persistence
- UI interactions
- Agent execution with local models

### End-to-End Tests
- Full workflow: Settings → Model Selection → Completion

## Dependencies

### Already Available
- `reqwest` - HTTP client ✅
- `serde` - Serialization ✅
- `tokio` - Async runtime ✅
- `thiserror` - Error handling ✅

### May Need to Add
- `async-trait` - For trait objects
- `test-fixtures` - For testing

## Performance Considerations

1. **Connection Pooling** - Reuse HTTP clients
2. **Caching** - Cache model lists with TTL
3. **Timeouts** - Configurable per provider
4. **Error Recovery** - Retry logic for transient failures

## Security Considerations

1. **API Keys** - Store securely using warpui_extras::secure_storage
2. **URLs** - Validate and sanitize
3. **Credentials** - Never log sensitive data
4. **HTTPS** - Support for authenticated endpoints

## Future Enhancements

- [ ] Streaming completions
- [ ] Batch requests
- [ ] Model fine-tuning
- [ ] Quantization options
- [ ] Memory optimization
- [ ] Performance metrics
- [ ] Model validation/verification
