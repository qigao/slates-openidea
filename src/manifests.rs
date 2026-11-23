use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;

#[derive(Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct Fragment {
    include: Vec<String>,
    default_preset: Option<String>,
    presets: BTreeMap<String, Preset>,
}

#[derive(Deserialize)]
struct Preset {
    config: Option<String>,
    inherits: Option<String>,
}

#[derive(Debug, Default)]
pub struct Presets {
    pub default: Option<String>,
    pub names: BTreeMap<String, String>,
}

pub fn slash(path: &str) -> String {
    path.replace('\\', "/")
}

pub fn relative<'a>(root: &str, path: &'a str) -> Option<&'a str> {
    let root = root.trim_end_matches('/');
    let prefix = path.get(..root.len())?;
    let windows = root.as_bytes().get(1) == Some(&b':') || root.starts_with("//");
    if (windows && prefix.eq_ignore_ascii_case(root)) || prefix == root {
        return match path.get(root.len()..) {
            Some("") => Some(""),
            Some(suffix) => suffix.strip_prefix('/'),
            None => None,
        };
    }
    None
}

pub fn join(root: &str, path: &str) -> String {
    match (root, path) {
        ("", path) => path.to_owned(),
        (root, "") => root.to_owned(),
        (root, path) => format!("{}/{path}", root.trim_end_matches('/')),
    }
}

fn include_path(parent: &str, path: &str) -> Result<String, String> {
    let path = slash(path);
    if path.is_empty() || path.starts_with('/') || path.as_bytes().get(1) == Some(&b':') {
        return Err(format!("Expected a relative preset include: {path}"));
    }
    let (directory, _) = parent.rsplit_once('/').unwrap_or(("", parent));
    let mut parts: Vec<_> = directory
        .split('/')
        .filter(|part| !part.is_empty())
        .collect();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." if parts.pop().is_some() => {}
            ".." => return Err(format!("Preset include leaves the workspace: {path}")),
            part => parts.push(part),
        }
    }
    Ok(parts.join("/"))
}

pub fn read(
    working_manifest: &str,
    read_file: impl Fn(&str) -> Result<String, String>,
) -> Result<Presets, String> {
    let mut all = Fragment::default();
    let mut loading = BTreeSet::new();
    let mut loaded = BTreeSet::new();
    load(
        working_manifest,
        &read_file,
        &mut all,
        &mut loading,
        &mut loaded,
    )?;

    let names = all
        .presets
        .keys()
        .filter_map(|name| {
            config_for(name, &all.presets).map(|config| (name.clone(), config.to_owned()))
        })
        .collect();
    Ok(Presets {
        default: all.default_preset,
        names,
    })
}

fn load(
    file: &str,
    read_file: &impl Fn(&str) -> Result<String, String>,
    all: &mut Fragment,
    loading: &mut BTreeSet<String>,
    loaded: &mut BTreeSet<String>,
) -> Result<(), String> {
    if loaded.contains(file) {
        return Ok(());
    }
    if loading.len() >= 128 || !loading.insert(file.to_owned()) {
        return Err(format!(
            "Preset include cycle or excessive nesting at {file}"
        ));
    }
    let text = read_file(file).map_err(|error| format!("{file}: {error}"))?;
    let fragment: Fragment = jsonc_parser::parse_to_serde_value(
        text.trim_start_matches('\u{feff}'),
        &Default::default(),
    )
    .map_err(|error| format!("{file}: {error}"))?;
    for path in fragment.include {
        load(&include_path(file, &path)?, read_file, all, loading, loaded)?;
    }
    if let Some(name) = fragment.default_preset {
        if all.default_preset.replace(name).is_some() {
            return Err(format!("Duplicate defaultPreset in {file}"));
        }
    }
    for (name, preset) in fragment.presets {
        if all.presets.insert(name.clone(), preset).is_some() {
            return Err(format!("Duplicate preset {name} in {file}"));
        }
    }
    loading.remove(file);
    loaded.insert(file.to_owned());
    Ok(())
}

fn config_for<'a>(name: &'a str, presets: &'a BTreeMap<String, Preset>) -> Option<&'a str> {
    let mut current = Some(name);
    let mut visited = BTreeSet::new();
    let mut config = None;
    while let Some(name) = current {
        if !visited.insert(name) {
            return None;
        }
        let preset = presets.get(name)?;
        config = config.or(preset.config.as_deref());
        current = preset.inherits.as_deref();
    }
    config.filter(|config| {
        !matches!(*config, "" | "." | "..")
            && config
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"_.-".contains(&c))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discovers_jsonc_presets_from_includes_and_inheritance() {
        let files = BTreeMap::from([
            (
                "apps/slate.json",
                r#"{"include":["../common.json"],"defaultPreset":"tests",
                "presets":{"tests":{"inherits":"debug"},},}"#,
            ),
            (
                "common.json",
                r#"{"presets":{"debug":{"config":"debug"},"base":{}, // abstract
                "cycle":{"config":"broken","inherits":"cycle"}}}"#,
            ),
        ]);
        let presets = read("apps/slate.json", |p| Ok(files[p].to_owned())).unwrap();
        assert_eq!(presets.default.as_deref(), Some("tests"));
        assert_eq!(
            presets.names,
            BTreeMap::from([
                ("debug".into(), "debug".into()),
                ("tests".into(), "debug".into())
            ])
        );
    }

    #[test]
    fn checks_workspace_boundary_and_include_cycles() {
        assert_eq!(relative("C:/SRC", "c:/src/apps"), Some("apps"));
        assert_eq!(relative("C:/SRC", "C:/SRC-other/file"), None);
        assert!(
            read(
                "slate.json",
                |_| Ok(r#"{"include":["../out.json"]}"#.into())
            )
            .unwrap_err()
            .contains("leaves")
        );
        assert!(
            read("slate.json", |_| Ok(r#"{"include":["a.json"]}"#.into()))
                .unwrap_err()
                .contains("cycle")
        );
    }
}
