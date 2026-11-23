mod manifests;

use std::collections::{BTreeMap, BTreeSet};

use openidea_plugin::views::{Body, Button, TreeItem, View, ViewEvent, Viewport, Views};
use openidea_plugin::{Error, Invocation, Plugin, Project, host, register_plugin};
use serde::{Deserialize, Serialize};

use manifests::{Presets, join, relative, slash};

const VIEW: &str = "slate";
const PROGRAM: &str = "slate";
const RUN_CONFIGURATION: &str = "run-target";
const PRESET_STORAGE: &str = "selected-presets";

#[derive(Default)]
struct Slate {
    workspaces: BTreeMap<String, Workspace>,
}

#[derive(Default)]
struct Workspace {
    projects: BTreeMap<String, SlateProject>,
    expanded: BTreeSet<String>,
    selected: Option<String>,
    scanned: bool,
}

#[derive(Default)]
struct SlateProject {
    presets: Presets,
    selected_preset: Option<String>,
    graph: Option<Graph>,
    graph_with_tests: bool,
    status: Option<String>,
}

#[derive(Deserialize)]
struct Graph {
    preset: String,
    platform: String,
    compiler: String,
    targets: Vec<Target>,
}

#[derive(Deserialize)]
struct Target {
    name: String,
    folder: String,
    kind: String,
    runnable: bool,
    test: bool,
    #[serde(default)]
    sources: Vec<String>,
    #[serde(default)]
    headers: Vec<String>,
}

#[derive(Clone, Copy)]
enum Action {
    Generate,
    Build,
    Run,
    Test,
}

impl Action {
    fn from_id(id: &str) -> Option<Self> {
        Some(match id {
            "gen" => Self::Generate,
            "build" => Self::Build,
            "run" => Self::Run,
            "test" => Self::Test,
            _ => return None,
        })
    }

    fn id(self) -> &'static str {
        match self {
            Self::Generate => "gen",
            Self::Build => "build",
            Self::Run => "run",
            Self::Test => "test",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Generate => "Generate",
            Self::Build => "Build",
            Self::Run => "Run",
            Self::Test => "Test",
        }
    }

    fn enabled(self, target: Option<&Target>) -> bool {
        match self {
            Self::Run => target.is_some_and(|target| target.runnable && !target.test),
            Self::Test => target.is_none_or(|target| target.test),
            Self::Build | Self::Generate => true,
        }
    }
}

fn key(kind: &str, project: &str, value: &str) -> String {
    format!("{kind}:{}", serde_json::json!([project, value]))
}

fn decode(key: &str) -> Option<(&str, String, String)> {
    let (kind, value) = key.split_once(':')?;
    let [project, item]: [String; 2] = serde_json::from_str(value).ok()?;
    Some((kind, project, item))
}

fn chosen_presets() -> BTreeMap<String, String> {
    host::storage::get(PRESET_STORAGE)
        .ok()
        .flatten()
        .and_then(|value| serde_json::from_str(&value).ok())
        .unwrap_or_default()
}

impl Workspace {
    fn scan(&mut self, project: &Project, root: &str) -> Result<(), Error> {
        let mut manifests = project.find_files("**/slate.json", 256)?;
        if project.read_file("slate.json").is_ok() {
            manifests.push(join(root, "slate.json"));
        }
        let mut old = std::mem::take(&mut self.projects);
        for path in manifests {
            let path = slash(&path);
            if let Some(path) = relative(root, &path) {
                let directory = match path {
                    "slate.json" => "",
                    path => match path.strip_suffix("/slate.json") {
                        Some(directory) => directory,
                        None => continue,
                    },
                };
                self.projects
                    .entry(directory.into())
                    .or_insert_with(|| old.remove(directory).unwrap_or_default());
            }
        }
        self.scanned = true;
        Ok(())
    }

    fn selection(&self, item: Option<&str>) -> Option<(String, Option<&Target>)> {
        let item = item.or(self.selected.as_deref());
        if let Some((kind, path, value)) = item.and_then(decode) {
            let project = self.projects.get(&path)?;
            let target = match kind {
                "target" | "run" | "test" => Some(
                    project
                        .graph
                        .as_ref()?
                        .targets
                        .iter()
                        .find(|target| target.name == value)?,
                ),
                _ => None,
            };
            return Some((path, target));
        }
        (self.projects.len() == 1).then(|| (self.projects.keys().next().unwrap().clone(), None))
    }

