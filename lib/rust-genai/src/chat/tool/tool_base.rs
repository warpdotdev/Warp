use super::{ToolConfig, ToolName};
use crate::chat::CacheControl;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Tool metadata used for function calling by LLMs.
pub struct Tool {
	/// Normalized tool identifier.
	/// Example: `ToolName::Custom("get_weather".to_string())`.
	pub name: ToolName,

	/// Human-readable description used by the LLM to understand when and how to call this tool.
	pub description: Option<String>,

	/// JSON Schema for the tool parameters.
	/// Example:
	/// ```rust
	/// json!({
	/// "type": "object",
	/// "properties": {
	///    "city": {
	///        "type": "string",
	///        "description": "The city name"
	///    },
	///    "country": {
	///        "type": "string",
	///        "description": "The most likely country of this city name"
	///    },
	///    "unit": {
	///        "type": "string",
	///        "enum": ["C", "F"],
	///        "description": "The temperature unit for the country. C for Celsius, and F for Fahrenheit"
	///    }
	/// },
	/// "required": ["city", "country", "unit"],
	/// })
	/// ```
	pub schema: Option<Value>,

	/// When `true`, the provider enforces strict schema validation on tool-call arguments.
	/// For OpenAI this sets `"strict": true` and auto-injects `"additionalProperties": false`
	/// on every `"type": "object"` node in the schema.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub strict: Option<bool>,

	/// Optional configuration for the tool.
	///
	/// Useful with embedded provider tools (e.g., Google Search for Gemini).
	pub config: Option<ToolConfig>,

	/// Optional per-tool cache control.
	///
	/// Currently only honored by the Anthropic adapter: when set on a `Tool`,
	/// the adapter emits the corresponding `cache_control` field on that tool's
	/// JSON object in the `tools` array. Use a 1h breakpoint on the last tool
	/// to mark the static prefix (tools → system) as long-lived cache, then
	/// short TTLs on later parts (messages tail) to satisfy Anthropic's
	/// `tools → system → messages` TTL ordering requirement.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub cache_control: Option<CacheControl>,
}

/// Computed accessors
impl Tool {
	/// Returns an approximate in-memory size of this `Tool`, in bytes,
	/// computed as the sum of the UTF-8 lengths of:
	/// - `name`
	/// - `description` (if any)
	/// - JSON-serialized `schema` (if any)
	/// - JSON-serialized `config` (if any)
	pub fn size(&self) -> usize {
		let mut size = match &self.name {
			ToolName::WebSearch => 9, // "WebSearch".len()
			ToolName::Custom(name) => name.len(),
		};
		size += self.description.as_ref().map(|d| d.len()).unwrap_or_default();
		size += self
			.schema
			.as_ref()
			.map(|s| serde_json::to_string(s).map(|j| j.len()).unwrap_or_default())
			.unwrap_or_default();
		size += self
			.config
			.as_ref()
			.map(|c| match c {
				ToolConfig::WebSearch(_) => 0,
				ToolConfig::Custom(v) => serde_json::to_string(v).map(|j| j.len()).unwrap_or_default(),
			})
			.unwrap_or_default();
		size
	}
}

/// Constructor
impl Tool {
	/// Create a new tool with the given name.
	pub fn new(name: impl Into<ToolName>) -> Self {
		Self {
			name: name.into(),
			description: None,
			schema: None,
			strict: None,
			config: None,
			cache_control: None,
		}
	}

	/// Create a new web search tool.
	pub fn new_web_search() -> Self {
		Self::new(ToolName::WebSearch)
	}
}

// region:    --- Setters

impl Tool {
	/// Set the tool description. Returns self for chaining.
	pub fn with_description(mut self, description: impl Into<String>) -> Self {
		self.description = Some(description.into());
		self
	}

	/// Set the JSON Schema for the tool parameters. Returns self for chaining.
	pub fn with_schema(mut self, parameters: Value) -> Self {
		self.schema = Some(parameters);
		self
	}

	/// Enable strict schema validation for tool-call arguments.
	/// When `true`, OpenAI enforces exact schema conformance.
	pub fn with_strict(mut self, strict: bool) -> Self {
		self.strict = Some(strict);
		self
	}

	/// Set provider-specific configuration (if any). Returns self for chaining.
	pub fn with_config(mut self, config: impl Into<ToolConfig>) -> Self {
		self.config = Some(config.into());
		self
	}

	/// Attach a `cache_control` breakpoint to this tool. Currently honored only
	/// by the Anthropic adapter (emitted on the tool JSON in the `tools` array).
	pub fn with_cache_control(mut self, cache_control: impl Into<CacheControl>) -> Self {
		self.cache_control = Some(cache_control.into());
		self
	}
}

// endregion: --- Setters
