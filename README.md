# Slate Build for OpenIDEA

Manage [Slate](https://qigao.github.io/slates-build/) build projects from the
Slate tool window in OpenIDEA's left sidebar. Open a workspace, then expand a
project to load its targets from Slate.

- The project tree discovers `slate.json` at the workspace root and in
  subdirectories. Targets follow their module folders. Double-click a source
  or header to open it.
- Expand **Presets** and click a preset to select it. The selection
  is saved per Slate project. **Project default** restores the manifest's
  selection.
- Click **Run** on an executable, **Build** on a project or target, or
  **Test** on a test target. The toolbar also offers **Generate** and
  acts on the current selection.
- Each action streams output to a named tab in OpenIDEA's **Run** tool
  window, with Stop and Rerun controls. The plugin supplies the selected
  project, preset, and target. Open a workspace containing `slate.json`
  or `apps/slate.json` to use these configurations.
- Run-panel launches enable Slate's ANSI status colors. Slate honors
  `NO_COLOR`, and the Run panel also renders ANSI output from child tools.

## Install

Use OpenIDEA 0.1.46 or a later release supporting plugin API 1.9, and Slate
0.1.14 or later. Install the `.oiplugin` package from **Settings → Plugins →
Install Plugin from Disk**. A package for each successful build is attached
to the [repository's GitHub Actions runs](https://github.com/qigao/slates-openidea/actions).

Approve the Slate executable when OpenIDEA asks for `process:slate`. Put the
Slate bundle's `bin/` directory on `PATH`, or select the installed executable
in the plugin's permission settings. Slate reads the project's `.env` and
resolves toolchains as it does on the command line.

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