    fn reload(&mut self, project: &Project, root: &str, path: &str) {
        let Some(state) = self.projects.get_mut(path) else {
            return;
        };
        let absolute = join(root, path);
        let manifest = join(path, "slate.json").trim_start_matches('/').to_owned();
        state.graph = None;
        state.graph_with_tests = false;
        state.status = None;
        state.presets = match manifests::read(&manifest, |file| {
            project.read_file(file).map_err(|error| error.to_string())
        }) {
            Ok(presets) => presets,
            Err(error) => {
                state.status = Some(error);
                return;
            }
        };
        state.selected_preset = chosen_presets()
            .remove(&absolute)
            .filter(|name| state.presets.names.contains_key(name));
        let mut args = state.args("graph", &absolute);
        args.extend(["--format".into(), "json".into(), "--tests".into()]);
        match read_graph(project, &args) {
            Ok(graph) => {
                state.graph = Some(graph);
                state.graph_with_tests = true;
            }
            Err(error) => {
                args.pop();
                match read_graph(project, &args) {
                    Ok(graph) => {
                        state.graph = Some(graph);
                        state.status = Some(format!("Tests unavailable: {error}"));
                    }
                    Err(error) => state.status = Some(error),
                }
            }
        }
    }

    fn refresh(&mut self, project: &Project, root: &str) -> Result<(), Error> {
        self.scan(project, root)?;
        let active = self
            .selected
            .as_deref()
            .and_then(decode)
            .map(|(_, path, _)| path)
            .filter(|path| self.projects.contains_key(path));
        let path = active.or_else(|| {
            (self.projects.len() == 1).then(|| self.projects.keys().next().unwrap().clone())
        });
        if let Some(path) = path {
            let project_key = key("project", &path, "");
            self.expanded.insert(project_key.clone());
            self.reload(project, root, &path);
            if self.selection(None).is_none() || self.selected.is_none() {
                self.selected = Some(project_key);
            }
        } else {
            self.selected = None;
        }
        Ok(())
    }

    fn launch(&self, root: &str, item: Option<&str>, action: Action) -> Result<(), Error> {
        let (path, target) = self
            .selection(item)
            .ok_or("Select a Slate project or target")?;
        if !action.enabled(target) {
            return Err("This action is unavailable for the selected target".into());
        }
        let project = &self.projects[&path];
        let mut args = project.args(action.id(), &join(root, &path));
        if let Some(target) = target {
            args.extend(["--target".into(), target.name.clone()]);
            if project.graph_with_tests && !matches!(action, Action::Test) {
                args.push("--tests".into());
            }
        }
        if matches!(action, Action::Run) {
            return host::start_run(RUN_CONFIGURATION, &args[1..], false)
                .map(|_| ())
                .map_err(|error| match error {
                    Error::NotFound(message) if !path.is_empty() => Error::NotFound(format!(
                        "{message}. Open {} directly to run it in the Run panel.",
                        join(root, &path)
                    )),
                    error => error,
                });
        }
        let name = target.map_or_else(|| display_root(root, &path), |target| target.name.as_str());
        host::open_terminal(&format!("Slate {} {name}", action.label()), PROGRAM, &args)
    }
}

impl SlateProject {
    fn args(&self, command: &str, root: &str) -> Vec<String> {
        let mut args = vec![command.into(), "--root".into(), root.into()];
        if let Some(preset) = self.selected_preset.as_ref() {
            args.extend(["--preset".into(), preset.clone()]);
        }
        args
    }
}

fn read_graph(project: &Project, args: &[String]) -> Result<Graph, String> {
    let result = project
        .run_process(PROGRAM, args, None)
        .map_err(|error| format!("Slate executable: {error}"))?;
    if result.status != Some(0) {
        let message = if result.stderr.trim().is_empty() {
            &result.stdout
        } else {
            &result.stderr
        };
        return Err(if message.trim().is_empty() {
            result.status.map_or_else(
                || "Slate process was interrupted".into(),
                |code| format!("Slate exited with {code}"),
            )
        } else {
            message.trim().chars().take(1500).collect()
        });
    }
    serde_json::from_str(&result.stdout).map_err(|error| format!("Slate graph JSON: {error}"))
}

