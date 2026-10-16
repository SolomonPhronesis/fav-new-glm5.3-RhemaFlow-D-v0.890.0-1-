#!/usr/bin/env python3
"""Fetch a Bible version into RhemaFlow's Bibles folder.

  python3 tools/fetch_bible.py list                       # your API.Bible versions
  python3 tools/fetch_bible.py api  <bibleId> NIV         # needs API_BIBLE_KEY
  python3 tools/fetch_bible.py pd   ASV                   # public-domain, no key

Why a script and not a button: NIV, ESV, NKJV, NLT ... are LICENSED. RhemaFlow can
neither bundle nor scrape them. If YOU hold access (for example a free key from
https://scripture.api.bible and its terms, which limit how much text may be cached
and shown), this fetches the text with your key and writes <Name>.json into the
Bibles folder, where the app picks it up (menu: Bible versions -> Rescan).

STATUS: the `pd` path is the same download the app does and was tested. The `api`
path follows the API.Bible v1 documentation but was NOT run against the live service
(no key here) - treat the first run as a trial and read the output.

Output layout: {"books":[{"name":"Genesis","chapters":[["verse 1","verse 2"],...]}, ...]}
"""
import json, os, re, sys, time, urllib.request, urllib.error, pathlib

API = "https://api.scripture.api.bible/v1"
PD = "https://raw.githubusercontent.com/scrollmapper/bible_databases/master/formats/json"
PD_NAMES = ["ASV", "YLT", "Darby", "Webster", "Wycliffe", "DRC", "Rotherham", "Noyes", "AKJV", "BBE", "KJVA"]

# canonical OSIS book ids in Bible order (API.Bible uses these for books)
OSIS = ("GEN EXO LEV NUM DEU JOS JDG RUT 1SA 2SA 1KI 2KI 1CH 2CH EZR NEH EST JOB PSA PRO ECC SNG ISA JER LAM EZK DAN "
        "HOS JOL AMO OBA JON MIC NAM HAB ZEP HAG ZEC MAL MAT MRK LUK JHN ACT ROM 1CO 2CO GAL EPH PHP COL 1TH 2TH 1TI "
        "2TI TIT PHM HEB JAS 1PE 2PE 1JN 2JN 3JN JUD REV").split()


def bibles_dir():
    home = pathlib.Path.home()
    if sys.platform == "darwin":
        return home / "Library" / "Application Support" / "RhemaFlow" / "Bibles"
    return home / ".local" / "share" / "RhemaFlow" / "Bibles"


def get(url, headers=None, tries=4):
    for n in range(tries):
        try:
            req = urllib.request.Request(url, headers=headers or {})
            with urllib.request.urlopen(req, timeout=60) as r:
                return r.read()
        except urllib.error.HTTPError as e:
            if e.code in (429, 500, 502, 503) and n < tries - 1:
                time.sleep(2 ** n * 2)
                continue
            raise
        except urllib.error.URLError:
            if n < tries - 1:
                time.sleep(2 ** n)
                continue
            raise


def save(name, books):
    out = bibles_dir()
    out.mkdir(parents=True, exist_ok=True)
    path = out / f"{name}.json"
    tmp = out / f".{name}.part"
    tmp.write_text(json.dumps({"books": books}, ensure_ascii=False, separators=(",", ":")), encoding="utf-8")
    tmp.replace(path)
    total = sum(len(c) for b in books for c in b["chapters"])
    print(f"saved {path}  ({total} verses)")
    if not 30000 <= total <= 32500:
        print("WARNING: that is not a complete 66-book Bible; the app will refuse it.")


def cmd_list():
    key = os.environ.get("API_BIBLE_KEY")
    if not key:
        sys.exit("set API_BIBLE_KEY first (free key: https://scripture.api.bible)")
    data = json.loads(get(f"{API}/bibles?language=eng", {"api-key": key}))["data"]
    for b in data:
        print(f'{b["id"]}  {b.get("abbreviation","")}  {b.get("name","")}')


def cmd_api(bible_id, name):
    key = os.environ.get("API_BIBLE_KEY")
    if not key:
        sys.exit("set API_BIBLE_KEY first")
    h = {"api-key": key}
    books_meta = json.loads(get(f"{API}/bibles/{bible_id}/books?include-chapters=true", h))["data"]
    by_id = {b["id"]: b for b in books_meta}
    books = []
    for i, osis in enumerate(OSIS):
        b = by_id.get(osis)
        if not b:
            sys.exit(f"book {osis} missing in this version (API.Bible lists {len(books_meta)} books)")
        chapters = []
        for ch in b["chapters"]:
            if ch["number"] == "intro":
                continue
            q = "content-type=text&include-notes=false&include-titles=false&include-chapter-numbers=false&include-verse-numbers=true"
            txt = json.loads(get(f'{API}/bibles/{bible_id}/chapters/{ch["id"]}?{q}', h))["data"]["content"]
            # verse numbers arrive as "[1] text [2] text"
            parts = re.split(r"\[(\d+)\]\s*", txt)
            verses = {}
            for n in range(1, len(parts) - 1, 2):
                verses[int(parts[n])] = " ".join(parts[n + 1].split())
            if not verses:
                sys.exit(f'no verses parsed for {b["name"]} {ch["number"]}; the text format differs from what this script expects')
            top = max(verses)
            chapters.append([verses.get(v, "") for v in range(1, top + 1)])
            time.sleep(0.12)  # stay well under the rate limit
        books.append({"name": b["name"], "chapters": chapters})
        print(f'{i+1:2}/66 {b["name"]}', flush=True)
    save(name, books)


def cmd_pd(name):
    if name not in PD_NAMES:
        sys.exit("choose one of: " + ", ".join(PD_NAMES))
    data = json.loads(get(f"{PD}/{name}.json").decode("utf-8-sig"))
    books = []
    for b in data["books"]:
        chs = []
        for c in b["chapters"]:
            vs = sorted(c["verses"], key=lambda v: v["verse"])
            chs.append([" ".join(v["text"].split()) for v in vs])
        books.append({"name": b["name"], "chapters": chs})
    save(name, books)


if __name__ == "__main__":
    a = sys.argv[1:]
    if a[:1] == ["list"]:
        cmd_list()
    elif a[:1] == ["api"] and len(a) == 3:
        cmd_api(a[1], a[2])
    elif a[:1] == ["pd"] and len(a) == 2:
        cmd_pd(a[1])
    else:
        print(__doc__)
