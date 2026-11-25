# Slate Build for OpenIDEA

Manage [Slate](https://qigao.github.io/slates-build/) build projects from the
Slate tool window in OpenIDEA's left sidebar. Open a workspace, then expand a
project to load its targets from Slate.

- The project tree discovers `slate.json` at the workspace root and in
  subdirectories. Each project keeps its own preset and target selection.
  Targets follow their module folders, with shared libraries outside the
  project root under `external`. Open a target's source, header, or manifest
  from its tree. **Refresh** reloads all expanded projects after shared
  manifests change.
- Expand **Presets** and click a preset to select it. The selection
  is saved per Slate project. **Project default** restores the manifest's
  selection.
- Click the play icon to **Run** an executable or **Test** a test target.
  Click the hammer icon to **Build** a project or target. Hover over an
  icon for its action and target name. The toolbar also offers **Generate**
  and acts on the current selection.
- Actions use the selected project, preset, and target. In workspaces with
  `slate.json` or `apps/slate.json`, the plugin uses OpenIDEA's **Run** panel.
  Workspaces with Slate projects in other locations use a named terminal
  when the Run configuration is unavailable.
- Run-panel launches enable Slate's ANSI status colors. Slate honors
  `NO_COLOR`, and the Run panel also renders ANSI output from child tools.

## Install

Use OpenIDEA 0.1.46 or a later release supporting plugin API 1.9, and Slate
0.1.14 or later. Download the `.oiplugin` package and install it from
**Settings → Plugins → Install Plugin from Disk**.
[Latest GitHub release](https://github.com/qigao/slates-openidea/releases/latest)

Approve the Slate executable when OpenIDEA asks for `process:slate`. Put the
Slate bundle's `bin/` directory on `PATH`, or select the installed executable
in the plugin's permission settings. Slate reads the project's `.env` and
resolves toolchains as it does on the command line.

The plugin reads project files to discover manifests and presets
(`project.read`), saves the selected preset in its own storage (`storage`),
and invokes the approved Slate binary to read graphs and launch commands
(`process:slate`).

## Run targets with arguments

Place `slate.run.json` beside the project's `slate.json` and map target names
to argument arrays. For the [HTTP example](https://qigao.github.io/slates-build/examples.html#make-an-http-request),
start the local server and save this in `apps/slate.run.json`.

```json
{
  "hello": ["tcp://127.0.0.1:8000", "127.0.0.1:8000", "/hello.c"]
}
```

Use a Slate CLI that supports `libraries` for the HTTP example. Select the
`apps` preset that matches the SDKs, then click **Run** on `hello`.

## Develop

Install a current Rust toolchain and the OpenIDEA plugin CLI. Then run:

```sh
rustup target add wasm32-wasip2
cargo test --locked
openidea-plugin build
(cd dist && openidea-plugin pack ..)
```

The SDK is pinned to OpenIDEA v0.1.46. The same WebAssembly component is used
on Windows, Linux, and macOS.
