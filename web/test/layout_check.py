import sys, asyncio
from playwright.sync_api import sync_playwright
path = sys.argv[1]; out = sys.argv[2]; w=int(sys.argv[3]); h=int(sys.argv[4])
with sync_playwright() as p:
    b = p.chromium.launch(args=["--no-sandbox"])
    pg = b.new_page(viewport={"width": w, "height": h}, device_scale_factor=1)
    logs=[]
    pg.on("console", lambda m: logs.append(m.text)); pg.on("pageerror", lambda e: logs.append("ERR "+str(e)))
    pg.goto("file://"+path)
    pg.wait_for_function("document.body.dataset.ready==='1'", timeout=20000)
    pg.evaluate("window.__rf.handleFinal('Genesis chapter one verse 4')")
    pg.wait_for_timeout(400)
    pg.screenshot(path=out)
    r = pg.evaluate("""()=>{const q=s=>{const e=document.querySelector(s); if(!e) return null; const r=e.getBoundingClientRect(); return [Math.round(r.x),Math.round(r.y),Math.round(r.width),Math.round(r.height), getComputedStyle(e).display]};
      return {top:q('#top'), main:q('#main'), left:q('#left'), center:q('#center'), right:q('#right'), browser:q('#browser'), cols:q('#cols'), books:q('#books'), verses:q('#verses'), win:[innerWidth,innerHeight], docH:document.documentElement.scrollHeight}}""")
    print(r); print(logs[:5])
    b.close()
