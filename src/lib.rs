use abi_stable::std_types::{ROption, RString, RVec};
use anyrun_plugin::*;
use serde::Deserialize;
use shellexpand::tilde;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use thiserror::Error;

#[derive(Deserialize, Default)]
#[serde(transparent)]
struct ConfigPrefix(Option<String>);

#[derive(Deserialize)]
#[serde(transparent)]
struct ConfigCommand(String);

#[derive(Deserialize)]
#[serde(transparent)]
struct ConfigIcon(String);

#[derive(Deserialize)]
#[serde(transparent)]
struct ConfigWorkspace(String);

impl Default for ConfigCommand {
    fn default() -> Self {
        Self("code".to_owned())
    }
}

impl Default for ConfigIcon {
    fn default() -> Self {
        Self("com.visualstudio.code".to_owned())
    }
}

impl Default for ConfigWorkspace {
    fn default() -> Self {
        Self("~/.config/Code/User/workspaceStorage".to_owned())
    }
}

#[derive(Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    prefix: ConfigPrefix,

    #[serde(default)]
    command: ConfigCommand,

    #[serde(default)]
    icon: ConfigIcon,

    #[serde(default)]
    workspace: ConfigWorkspace,
}

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("config file not found")]
    Io(#[from] std::io::Error),
    #[error("config file is invalid")]
    Ron(#[from] ron::de::SpannedError),
}

pub struct State {
    results: Vec<(String, String, u64)>,
    config: Config,
}

#[derive(Debug, Deserialize)]
struct Workspace {
    folder: Option<String>,
}

#[init]
fn init(config_dir: RString) -> State {
    let config: Config = fs::read_to_string(format!("{}/vscode.ron", config_dir))
        .map_err(ConfigError::Io)
        .and_then(|content| ron::from_str(&content).map_err(ConfigError::Ron))
        .map_err(|err| eprintln!("{}", err))
        .unwrap_or_default();

    let base_path_str = &(config.workspace.0.to_owned())[..];

    let expanded_path = tilde(base_path_str);
    let base_path = PathBuf::from(expanded_path.into_owned());

    let mut results: Vec<(String, String, u64)> = Vec::new();
    let mut index: u64 = 0;

    let mut already_have: HashSet<String> = HashSet::new();

    if let Ok(entries) = fs::read_dir(base_path) {
        for entry in entries.flatten() {
            let file_path = entry.path().join("workspace.json");

            if file_path.exists() && file_path.is_file() {
                if let Ok(contents) = fs::read_to_string(&file_path) {
                    if let Ok(parsed) = serde_json::from_str::<Workspace>(&contents) {
                        if let Some(folder_tmp) = parsed.folder {
                            let folder = Path::new(&folder_tmp);

                            let full_path = folder_tmp.replace("file://", "");
                            let shortcut =
                                folder.file_name().unwrap().to_str().unwrap().to_string();

                            if !already_have.contains(&full_path) {
                                already_have.insert(full_path.clone());
                                results.push((full_path, shortcut, index));
                                index += 1;
                            }
                        }
                    }
                }
            }
        }
    }

    State { results, config }
}

#[info]
fn info() -> PluginInfo {
    PluginInfo {
        name: "VSCode Recents".into(),
        icon: "com.visualstudio.code".into(), // Icon from the icon theme
    }
}

#[get_matches]
fn get_matches(input: RString, state: &State) -> RVec<Match> {
    if input.is_empty() {
        return RVec::new();
    }

    if let ConfigPrefix(Some(prefix)) = &state.config.prefix {
        if !input.starts_with(prefix) {
            return RVec::new();
        }
    }

    let query = if let ConfigPrefix(Some(prefix)) = &state.config.prefix {
        input.replace(prefix, "")
    } else {
        input.to_string()
    };

    let matches = state
        .results
        .iter()
        .filter_map(|(full, short, id)| {
            if short.contains(query.trim()) {
                Some(Match {
                    title: format!("VSCode: {}", short).into(),
                    icon: ROption::RSome((state.config.icon.0.to_owned())[..].into()),
                    use_pango: false,
                    description: ROption::RSome(full[..].into()),
                    id: ROption::RSome(*id),
                })
            } else {
                None
            }
        })
        .take(5)
        .collect::<RVec<Match>>();

    matches
}

#[handler]
fn handler(selection: Match, state: &State) -> HandleResult {
    let entry = state
        .results
        .iter()
        .find_map(|(full, _short, id)| {
            if *id == selection.id.unwrap() {
                Some(full)
            } else {
                None
            }
        })
        .unwrap();

    if Command::new("bash")
        .arg("-c")
        .arg(format!("{} {}", state.config.command.0.to_owned(), entry))
        .spawn()
        .is_err()
    {
        eprintln!("Error running vscode");
    }

    HandleResult::Close
}
