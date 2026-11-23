use super::*;
use openidea_plugin::{ProcessOutput, mock::MockHost};

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
    host.set_run_configurations(
        Slate::default()
            .launch_configurations("/project")
            .into_iter()
            .map(|launch| launch.config)
            .collect(),
    );
    host
}

fn send(slate: &mut Slate, ctx: &Invocation, message: serde_json::Value) {
    slate
        .handle(VIEW, ViewEvent::Message(message.to_string()), ctx)
        .unwrap();
}

fn visible_rows(host: &MockHost) -> Vec<serde_json::Value> {
    let messages = host.view_messages();
    let message: serde_json::Value = serde_json::from_str(&messages.last().unwrap().1).unwrap();
    message["rows"].as_array().unwrap().clone()
}

#[test]
fn groups_targets_keeps_project_presets_and_opens_source_files() {
    let host = workspace();
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
    assert!(matches!(initial.body, Body::Html(_)));
    send(&mut slate, &ctx, serde_json::json!({"action": "ready"}));
    assert_eq!(
        visible_rows(&host)
            .iter()
            .map(|item| item["label"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["app", "other"]
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
    for label in ["Targets", "examples", "hello", "hello", "src"] {
        let rows = visible_rows(&host);
        let item = rows
            .iter()
            .find(|item| item["label"] == label && item["expanded"] == false)
            .unwrap();
        send(
            &mut slate,
            &ctx,
            serde_json::json!({"action": "toggle", "key": item["key"], "expanded": true}),
        );
    }
    let rows = visible_rows(&host);
    assert!(rows.iter().any(|item| item["label"] == "include"));
    let source = rows.iter().find(|item| item["label"] == "main.c").unwrap();
    send(
        &mut slate,
        &ctx,
        serde_json::json!({"action": "activate", "key": source["key"]}),
    );
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
    send(
        &mut slate,
        &ctx,
        serde_json::json!({
            "action": "launch", "command": "run", "key": key("run", "app", "hello")
        }),
    );
    let runs = host.configuration_runs();
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].1, Action::Run.run_configuration());
    assert!(!runs[0].3);
    assert_eq!(
        runs[0].2,
        [
            "--root",
            "/project/app",
            "--preset",
            "release",
            "--target",
            "hello",
            "--tests"
        ]
    );
    slate
        .run_command("build", &ctx, Some(&key("test", "app", "suite")))
        .unwrap();
    send(
        &mut slate,
        &ctx,
        serde_json::json!({
            "action": "launch", "command": "test", "key": key("test", "app", "suite")
        }),
    );
    slate
        .handle(VIEW, ViewEvent::Button("gen".into()), &ctx)
        .unwrap();
    assert!(
        slate
            .run_command("run", &ctx, Some(&key("target", "app", "math")))
            .is_err()
    );
    send(
        &mut slate,
        &ctx,
        serde_json::json!({
            "action": "launch", "command": "build", "key": key("run", "app", "hello")
        }),
    );
    let runs = host.configuration_runs();
    assert_eq!(runs.len(), 5);
    for (run, action, args) in [
        (
            &runs[1],
            Action::Build,
            ["--target", "suite", "--tests"].as_slice(),
        ),
        (&runs[2], Action::Test, ["--target", "suite"].as_slice()),
        (
            &runs[3],
            Action::Generate,
            ["--target", "hello", "--tests"].as_slice(),
        ),
        (
            &runs[4],
            Action::Build,
            ["--target", "hello", "--tests"].as_slice(),
        ),
    ] {
        assert_eq!(run.1, action.run_configuration());
        assert_eq!(
            &run.2[..4],
            ["--root", "/project/app", "--preset", "release"]
        );
        assert_eq!(&run.2[4..], args);
        assert!(!run.3);
    }
    assert!(host.terminals().is_empty());
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
    assert!(matches!(view.body, Body::Html(_)));
    assert!(slate.workspaces["/project"].projects["app"].graph.is_some());
    slate
        .run_command("build", &ctx, Some(&key("run", "app", "hello")))
        .unwrap();
    assert!(
        !host.configuration_runs()[0]
            .2
            .iter()
            .any(|arg| arg == "--tests")
    );
}

#[test]
fn enables_slate_ansi_without_replacing_color_preferences() {
    let configs = Slate::default().launch_configurations("/project");
    assert_eq!(configs.len(), Action::ALL.len());
    for (launch, action) in configs.iter().zip(Action::ALL) {
        assert_eq!(launch.config.id, action.run_configuration());
        assert_eq!(launch.config.args, [action.id()]);
        assert_eq!(launch.env, [("SLATE_CONSOLE".into(), "1".into())]);
    }
}

#[test]
fn launches_with_the_same_arguments_during_run_configuration_refresh() {
    let host = workspace();
    host.set_run_configurations(vec![PluginRunConfiguration {
        id: CLI_CONFIGURATION.into(),
        program: PROGRAM.into(),
        ..Default::default()
    }]);
    let ctx = host.invocation();
    Slate::default()
        .run_command("build", &ctx, Some(&key("project", "app", "")))
        .unwrap();
    let runs = host.configuration_runs();
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].1, CLI_CONFIGURATION);
    assert_eq!(runs[0].2, ["build", "--root", "/project/app"]);
}
