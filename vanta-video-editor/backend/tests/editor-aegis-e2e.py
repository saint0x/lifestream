#!/usr/bin/env python3
import json
import os
import sys
import time
import urllib.error
import urllib.request


AEGIS = os.environ.get("AEGIS_SERVER", "http://127.0.0.1:7878").rstrip("/")
FRONTEND = os.environ.get("VANTA_EDITOR_FRONTEND_URL", "http://127.0.0.1:5178/?aegis-e2e=1")


def request(method: str, path: str, body=None):
    data = None if body is None else json.dumps(body).encode()
    req = urllib.request.Request(
        f"{AEGIS}{path}",
        data=data,
        method=method,
        headers={"Content-Type": "application/json"},
    )
    try:
        with urllib.request.urlopen(req, timeout=20) as response:
            raw = response.read()
    except urllib.error.HTTPError as error:
        detail = error.read().decode(errors="replace")
        raise AssertionError(f"{method} {path} failed: {error.code} {detail}") from error
    return json.loads(raw.decode() or "{}")


def execute(code: str):
    result = request("POST", "/execute", {"commands": [{"type": "eval", "code": code}]})
    item = result["results"][0]
    if not item.get("ok"):
        raise AssertionError(f"Aegis eval failed: {item}")
    return item.get("value")


def text():
    value = execute("({ text: document.body.innerText })")
    return value.get("text", "") if isinstance(value, dict) else ""


def wait_for(label: str, predicate, timeout: float = 15.0):
    deadline = time.time() + timeout
    last = None
    while time.time() < deadline:
        last = predicate()
        if last:
            return last
        time.sleep(0.25)
    raise AssertionError(f"Timed out waiting for {label}. Last value: {last!r}")


def click_button(text_value: str, occurrence: str = "first"):
    code = f"""
(() => {{
  const wanted = {json.dumps(text_value)};
  const buttons = [...document.querySelectorAll('button')].filter((button) =>
    (button.textContent || '').trim() === wanted || button.title === wanted
  );
  const button = { "buttons[buttons.length - 1]" if occurrence == "last" else "buttons[0]" };
  if (!button) return {{ clicked: false, wanted, available: [...document.querySelectorAll('button')].map((b) => (b.textContent || b.title || '').trim()).filter(Boolean) }};
  button.click();
  return {{ clicked: true, wanted }};
}})()
"""
    result = execute(code)
    if not isinstance(result, dict) or not result.get("clicked"):
        raise AssertionError(f"Button {text_value!r} was not clicked: {result}")


def click_selector(selector: str):
    code = f"""
(() => {{
  const element = document.querySelector({json.dumps(selector)});
  if (!element) return {{ clicked: false, selector: {json.dumps(selector)} }};
  element.click();
  return {{ clicked: true, selector: {json.dumps(selector)} }};
}})()
"""
    result = execute(code)
    if not isinstance(result, dict) or not result.get("clicked"):
        raise AssertionError(f"Selector {selector!r} was not clicked: {result}")


