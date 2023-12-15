use abi_stable::std_types::{ROption, RString, RVec};
use anyrun_plugin::*;
use itertools::Itertools;
use serde::Deserialize;
use shellexpand::tilde;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use textdistance::str::damerau_levenshtein;
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
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Ron(#[from] ron::de::SpannedError),
}

#[derive(Error, Debug)]
pub enum ScanError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

pub struct State {
    results: Vec<Project>,
    config: Config,
}

#[init]
fn init(config_dir: RString) -> State {
    let config: Config = fs::read_to_string(format!("{}/vscode.ron", config_dir))
        .map_err(ConfigError::Io)
        .and_then(|content| ron::from_str(&content).map_err(ConfigError::Ron))
        .map_err(|err| eprintln!("Error parsing config: {}", err))
        .unwrap_or_default();

    let expanded_path = tilde(&config.workspace.0);
    let base_path = PathBuf::from(expanded_path.into_owned());

    let results = scan_workspaces(&base_path)
        .map_err(|err| eprintln!("Error listing vscode projects: {}", err))
        .map(|projects| {
            projects
                .into_iter()
                .unique_by(|p| p.fullpath.clone())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    State { results, config }
}

#[derive(Debug, Deserialize)]
struct Workspace {
    folder: String,
}

#[derive(Debug)]
struct Project {
    index: u64,
    fullpath: String,
    shortname: String,
}

fn scan_workspaces(path: &PathBuf) -> Result<Vec<Project>, ScanError> {
    fs::read_dir(path)
        .map_err(ScanError::Io)
        .map(|entries| entries.flatten())?
        .map(|entry| entry.path().join("workspace.json"))
        .filter(|path| path.exists() && path.is_file())
        .enumerate()
        .map(|(index, path)| scan_workspace(&path, index as u64))
        .filter(|res| res.is_ok())
        .collect()
}

fn scan_workspace(path: &PathBuf, index: u64) -> Result<Project, ScanError> {
    fs::read_to_string(path)
        .map_err(ScanError::Io)
        .map(|content| serde_json::from_str::<Workspace>(&content).map_err(ScanError::Json))?
        .map(|ws| {
            let folder = Path::new(&ws.folder);
            let fullpath = ws.folder.replace("file://", "");
            let shortname = folder.file_name().unwrap().to_str().unwrap().to_string();

            Project {
                index,
                fullpath,
                shortname,
            }
        })
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
        .map(|project| {
            (
                damerau_levenshtein(&query, &project.shortname),
                Some(Match {
                    title: format!("VSCode: {}", project.shortname).into(),
                    icon: ROption::RSome((state.config.icon.0.to_owned())[..].into()),
                    use_pango: false,
                    description: ROption::RSome(project.fullpath[..].into()),
                    id: ROption::RSome(project.index),
                }),
            )
        })
        .sorted_by(|a, b| Ord::cmp(&b.0, &a.0))
        .rev()
        .flat_map(|i| i.1)
        .take(5)
        .collect::<RVec<Match>>();

    matches
}

#[handler]
fn handler(selection: Match, state: &State) -> HandleResult {
    let entry = state
        .results
        .iter()
        .find_map(|project| {
            if project.index == selection.id.unwrap() {
                Some(project.fullpath.clone())
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
