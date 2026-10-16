#!/usr/bin/env python3
"""Real Chromium: installs the worker, reloads with the network OFF, checks installability. Run after build.py.
   PLAYWRIGHT_BROWSERS_PATH=/opt/pw-browsers python3 web/test/pwa_test.py"""
import http.server, pathlib, socketserver, sys, threading, functools, json
from playwright.sync_api import sync_playwright

DIST = pathlib.Path(__file__).parent.parent / "dist"
fails = 0


def check(ok, name):
    global fails
    print(("PASS " if ok else "FAIL ") + name)
    fails += 0 if ok else 1


class Quiet(http.server.SimpleHTTPRequestHandler):
    def log_message(self, *a):
        pass


def main():
    handler = functools.partial(Quiet, directory=str(DIST))
    httpd = socketserver.ThreadingTCPServer(("127.0.0.1", 0), handler)
    httpd.daemon_threads = True
    port = httpd.server_address[1]
    threading.Thread(target=httpd.serve_forever, daemon=True).start()
    base = f"http://127.0.0.1:{port}/"
    with sync_playwright() as p:
        b = p.chromium.launch()
        ctx = b.new_context()
        page = ctx.new_page()
        errors = []
        page.on("pageerror", lambda e: errors.append(str(e)))
        page.goto(base)
        page.wait_for_selector("#btnStart")
        check(page.title() == "RhemaFlow Web", "the app opens from the deploy folder")
        # the worker installs and takes control
        state = page.evaluate("""async () => { const r = await navigator.serviceWorker.ready; const w = r.active;
            if (w.state !== 'activated') await new Promise((ok) => w.addEventListener('statechange', () => w.state === 'activated' && ok()));
            return w.state; }""")
        check(state == "activated", f"service worker active ({state})")
        page.reload()
        page.wait_for_selector("#btnStart")
        check(page.evaluate("!!navigator.serviceWorker.controller"), "the page is controlled after one reload")
        # installability, straight from Chromium
        cdp = ctx.new_cdp_session(page)
        inst = cdp.send("Page.getInstallabilityErrors")
        check(inst.get("installabilityErrors") == [], f"Chromium finds no installability errors: {inst.get('installabilityErrors')}")
        man = cdp.send("Page.getAppManifest")
        check(not man.get("errors"), f"manifest has no errors: {man.get('errors')}")
        parsed = json.loads(man.get("data") or "{}")
        check(parsed.get("name") == "RhemaFlow", "manifest read by the browser")
        # one Bible version is fetched online (so it gets remembered), one is not
        have_bibles = (DIST / "bibles" / "ASV.json").exists()  # the slim zip has no Bible files
        ok_online = page.evaluate("fetch('bibles/ASV.json').then(r => r.ok ? r.arrayBuffer().then(b => b.byteLength) : -1)") if have_bibles else 0
        if have_bibles:
            check(ok_online > 1_000_000, f"ASV fetched online ({ok_online} bytes)")
        else:
            print("SKIP Bible caching checks (no web/dist/bibles in this copy)")
        # ---- the server is SHUT DOWN (Playwright's set_offline does not reach a service worker's own requests,
        # so it could pass for the wrong reason; with nothing listening there is no way to cheat)
        httpd.shutdown()
        httpd.server_close()
        control = b.new_context().new_page()
        try:
            control.goto(base, timeout=5000)
            control_ok = control.title() == "RhemaFlow Web"
        except Exception:
            control_ok = False
        check(not control_ok, "control: a browser WITHOUT the saved copy cannot open the app now")
        try:
            page.reload()
            page.wait_for_selector("#btnStart", timeout=15000)
            opened = page.title() == "RhemaFlow Web"
        except Exception as e:
            opened = False
            print("   (reload failed:", str(e).splitlines()[0], ")")
        check(opened, "OFFLINE (server stopped): the app still opens")
        if not opened:
            b.close()
            print(f"\n{fails} FAILED")
            sys.exit(1)
        check(page.evaluate("document.querySelectorAll('#trans option').length >= 1"), "OFFLINE: the Bible menu is there (KJV is inside the page)")
        off_cached = page.evaluate("fetch('bibles/ASV.json').then(r => r.ok ? r.arrayBuffer().then(b => b.byteLength) : -1).catch(() => -2)")
        if have_bibles:
            check(off_cached == ok_online, f"OFFLINE: a Bible fetched before is still there ({off_cached})")
        off_new = page.evaluate("fetch('bibles/YLT.json').then(r => r.ok ? 1 : -1).catch(() => -2)")
        check(off_new in (-1, -2), f"OFFLINE: one never fetched is honestly missing ({off_new})")
        check(page.evaluate("!!navigator.serviceWorker.controller"), "OFFLINE: it was the saved copy that answered")
        page.evaluate("document.querySelector('#btnHelp').click()")
        check(not errors, f"no JavaScript errors: {errors}")
        b.close()
    print("\nall passed" if not fails else f"\n{fails} FAILED")
    sys.exit(1 if fails else 0)


main()
