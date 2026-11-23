use super::*;

pub fn render(workspace: &Workspace, root: &str) -> View {
    let mut rows = Vec::new();
    let mut folders = BTreeSet::new();
    for (path, project) in &workspace.projects {
        let project_key = key("project", path, "");
        let project_root = join(root, path);
        let preset = project
            .selected_preset
            .as_deref()
            .or(project.presets.default.as_deref())
            .unwrap_or("Default");
        let detail = project.graph.as_ref().map_or_else(
            || preset.to_owned(),
            |graph| format!("{} · {} · {}", graph.preset, graph.platform, graph.compiler),
        );
        rows.push(
            TreeItem::new(&project_key, display_root(root, path))
                .icon("folder")
                .detail(&detail)
                .expandable(workspace.expanded.contains(&project_key)),
        );

        if !project.presets.names.is_empty() {
            let presets_key = key("presets", path, "");
            rows.push(
                TreeItem::new(&presets_key, "Presets")
                    .parent(&project_key)
                    .icon("settings")
                    .expandable(workspace.expanded.contains(&presets_key)),
            );
            let current = project.selected_preset.as_deref();
            let default =
                TreeItem::new(&key("preset", path, ""), "Project default").parent(&presets_key);
            rows.push(if current.is_none() {
                default.detail("Active")
            } else {
                default
            });
            for (name, config) in &project.presets.names {
                let detail = if current == Some(name.as_str()) {
                    "Active"
                } else {
                    config
                };
                rows.push(
                    TreeItem::new(&key("preset", path, name), name)
                        .parent(&presets_key)
                        .detail(detail),
                );
            }
        }

        if let Some(status) = &project.status {
            rows.push(
                TreeItem::new(&key("status", path, ""), "Slate graph")
                    .parent(&project_key)
                    .detail(status),
            );
        }
        let Some(graph) = &project.graph else {
            continue;
        };
        let targets_key = key("targets", path, "");
        rows.push(
            TreeItem::new(&targets_key, "Targets")
                .parent(&project_key)
                .icon("folder")
                .expandable(workspace.expanded.contains(&targets_key)),
        );
        for target in &graph.targets {
            let parent = add_folders(
                &mut rows,
                &mut folders,
                workspace,
                path,
                &targets_key,
                &slash(&target.folder),
            );
            let kind = if target.test {
                "test"
            } else if target.runnable {
                "run"
            } else {
                "target"
            };
            let target_key = key(kind, path, &target.name);
            let files: BTreeSet<_> = target
                .sources
                .iter()
                .chain(&target.headers)
                .map(|file| slash(file))
                .filter(|file| relative(root, file).is_some())
                .collect();
            let mut row = TreeItem::new(&target_key, &target.name)
                .parent(&parent)
                .detail(&target.kind)
                .icon(if target.test || target.runnable {
                    "run"
                } else {
                    "file"
                });
            if !files.is_empty() {
                row = row.expandable(workspace.expanded.contains(&target_key));
            }
            rows.push(row);
            let module_root = join(&project_root, &target.folder);
            for file in files {
                let label = relative(&module_root, &file)
                    .or_else(|| relative(&project_root, &file))
                    .or_else(|| relative(root, &file))
                    .unwrap_or(&file);
                let (directory, basename) = label.rsplit_once('/').unwrap_or(("", label));
                let parent = add_folders(
                    &mut rows,
                    &mut folders,
                    workspace,
                    path,
                    &target_key,
                    directory,
                );
                let file_key = key(
                    "file",
                    path,
                    &serde_json::json!([target.name, file]).to_string(),
                );
                rows.push(
                    TreeItem::new(&file_key, basename)
                        .parent(&parent)
                        .icon("file"),
                );
            }
        }
    }
    let mut view = if rows.is_empty() {
        View::message("No slate.json was found in this workspace.")
    } else {
        View::new(Body::Tree(rows))
    };
    view.toolbar
        .push(Button::new("refresh", "Refresh").icon("refresh"));
    let selected = workspace.selection(None);
    if let Some((path, target)) = &selected {
        view.status = workspace.projects[path].status.clone().or_else(|| {
            workspace.projects[path]
                .graph
                .as_ref()
                .map(|graph| format!("{} · {} targets", graph.preset, graph.targets.len()))
        });
        if view.status.is_none() && target.is_none() {
            view.status = Some("Expand the project to load its targets and presets.".into());
        }
    }
    for (action, icon) in [
        (Action::Generate, "settings"),
        (Action::Build, "build"),
        (Action::Run, "run"),
        (Action::Test, "test"),
    ] {
        let label = if matches!(action, Action::Run) {
            "Run Selected Target"
        } else {
            action.label()
        };
        view.toolbar.push(
            Button::new(action.id(), label).icon(icon).enabled(
                selected
                    .as_ref()
                    .is_some_and(|(_, target)| action.enabled(*target)),
            ),
        );
    }
    view
}

fn add_folders(
    rows: &mut Vec<TreeItem>,
    folders: &mut BTreeSet<String>,
    workspace: &Workspace,
    project: &str,
    branch: &str,
    path: &str,
) -> String {
    let mut parent = branch.to_owned();
    let mut prefix = String::new();
    for part in path.split('/').filter(|part| !matches!(*part, "" | ".")) {
        prefix.push('/');
        prefix.push_str(part);
        let folder = key("folder", project, &format!("{branch}{prefix}"));
        if folders.insert(folder.clone()) {
            rows.push(
                TreeItem::new(&folder, part)
                    .parent(&parent)
                    .icon("folder")
                    .expandable(workspace.expanded.contains(&folder)),
            );
        }
        parent = folder;
    }
    parent
}
