use super::*;
use openidea_plugin::{PluginRunConfiguration, ProcessOutput, mock::MockHost};

const GRAPH: &str = r#"{
  "preset": "debug", "platform": "windows-x64", "compiler": "msvc",
  "targets": [
    {"name": "hello", "folder": "examples/hello", "kind": "executable", "runnable": true, "test": false,
     "sources": ["/project/app/examples/hello/src/main.c"], "headers": ["/project/app/examples/hello/include/hello.h"]},
    {"name": "math", "folder": "core", "kind": "static", "runnable": false, "test": false},
    {"name": "suite", "folder": "core/test", "kind": "header_only", "runnable": false, "test": true}
  ]
}"#;

fn workspace() -> MockHost {
    let host = MockHost::install().with_capabilities(&["project.read", "storage", "process:slate"]);
    host.add_file(
        "app/slate.json",
        r#"{"include":["presets.json"],"defaultPreset":"debug"}"#,
    );
    host.add_file(
        "app/presets.json",
        r#"{"presets":{"debug":{"config":"debug"},"release":{"config":"release"}}}"#,
    );
    host.add_file(
        "other/slate.json",
        r#"{"presets":{"debug":{"config":"debug"}}}"#,
    );
    host
}

#[test]
fn groups_targets_keeps_project_presets_and_opens_source_files() {
    let host = workspace();
    host.add_file("apps/slate.json", "{}");
    host.set_run_configurations(vec![PluginRunConfiguration {
        id: RUN_CONFIGURATION.into(),
        name: "Slate Run".into(),
        program: PROGRAM.into(),
        args: vec!["run".into()],
        ..Default::default()
    }]);
    host.on_process(PROGRAM, |args, _| {
        assert_eq!(args[0], "graph");
        assert_eq!(args[1..3], ["--root", "/project/app"]);
        assert_eq!(args[args.len() - 3..], ["--format", "json", "--tests"]);
        Ok(ProcessOutput {
            status: Some(0),
            stdout: GRAPH.into(),
            ..Default::default()
        })
    });
    let ctx = host.invocation();
    let mut slate = Slate::default();
    let initial = slate.render(VIEW, None, &ctx).unwrap();
    let Body::Tree(initial) = initial.body else {
        panic!()
    };
    assert_eq!(
        initial
            .iter()
            .map(|item| item.label.as_str())
            .collect::<Vec<_>>(),
        ["app", "apps", "other"]
    );
    assert!(host.process_runs().is_empty());

    slate
        .handle(VIEW, ViewEvent::Expand(key("project", "app", "")), &ctx)
        .unwrap();
    slate
        .handle(
            VIEW,
            ViewEvent::Activate(key("preset", "app", "release")),
            &ctx,
        )
        .unwrap();
    assert_eq!(chosen_presets()["/project/app"], "release");
    let view = slate.render(VIEW, None, &ctx).unwrap();
    let Body::Tree(rows) = view.body else {
        panic!()
    };
    assert!(rows.iter().any(|item| item.label == "include"));
    let source = rows.iter().find(|item| item.label == "main.c").unwrap();
    slate
        .handle(VIEW, ViewEvent::Activate(source.key.clone()), &ctx)
        .unwrap();
    assert_eq!(
        host.opened_locations()[0].0,
        "/project/app/examples/hello/src/main.c"
    );

    slate
        .handle(VIEW, ViewEvent::Select(key("run", "app", "hello")), &ctx)
        .unwrap();
    let view = slate.render(VIEW, None, &ctx).unwrap();
    let run = view
        .toolbar
        .iter()
        .find(|button| button.id == "run")
        .unwrap();
    assert!(run.enabled);
    slate
        .handle(VIEW, ViewEvent::Button(run.id.clone()), &ctx)
        .unwrap();
    let runs = host.configuration_runs();
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].1, RUN_CONFIGURATION);
    assert!(!runs[0].3);
    assert_eq!(
        runs[0].2,
        [
            "--root",
            "/project/app",
            "--preset",
            "release",
            "--target",
            "hello"
        ]
    );
    assert!(host.terminals().is_empty());
    slate
        .run_command("build", &ctx, Some(&key("test", "app", "suite")))
        .unwrap();
    assert_eq!(host.terminals()[0].args.last().unwrap(), "--tests");
    slate
        .run_command("test", &ctx, Some(&key("test", "app", "suite")))
        .unwrap();
    assert_eq!(host.terminals()[1].args.last().unwrap(), "suite");
    assert!(
        slate
            .run_command("run", &ctx, Some(&key("target", "app", "math")))
            .is_err()
    );
    assert_eq!(host.terminals().len(), 2);
    assert_eq!(host.configuration_runs().len(), 1);
}

#[test]
fn graph_without_test_dependencies_remains_browsable() {
    let host = workspace();
    host.on_process(PROGRAM, |args, _| {
        if args.last().is_some_and(|arg| arg == "--tests") {
            Ok(ProcessOutput {
                status: Some(1),
                stderr: "missing test SDK".into(),
                ..Default::default()
            })
        } else {
            Ok(ProcessOutput {
                status: Some(0),
                stdout: GRAPH.into(),
                ..Default::default()
            })
        }
    });
    let ctx = host.invocation();
    let mut slate = Slate::default();
    slate.render(VIEW, None, &ctx).unwrap();
    slate
        .handle(VIEW, ViewEvent::Expand(key("project", "app", "")), &ctx)
        .unwrap();
    let view = slate.render(VIEW, None, &ctx).unwrap();
    assert_eq!(
        view.status.as_deref(),
        Some("Tests unavailable: missing test SDK")
    );
    let Body::Tree(rows) = view.body else {
        panic!()
    };
    assert!(rows.iter().any(|item| item.label == "hello"));
}
