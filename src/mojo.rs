use std::path::PathBuf;

use zed_extension_api::{
    self as zed,
    serde_json::{self, Value},
    settings::LspSettings,
    DebugAdapterBinary, DebugConfig, DebugRequest, DebugScenario, DebugTaskDefinition, Result,
    StartDebuggingRequestArguments, StartDebuggingRequestArgumentsRequest,
};

struct MojoExtension {}

#[derive(Debug, Clone, Copy)]
enum PythonEnvironmentKind {
    Pixi,
    Venv,
    Conda,
}

// Environment priority list for Mojo toolchain detection
static ENV_PRIORITY_LIST: &[PythonEnvironmentKind] = &[
    PythonEnvironmentKind::Pixi,
    PythonEnvironmentKind::Venv,
    PythonEnvironmentKind::Conda,
];

static BIN_DIR: &str = if cfg!(target_os = "windows") {
    "Scripts"
} else {
    "bin"
};

impl MojoExtension {
    fn find_command(
        &self,
        worktree: &zed::Worktree,
        command: &str,
    ) -> Option<(String, Vec<String>)> {
        for env_type in ENV_PRIORITY_LIST {
            match env_type {
                PythonEnvironmentKind::Pixi => {
                    if let Some(env_path) = worktree.which("pixi") {
                        return Some((env_path, vec!["run".to_string(), command.to_string()]));
                    }
                }
                PythonEnvironmentKind::Conda => {
                    if let Some(env_path) = worktree.which("conda") {
                        return Some((env_path, vec!["run".to_string(), command.to_string()]));
                    }
                }
                PythonEnvironmentKind::Venv => {
                    let worktree_root = PathBuf::from(worktree.root_path());
                    let venv_path = worktree_root.join(".venv");
                    if venv_path.exists() {
                        let cmd_path = venv_path.join(BIN_DIR).join(command);
                        if cmd_path.exists() {
                            return Some((cmd_path.to_string_lossy().to_string(), vec![]));
                        }
                    }
                }
            }
        }

        if let Some(tool_path) = worktree.which(command) {
            return Some((tool_path, vec![]));
        }

        None
    }
}

impl zed::Extension for MojoExtension {
    fn new() -> Self {
        Self {}
    }

    fn language_server_command(
        &mut self,
        language_server_name: &zed::LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<zed::Command> {
        let settings = LspSettings::for_worktree(language_server_name.as_ref(), worktree)?;

        let args = settings
            .settings
            .as_ref()
            .and_then(|s| s.get("args"))
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        // Check if user provided a direct path
        if let Some(lsp_path) = settings
            .settings
            .as_ref()
            .and_then(|s| s.get("lsp_path"))
            .and_then(|v| v.as_str())
        {
            return Ok(zed::Command {
                command: lsp_path.to_string(),
                args,
                env: Default::default(),
            });
        }

        if let Some((command, mut env_args)) = self.find_command(worktree, "mojo-lsp-server") {
            env_args.extend(args);
            return Ok(zed::Command {
                command,
                args: env_args,
                env: Default::default(),
            });
        }

        Err("Must install a supported environment (pixi, uv, conda) or provide mojo-lsp-server path in settings".to_string())
    }

    fn get_dap_binary(
        &mut self,
        adapter_name: String,
        config: DebugTaskDefinition,
        user_provided_debug_adapter_path: Option<String>,
        worktree: &zed::Worktree,
    ) -> Result<DebugAdapterBinary, String> {
        if adapter_name != "mojo-lldb" {
            return Err(format!(
                "Mojo extension does not support unknown adapter in `get_dap_binary`: {adapter_name} (supported: [mojo-lldb])"
            ));
        }

        let connection = config.tcp_connection.map(|tcp| zed::TcpArguments {
            host: tcp.host.unwrap_or_default(),
            port: tcp.port.unwrap_or_default(),
            timeout: None,
        });

        // Check if user provided a direct path
        if let Some(path) = user_provided_debug_adapter_path {
            return Ok(DebugAdapterBinary {
                command: Some(path),
                arguments: vec![],
                envs: vec![],
                cwd: None,
                connection,
                request_args: StartDebuggingRequestArguments {
                    configuration: config.config.to_string(),
                    request: StartDebuggingRequestArgumentsRequest::Launch,
                },
            });
        }

        if let Some((command, arguments)) = self.find_command(worktree, "mojo-lldb-dap") {
            return Ok(DebugAdapterBinary {
                command: Some(command),
                arguments,
                envs: vec![],
                cwd: None,
                connection,
                request_args: StartDebuggingRequestArguments {
                    configuration: config.config.to_string(),
                    request: StartDebuggingRequestArgumentsRequest::Launch,
                },
            });
        }

        Err("Must install a supported environment (pixi, uv, conda) or provide mojo-lldb-dap path in settings".to_string())
    }

    fn dap_request_kind(
        &mut self,
        adapter_name: String,
        config: Value,
    ) -> Result<StartDebuggingRequestArgumentsRequest, String> {
        if adapter_name != "mojo-lldb" {
            return Err(format!(
                "Mojo extension does not support unknown adapter in `dap_request_kind`: {adapter_name} (supported: [mojo-lldb])"
            ));
        }

        config
            .get("request")
            .and_then(|request| {
                request.as_str().and_then(|s| match s {
                    "launch" => Some(StartDebuggingRequestArgumentsRequest::Launch),
                    "attach" => Some(StartDebuggingRequestArgumentsRequest::Attach),
                    _ => None,
                })
            })
            .ok_or_else(|| {
                "Invalid request, expected `request` to be either `launch` or `attach`".into()
            })
    }

    fn dap_config_to_scenario(&mut self, config: DebugConfig) -> Result<DebugScenario, String> {
        if config.adapter != "mojo-lldb" {
            return Err(format!(
                "Mojo extension does not support unknown adapter in `dap_config_to_scenario`: {} (supported: [mojo-lldb])",
                config.adapter
            ));
        }

        let mut configuration = serde_json::json!({
            "type": "lldb-dap",
            "request": match config.request {
                DebugRequest::Launch(_) => "launch",
                DebugRequest::Attach(_) => "attach",
            },
        });

        let map = configuration.as_object_mut().unwrap();

        match &config.request {
            DebugRequest::Attach(attach) => {
                if let Some(pid) = attach.process_id {
                    map.insert("pid".into(), pid.into());
                }
            }
            DebugRequest::Launch(launch) => {
                if !launch.program.is_empty() {
                    map.insert("program".into(), launch.program.clone().into());
                }

                if !launch.args.is_empty() {
                    map.insert("args".into(), launch.args.clone().into());
                }
                if !launch.envs.is_empty() {
                    let env_map: serde_json::Map<String, Value> = launch
                        .envs
                        .clone()
                        .into_iter()
                        .map(|(k, v)| (k, Value::String(v)))
                        .collect();
                    map.insert("env".into(), Value::Object(env_map));
                }
                if let Some(stop_on_entry) = config.stop_on_entry {
                    map.insert("stopOnEntry".into(), stop_on_entry.into());
                }
                if let Some(cwd) = launch.cwd.as_ref() {
                    map.insert("cwd".into(), cwd.to_string().into());
                }
            }
        }

        let debug_scenario = DebugScenario {
            adapter: config.adapter,
            label: config.label,
            config: configuration.to_string(),
            build: None,
            tcp_connection: None,
        };

        Ok(debug_scenario)
    }
}

zed::register_extension!(MojoExtension);
