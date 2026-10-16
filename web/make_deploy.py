#!/usr/bin/env python3
"""Make `deploy/` = landing page at / and the web app at /app/ . Run web/build.py first.
Cloudflare Pages: build command `exit 0`, output folder `deploy`."""
import pathlib, shutil
root = pathlib.Path(__file__).parent.parent
out = root / "deploy"
shutil.rmtree(out, ignore_errors=True)
shutil.copytree(root / "web" / "dist", out / "app")
for f in (root / "site").glob("*"):
    shutil.copy(f, out / f.name)
(out / "_headers").write_text("""/app/sw.js
  Cache-Control: no-cache
/app/index.html
  Cache-Control: no-cache
/app/manifest.webmanifest
  Content-Type: application/manifest+json
  Cache-Control: no-cache
/*
  X-Content-Type-Options: nosniff
  Referrer-Policy: same-origin
  Permissions-Policy: microphone=(self), camera=(), geolocation=()
""")
print("wrote", out, sum(1 for _ in out.rglob("*") if _.is_file()), "files")