fn display_root<'a>(root: &'a str, path: &'a str) -> &'a str {
    if path.is_empty() {
        root.rsplit('/')
            .next()
            .filter(|part| !part.is_empty())
            .unwrap_or(root)
    } else {
        path
    }
}

impl Plugin for Slate {
    fn run_command(
        &mut self,
        id: &str,
        ctx: &Invocation,
        arg: Option<&str>,
    ) -> Result<Option<String>, Error> {
        let project = ctx.project().ok_or("Open a project first")?;
        let root = slash(&project.root()?);
        let state = self.workspaces.entry(root.clone()).or_default();
        if id == "refresh" {
            state.refresh(&project, &root)?;
            host::refresh_view(VIEW);
            tree::publish(state, &root)?;
            host::reveal_view(VIEW)?;
        } else if let Some(action) = Action::from_id(id) {
            if !state.scanned {
                state.scan(&project, &root)?;
            }
            state.launch(&root, arg, action)?;
        }
        Ok(None)
    }
}

impl Views for Slate {
    fn render(&mut self, _: &str, _: Option<Viewport>, ctx: &Invocation) -> Result<View, Error> {
        let Some(project) = ctx.project() else {
            return Ok(View::message("Open a workspace containing slate.json."));
        };
        let root = slash(&project.root()?);
        let state = self.workspaces.entry(root.clone()).or_default();
        if !state.scanned {
            state.scan(&project, &root)?;
        }
        Ok(tree::render(state, &root))
    }

    fn handle(&mut self, _: &str, event: ViewEvent, ctx: &Invocation) -> Result<(), Error> {
        let project = ctx.project().ok_or("Open a project first")?;
        let root = slash(&project.root()?);
        let state = self.workspaces.entry(root.clone()).or_default();
        let event = match event {
            ViewEvent::Message(text) => match serde_json::from_str::<tree::Message>(&text)
                .map_err(|error| Error::Invalid(error.to_string()))?
            {
                tree::Message::Ready => return tree::publish(state, &root),
                tree::Message::Toggle {
                    key,
                    expanded: true,
                } => ViewEvent::Expand(key),
                tree::Message::Toggle {
                    key,
                    expanded: false,
                } => ViewEvent::Collapse(key),
                tree::Message::Select { key } => ViewEvent::Select(key),
                tree::Message::Activate { key } => ViewEvent::Activate(key),
                tree::Message::Launch { command, key } => {
                    let action = Action::from_id(&command).ok_or("Unknown Slate action")?;
                    return state.launch(&root, Some(&key), action);
                }
            },
            event => event,
        };
        match event {
            ViewEvent::Button(id) if id == "refresh" => state.refresh(&project, &root)?,
            ViewEvent::Button(id) => {
                if let Some(action) = Action::from_id(&id) {
                    state.launch(&root, None, action)?;
                }
                return Ok(());
            }
            ViewEvent::Select(row) => state.selected = Some(row),
            ViewEvent::Expand(row) => {
                if let Some(("project", path, _)) = decode(&row) {
                    state.selected = Some(row.clone());
                    if state.projects.get(&path).is_some_and(|p| p.graph.is_none()) {
                        state.reload(&project, &root, &path);
                    }
                }
                state.expanded.insert(row);
            }
            ViewEvent::Collapse(row) => {
                state.expanded.remove(&row);
            }
            ViewEvent::Activate(row) => match decode(&row) {
                Some(("preset", path, name)) => {
                    if state
                        .projects
                        .get(&path)
                        .is_some_and(|p| name.is_empty() || p.presets.names.contains_key(&name))
                    {
                        let absolute = join(&root, &path);
                        let mut choices = chosen_presets();
                        if name.is_empty() {
                            choices.remove(&absolute);
                        } else {
                            choices.insert(absolute, name);
                        }
                        host::storage::set(
                            PRESET_STORAGE,
                            &serde_json::json!(choices).to_string(),
                        )?;
                        state.selected = Some(key("project", &path, ""));
                        state.reload(&project, &root, &path);
                    }
                }
                Some(("file", _, item)) => {
                    if let Ok([_, file]) = serde_json::from_str::<[String; 2]>(&item) {
                        host::open_location(&file, 0, 0)?;
                    }
                }
                _ => return Ok(()),
            },
            _ => return Ok(()),
        }
        host::refresh_view(VIEW);
        tree::publish(state, &root)
    }
}

mod tree;
register_plugin!(Slate, views);

#[cfg(test)]
mod tests;
