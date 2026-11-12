#!/usr/bin/env python3
"""Render prototype pages and record what is on them: the reference for 1:1 screens.

    tools/proto-reference.py --runtime /path/to/dc-runtime.js [Page ...]

For every page of docs/design/style/*.dc.html (or the ones named) it writes
  docs/design/reference/<Page>.png    the page at 800x480, as the canvas shows it
  docs/design/measure/<Page>.json     every visible element: box, text, font, colours, radius

The pages need the canvas runtime (the Design artifact's `artifact-type/dc-runtime.js`;
the pages load it as ./support.js). It is not kept in the repository: pass its path.
Both outputs are committed, so CI compares against them without a browser.

Needs: Python Playwright and a Chromium (CHROME=/path, else Playwright's headless shell).
"""
import argparse, glob, json, os, pathlib, shutil, sys, tempfile, threading
import http.server, socketserver, functools

ROOT = pathlib.Path(__file__).resolve().parent.parent
STYLE = ROOT / "docs/design/style"
REF = ROOT / "docs/design/reference"
MEASURE = ROOT / "docs/design/measure"

# Runs in the page: every element that is drawn, in document order.
MEASURE_JS = r"""
() => {
  const out = [];
  const px = v => Math.round(parseFloat(v) * 10) / 10;
  for (const el of document.querySelectorAll('body *')) {
    const r = el.getBoundingClientRect();
    if (r.width < 1 || r.height < 1 || r.right <= 0 || r.bottom <= 0 || r.left >= 800 || r.top >= 480) continue;
    const s = getComputedStyle(el);
    if (s.visibility === 'hidden' || s.display === 'none' || parseFloat(s.opacity) === 0) continue;
    const own = [...el.childNodes].filter(n => n.nodeType === 3).map(n => n.textContent.trim()).join(' ').trim();
    const bg = s.backgroundColor;
    const paints = (bg && bg !== 'rgba(0, 0, 0, 0)') || own || ['svg', 'img', 'canvas'].includes(el.tagName.toLowerCase());
    if (!paints) continue;
    const e = { tag: el.tagName.toLowerCase(), x: px(r.left), y: px(r.top), w: px(r.width), h: px(r.height) };
    if (own) Object.assign(e, { text: own, font: px(s.fontSize), weight: +s.fontWeight, color: s.color });
    if (bg && bg !== 'rgba(0, 0, 0, 0)') e.bg = bg;
    if (parseFloat(s.borderTopLeftRadius)) e.radius = px(s.borderTopLeftRadius);
    const label = el.getAttribute('aria-label');
    if (label) e.label = label;
    out.push(e);
  }
  return out;
}
"""

def chrome():
    if os.environ.get("CHROME"):
        return os.environ["CHROME"]
    found = sorted(glob.glob(os.path.expanduser(
        "~/Library/Caches/ms-playwright/chromium_headless_shell-*/*/chrome-headless-shell")))
    return found[-1] if found else None

def serve(folder):
    handler = functools.partial(http.server.SimpleHTTPRequestHandler, directory=folder)
    handler.log_message = lambda *a: None
    srv = socketserver.TCPServer(("127.0.0.1", 0), handler)
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    return srv

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--runtime", required=True, help="dc-runtime.js of the Design artifact")
    ap.add_argument("pages", nargs="*")
    args = ap.parse_args()
    from playwright.sync_api import sync_playwright

    pages = args.pages or sorted(p.name[:-8] for p in STYLE.glob("*.dc.html"))
    REF.mkdir(parents=True, exist_ok=True)
    MEASURE.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory() as tmp:
        for f in STYLE.glob("*.dc.html"):
            shutil.copy(f, tmp)
        shutil.copy(args.runtime, os.path.join(tmp, "support.js"))
        srv = serve(tmp)
        base = f"http://127.0.0.1:{srv.server_address[1]}"
        with sync_playwright() as pw:
            browser = pw.chromium.launch(executable_path=chrome())
            page = browser.new_page(viewport={"width": 800, "height": 480})
            for name in pages:
                page.goto(f"{base}/{name}.dc.html", wait_until="networkidle")
                page.evaluate("document.fonts.ready")
                page.wait_for_timeout(300)
                page.screenshot(path=str(REF / f"{name}.png"))
                items = page.evaluate(MEASURE_JS)
                (MEASURE / f"{name}.json").write_text(
                    json.dumps(items, ensure_ascii=False, indent=0) + "\n", encoding="utf-8")
                print(f"{name}: {len(items)} elements")
            browser.close()
        srv.shutdown()

if __name__ == "__main__":
    main()
