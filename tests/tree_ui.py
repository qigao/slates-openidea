"""Browser smoke check for the sandboxed Slate tree (requires Playwright)."""

import json
import os
import sys
from pathlib import Path

from playwright.sync_api import expect, sync_playwright


ROOT = Path(__file__).resolve().parents[1]
WORKSPACE = "C:/smoke workspace"
html = (ROOT / "src/tree.html").read_text(encoding="utf-8")
html = html.replace("/*WORKSPACE*/null", json.dumps(WORKSPACE))
html = html.replace("/*ICONS*/{}", (ROOT / "src/icons.json").read_text(encoding="utf-8"))
bootstrap = """<!doctype html><meta charset="utf-8">
<meta http-equiv="Content-Security-Policy" content="default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; img-src data:; font-src data:">
<script>
window.messages = [];
window.openidea = {
  postMessage(text) { messages.push(JSON.parse(text)); },
  onMessage(callback) { window.receive = data => callback(JSON.stringify(data)); }
};
</script>"""
rows = [
    dict(key="root", parent=None, label="rime", kind="project", icon="folder", expandable=True,
         expanded=True, detail="workspace · windows-x64 · msvc"),
    dict(key="apps", parent="root", label="apps", kind="directory", icon="folder", expandable=True, expanded=True),
    dict(key="nested", parent="apps", label="handwriting", kind="project", icon="folder", expandable=True,
         expanded=False, detail="release-msvc · windows-x64 · msvc"),
    dict(key="targets", parent="root", label="Targets", kind="targets", icon="folder", expandable=True, expanded=True),
    dict(key="folder", parent="targets", label="ime", kind="folder", icon="folder", expandable=True, expanded=True),
    dict(key="run", parent="folder", label="rime_console", kind="run", icon="file-code", expandable=True, expanded=False, detail="executable"),
    dict(key="library", parent="folder", label="librime", kind="target", icon="package", expandable=False, expanded=False, detail="shared"),
    dict(key="suite", parent="folder", label="rime_test", kind="test", icon="beaker", expandable=False, expanded=False, detail="executable"),
    dict(key="source", parent="folder", label="main.cpp", kind="file", icon="file", expandable=False, expanded=False),
    dict(key="malformed", parent="folder", label="<img src=x onerror=alert(1)>", kind="target", icon="package", expandable=False, expanded=False),
]
snapshot = dict(workspace=WORKSPACE, rows=rows, selected="library")
themes = {
    "dark": {"bg": "#1f1f1f", "fg": "#cccccc", "fg-strong": "#ffffff", "fg-muted": "#a1a1a1",
             "bg-hover": "#2a2d2e", "bg-selected": "#37373d", "focus-ring": "#6ba5e7"},
    "light": {"bg": "#f8f8f8", "fg": "#333333", "fg-strong": "#1f1f1f", "fg-muted": "#616161",
              "bg-hover": "#e8e8e8", "bg-selected": "#e4e6f1", "focus-ring": "#005fb8"},
}


with sync_playwright() as playwright:
    channel = os.environ.get("SLATE_TEST_BROWSER_CHANNEL", "msedge" if sys.platform == "win32" else None)
    browser = playwright.chromium.launch(channel=channel, headless=True)
    page = browser.new_page(viewport={"width": 420, "height": 430})
    errors, requests = [], []
    page.on("pageerror", lambda error: errors.append(str(error)))
    page.on("request", lambda request: requests.append(request.url))
    page.set_content(bootstrap + html)
    assert page.evaluate("messages[0].action") == "ready"
    page.evaluate("data => receive(data)", dict(workspace="another workspace", rows=rows))
    assert page.get_by_role("button").count() == 0
    page.evaluate("data => receive(data)", snapshot)
    assert page.locator("img").count() == 0
    assert page.locator(".label > .icon svg").count() == len(rows)
    assert page.locator(".icon:not([aria-hidden=true])").count() == 0
    assert page.locator(".icon svg:not([focusable=false])").count() == 0

    def button(label):
        return page.get_by_role("button", name=label, exact=True)

    def action(label, expected):
        control = button(label)
        page.mouse.move(419, 429)
        control.focus()
        assert control.evaluate("e => getComputedStyle(e.parentElement).opacity") == "1"
        box = control.bounding_box()
        assert box["width"] >= 24 and box["height"] >= 24
        assert control.locator("svg").count() == 1
        page.keyboard.press("Enter")
        assert page.evaluate("messages.at(-1)") == expected

    page.mouse.move(419, 429)
    assert button("Run rime_console").evaluate("e => getComputedStyle(e.parentElement).opacity") == "0"
    button("rime_console").hover()
    assert button("Run rime_console").evaluate("e => getComputedStyle(e.parentElement).opacity") == "1"
    action("Build librime", dict(action="launch", command="build", key="library"))
    action("Run rime_console", dict(action="launch", command="run", key="run"))
    action("Test rime_test", dict(action="launch", command="test", key="suite"))
    action("Generate Ninja files for rime", dict(action="launch", command="gen", key="root"))
    action("Select preset for handwriting", dict(action="presets", key="nested"))
    button("Build librime").focus()
    page.keyboard.press("Space")
    assert page.evaluate("messages.at(-1)") == dict(action="launch", command="build", key="library")
    button("Expand rime_console").click()
    assert page.evaluate("messages.at(-1)") == dict(action="toggle", expanded=True, key="run")
    page.evaluate("data => receive(data)", snapshot)
    expect(button("Expand rime_console")).to_be_focused()
    button("main.cpp").click()
    assert page.evaluate("messages.at(-1)") == dict(action="activate", key="source")
    assert button("Run librime").count() == 0

    output = ROOT / ".dev/tree-ui"
    output.mkdir(parents=True, exist_ok=True)
    for name, variables in themes.items():
        page.evaluate("""({ name, variables }) => {
          document.documentElement.dataset.theme = name;
          document.documentElement.style.colorScheme = name;
          for (const [key, value] of Object.entries(variables))
            document.documentElement.style.setProperty('--openidea-' + key, value);
          document.body.style.background = 'var(--openidea-bg)';
        }""", dict(name=name, variables=variables))
        for width in (420, 320, 240):
            page.set_viewport_size(dict(width=width, height=430))
            assert page.evaluate("document.documentElement.scrollWidth <= innerWidth")
        page.set_viewport_size(dict(width=420, height=430))
        page.mouse.move(419, 429)
        page.keyboard.press("Tab")
        button("Build librime").focus()
        assert button("Build librime").evaluate("e => getComputedStyle(e).outlineStyle") == "solid"
        page.screenshot(path=str(output / f"{name}.png"))
    page.emulate_media(forced_colors="active", reduced_motion="reduce")
    page.screenshot(path=str(output / "high-contrast.png"))
    assert button("Build librime").evaluate("e => getComputedStyle(e).outlineStyle") == "solid"
    assert errors == [], errors
    assert requests == [], requests
    browser.close()

print("Codicons, themes, inline actions, keyboard focus and narrow layouts passed.")
