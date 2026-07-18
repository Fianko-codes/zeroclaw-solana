//! Milestone 0 ZeroClaw `tool-plugin` component build spike.
//!
//! The pure dependency probe is host-testable in [`core`]. The component shim
//! is compiled only for WASM and binds directly to the checked-out vendored WIT.

pub mod core;

#[cfg(target_family = "wasm")]
mod component {
    wit_bindgen::generate!({
        path: "../../zeroclaw-plugins/wit/v0",
        world: "tool-plugin",
        features: ["plugins-wit-v0"],
    });

    use crate::core::probe_json;
    use exports::zeroclaw::plugin::plugin_info::Guest as PluginInfo;
    use exports::zeroclaw::plugin::tool::{Guest as Tool, ToolResult};
    use zeroclaw::plugin::logging::{
        log_record, LogLevel, PluginAction, PluginEvent, PluginOutcome,
    };

    const PLUGIN_NAME: &str = "zeroclaw-wasm-build-spike";
    const PLUGIN_VERSION: &str = env!("CARGO_PKG_VERSION");
    const TOOL_NAME: &str = "m0_dependency_probe";

    struct WasmBuildSpike;

    impl PluginInfo for WasmBuildSpike {
        fn plugin_name() -> String {
            PLUGIN_NAME.to_string()
        }

        fn plugin_version() -> String {
            PLUGIN_VERSION.to_string()
        }
    }

    impl Tool for WasmBuildSpike {
        fn name() -> String {
            TOOL_NAME.to_string()
        }

        fn description() -> String {
            "Run the isolated M0 base58, SHA-256, curve, JSON, and base64 build probe.".to_string()
        }

        fn parameters_schema() -> String {
            serde_json::json!({
                "type": "object",
                "properties": {
                    "public_key": {
                        "type": "string",
                        "description": "A base58-encoded 32-byte public key."
                    },
                    "payload": {
                        "type": "string",
                        "description": "Text to hash with SHA-256."
                    }
                },
                "required": ["public_key", "payload"],
                "additionalProperties": false
            })
            .to_string()
        }

        fn execute(args: String) -> Result<ToolResult, String> {
            // Constructing the WASM-only client exercises waki's compiled API
            // without issuing a network request.
            let _http_client = waki::Client::new();

            match probe_json(&args) {
                Ok(output) => {
                    emit(
                        LogLevel::Info,
                        PluginAction::Complete,
                        PluginOutcome::Success,
                        "completed M0 dependency probe",
                    );
                    Ok(ToolResult {
                        success: true,
                        output,
                        error: None,
                    })
                }
                Err(error) => {
                    emit(
                        LogLevel::Warn,
                        PluginAction::Validate,
                        PluginOutcome::Failure,
                        "rejected invalid M0 dependency probe arguments",
                    );
                    Ok(ToolResult {
                        success: false,
                        output: String::new(),
                        error: Some(error.to_string()),
                    })
                }
            }
        }
    }

    fn emit(level: LogLevel, action: PluginAction, outcome: PluginOutcome, message: &str) {
        log_record(
            level,
            &PluginEvent {
                function_name: "zeroclaw_wasm_build_spike::tool::execute".to_string(),
                action,
                outcome: Some(outcome),
                duration_ms: None,
                attrs: Some(serde_json::json!({ "tool": TOOL_NAME }).to_string()),
                message: message.to_string(),
            },
        );
    }

    export!(WasmBuildSpike);
}