def drag_timeline_clip(mode: str):
    start_code = f"""
(() => {{
  const mode = {json.dumps(mode)};
  const clip = document.querySelector('.ve-clip');
  if (!clip) return {{ ok: false, reason: 'missing timeline clip' }};

  Element.prototype.setPointerCapture = function() {{}};
  const before = {{ left: clip.style.left, width: clip.style.width }};
  const rect = clip.getBoundingClientRect();
  const startX = mode === 'trim-end' ? rect.right - 2 : rect.left + (rect.width / 2);
  const delta = mode === 'trim-end' ? 36 : 30;
  const pointerId = Math.floor(Math.random() * 100000) + 1;
  window.__vantaAegisDrag = {{
    mode,
    before,
    pointerId,
    startX,
    endX: startX + delta,
    clientY: rect.top + (rect.height / 2),
  }};
  clip.dispatchEvent(new PointerEvent('pointerdown', {{
    bubbles: true,
    cancelable: true,
    pointerId,
    pointerType: 'mouse',
    isPrimary: true,
    clientX: startX,
    clientY: window.__vantaAegisDrag.clientY,
  }}));
  return {{ ok: true, ...window.__vantaAegisDrag }};
}})()
"""
    move_code = """
(() => {
  const drag = window.__vantaAegisDrag;
  const timeline = document.querySelector('.ve-timeline');
  if (!drag || !timeline) return { ok: false, reason: 'missing drag state' };
  timeline.dispatchEvent(new PointerEvent('pointermove', {
    bubbles: true,
    cancelable: true,
    pointerId: drag.pointerId,
    pointerType: 'mouse',
    isPrimary: true,
    clientX: drag.endX,
    clientY: drag.clientY,
  }));
  return { ok: true };
})()
"""
    end_code = """
(() => {
  const drag = window.__vantaAegisDrag;
  const timeline = document.querySelector('.ve-timeline');
  const clip = document.querySelector('.ve-clip');
  if (!drag || !timeline || !clip) return { ok: false, reason: 'missing drag state' };
  timeline.dispatchEvent(new PointerEvent('pointerup', {
    bubbles: true,
    cancelable: true,
    pointerId: drag.pointerId,
    pointerType: 'mouse',
    isPrimary: true,
    clientX: drag.endX,
    clientY: drag.clientY,
  }));
  const after = { left: clip.style.left, width: clip.style.width };
  const result = { ok: drag.before.left !== after.left || drag.before.width !== after.width, mode: drag.mode, before: drag.before, after };
  window.__vantaAegisDrag = null;
  return result;
})()
"""
    start = execute(start_code)
    if not isinstance(start, dict) or not start.get("ok"):
        raise AssertionError(f"Timeline {mode} drag did not start: {start}")
    time.sleep(0.1)
    moved = execute(move_code)
    if not isinstance(moved, dict) or not moved.get("ok"):
        raise AssertionError(f"Timeline {mode} drag did not move: {moved}")
    time.sleep(0.1)
    result = execute(end_code)
    time.sleep(0.25)
    if not isinstance(result, dict) or not result.get("ok"):
        raise AssertionError(f"Timeline {mode} drag did not change the clip: {result}")
    return result


def main():
    request("GET", "/version")
    request("POST", "/navigate", {"url": FRONTEND})
    wait_for("editor shell", lambda: ("RENDER SAFE" in text() and "Split" in text()))

    click_button("renders")
    wait_for("render panel", lambda: "Render advertiser cut" in text())

    click_button("comments")
    wait_for("comments panel", lambda: "Add frame comment" in text())
    click_button("Add frame comment")
    wait_for("comment creation", lambda: "Frame comment added" in text() or "Review this frame" in text())

    click_button("transcript")
    wait_for("transcript panel", lambda: "Before the next sequence" in text() or "This is where" in text())
    click_selector(".ve-transcript")
    time.sleep(0.5)

    move_result = drag_timeline_clip("move")
    wait_for("timeline move save", lambda: "Timeline edit saved" in text())
    trim_result = drag_timeline_clip("trim-end")
    wait_for("timeline trim save", lambda: "Timeline edit saved" in text())

    click_button("Frame", occurrence="last")
    time.sleep(0.5)
    click_button("Split")
    wait_for("split persisted", lambda: "Clip split saved" in text())

    click_button("Save")
    wait_for("version save", lambda: "Timeline version saved" in text())

    click_button("Render")
    wait_for("render request", lambda: "Render completed" in text() or "Render running" in text() or "Render waiting" in text(), timeout=20)

    click_button("renders")
    wait_for("rendered export actions", lambda: "Proof" in text() and "Review" in text() and "Publish" in text())
    click_button("Proof")
    wait_for("proof link", lambda: "Proof link ready" in text())
    click_button("Review")
    wait_for("ad hub review", lambda: "Advertiser room submitted" in text())
    click_button("Publish")
    wait_for("publish notice", lambda: "Export published into Vanta media pipeline" in text())

    snapshot = request("GET", "/page")
    controls = snapshot.get("semantic_summary", {}).get("control_count", 0)
    if controls < 8:
        raise AssertionError(f"Expected interactive editor controls, saw {controls}")
    print(json.dumps({
        "ok": True,
        "frontend": FRONTEND,
        "controls": controls,
        "timeline_move": move_result,
        "timeline_trim": trim_result,
        "url": snapshot.get("url"),
        "title": snapshot.get("title"),
    }))


if __name__ == "__main__":
    try:
        main()
    except Exception as exc:
        print(f"editor-aegis-e2e=failed: {exc}", file=sys.stderr)
        raise
