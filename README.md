# Slate Build for OpenIDEA

Browse, build, test, and run Slate targets from OpenIDEA's left sidebar.

## Quick start

1. In OpenIDEA, open **Settings → Plugins → Available** and install **Slate Build**.
   For a manual install, download the [`.oiplugin` package](https://github.com/qigao/slates-openidea/releases/latest)
   and select **Settings → Plugins → Install Plugin from Disk**.
2. Install [the Slate CLI](https://qigao.github.io/slates-build/), Ninja, and
   your project's compiler. Ensure `slate --version` works in a terminal, or
   select the executable in the plugin's permission settings when OpenIDEA
   asks you to approve `process:slate`.
3. Open the directory containing `slate.json` or a parent directory. Click
   **Slate** in the left sidebar, expand your project, and open **Targets**.
4. Choose a preset under **Presets** if needed. Use the tools icon to build
   a target, the play icon to run an executable, or the beaker icon to run a test.

Output appears in the **Run** panel when an OpenIDEA launch configuration is
available, or in a named terminal for other project layouts.

Requires OpenIDEA 0.1.46 or later with plugin API 1.9 and Slate 0.1.14 or later.

## Browse projects and choose a preset

The project tree discovers `slate.json` at the workspace root and in
subdirectories and nests projects under their directories. For example,
`apps/fetch/slate.json` appears under **apps → fetch**. A project can contain
further projects, and its own targets stay in its **Targets** branch.
Each project keeps its own preset and target selection.
The tree uses the same 16px Codicons and Slate sidebar mark as the VS Code
extension. The gear on a project generates Ninja files; the preset button
opens its **Presets** branch. Row actions appear on hover, selection, or
keyboard focus. Colors and fonts follow the OpenIDEA theme.
Targets follow their module folders, with shared libraries outside the
project root under `external`. Open a target's source, header, or manifest
from its tree. **Refresh** reloads all expanded projects after shared
manifests change.

Choose **Project default** to restore the manifest's preset selection. The
toolbar also offers **Generate** and acts on the current selection. Slate
reads the project's `.env` and resolves toolchains as it does on the command
line. Run-panel launches enable Slate's ANSI status colors and honor
`NO_COLOR`.

## Run targets with arguments

Place `slate.run.json` beside the project's `slate.json` and map target names
to argument arrays. For a target named `my_tool`, each array entry is one
process argument.

```json
{
  "my_tool": ["--input", "file with spaces.txt"]
}
```

To run the repository's
[SDK walkthrough](https://qigao.github.io/slates-build/examples.html#export-and-consume-an-sdk),
install the SDK, expand **apps → consumer**, and run `stats-demo` with the
matching toolchain preset. That program needs no run arguments.

## Permissions

The plugin reads project files to discover manifests and presets
(`project.read`), saves the selected preset in its own storage (`storage`),
and invokes the approved Slate binary to read graphs and launch commands
(`process:slate`).

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

To check the HTML tree in a browser, install Python Playwright and run
`python tests/tree_ui.py`. It checks light, dark, high contrast, narrow layouts,
inline actions, and keyboard focus. Screenshots are saved under `.dev/tree-ui`.

See [third-party notices](NOTICE.md) for the bundled Codicons.
