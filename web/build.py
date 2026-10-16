#!/usr/bin/env python3
"""Build the web app.

  web/rhemaflow-web.html   one self-contained file (KJV embedded, gzip+base64): double-click or serve it
  web/dist/                the folder to PUBLISH (Cloudflare, Netlify, any static host): the same app as index.html
                           plus manifest, icons, service worker, headers, and the public-domain Bibles (if web/bibles has them)

Run:  python3 web/build.py
"""
import base64, gzip, hashlib, pathlib, shutil

here = pathlib.Path(__file__).parent
src = here / "src"
kjv = (here / "data" / "kjv.json").read_bytes()
b64 = base64.b64encode(gzip.compress(kjv, 9)).decode()
html = (src / "index.html").read_text(encoding="utf-8")
html = html.replace("/*CSS*/", (src / "style.css").read_text(encoding="utf-8"))
html = html.replace("/*CORE*/", (src / "core.js").read_text(encoding="utf-8"))
html = html.replace("/*APP*/", (src / "app.js").read_text(encoding="utf-8"))
html = html.replace("/*KJV*/", b64)
out = here / "rhemaflow-web.html"
out.write_text(html, encoding="utf-8")
print(f"wrote {out}  {out.stat().st_size/1e6:.2f} MB")

# ---- the deploy folder ----
dist = here / "dist"
shutil.rmtree(dist, ignore_errors=True)
(dist / "icons").mkdir(parents=True)
(dist / "index.html").write_text(html, encoding="utf-8")
build_id = hashlib.sha1(html.encode("utf-8")).hexdigest()[:10]  # any change to the app gives a new id, so phones get a fresh copy
(dist / "sw.js").write_text((src / "sw.js").read_text(encoding="utf-8").replace("__BUILD__", build_id), encoding="utf-8")
for name in ("manifest.webmanifest", "_headers"):
    shutil.copy(src / name, dist / name)
for icon in (src / "icons").glob("*.png"):
    shutil.copy(icon, dist / "icons" / icon.name)
bibles = sorted((here / "bibles").glob("*.json")) if (here / "bibles").exists() else []
if bibles:
    (dist / "bibles").mkdir()
    for b in bibles:
        shutil.copy(b, dist / "bibles" / b.name)
total = sum(f.stat().st_size for f in dist.rglob("*") if f.is_file())
print(f"wrote {dist}  build {build_id}  {len(list(dist.rglob('*')))} entries, {total/1e6:.1f} MB, {len(bibles)} Bible file(s)")
