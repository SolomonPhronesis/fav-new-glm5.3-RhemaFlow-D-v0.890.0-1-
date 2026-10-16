#!/usr/bin/env python3
"""Install the OPTIONAL Strong's files for RhemaFlow into ~/.rhemaflow/strongs/ .

The Greek dictionary is already built into the app (CC0). Two things are optional:

  python3 tools/fetch_strongs.py hebrew
      Downloads OpenScriptures' Hebrew Strong's lexicon (HebrewStrong.xml) and converts it to hebrew.json.
      Licence: I could NOT confirm the licence of this file from where I built the app, so it is NOT
      in the zip. The tool shows you where it comes from; please read the repository's licence
      (github.com/openscriptures/HebrewLexicon) before you use it in a church that sells or
      redistributes anything.

  python3 tools/fetch_strongs.py hebrew --from HebrewStrong.xml
      Same, from a copy you already downloaded.

  python3 tools/fetch_strongs.py tagged  my-kjv-strongs.osis.xml
      Checks a Strong's-tagged KJV in OSIS form (words like <w lemma="strong:G3056">Word</w>) and
      installs it as kjv-strongs.xml. See docs/STRONGS.md for where to get one and how to export it.

  python3 tools/fetch_strongs.py status
"""
import json
import os
import re
import sys
import unicodedata
import urllib.request
import xml.etree.ElementTree as ET

HEB_URL = "https://raw.githubusercontent.com/openscriptures/HebrewLexicon/master/HebrewStrong.xml"
DEST = os.path.join(os.path.expanduser("~"), ".rhemaflow", "strongs")
NS = "{http://openscriptures.github.com/morphhb/namespace}"


def flat(el):
    """Plain text of an element; <w src="H1">1</w> becomes the number H1."""
    out = [el.text or ""]
    for c in el:
        tag = c.tag.replace(NS, "")
        if tag == "w" and c.get("src"):
            out.append(c.get("src").split()[0])
        else:
            out.append(flat(c))
        out.append(c.tail or "")
    return re.sub(r"\s+", " ", "".join(out)).strip()


def convert_hebrew(xml_bytes):
    root = ET.fromstring(xml_bytes)
    entries = {}
    for e in root.iter(NS + "entry"):
        num = e.get("id")
        if not num or not re.fullmatch(r"H\d+", num):
            continue
        w = e.find(NS + "w")
        def text_of(tag):
            x = e.find(NS + tag)
            return flat(x) if x is not None else ""
        entries[num] = {
            "l": unicodedata.normalize("NFC", (w.text or "").strip()) if w is not None else "",
            "t": w.get("xlit", "") if w is not None else "",
            "p": w.get("pron", "") if w is not None else "",
            "v": text_of("source").rstrip(";").strip(),
            "d": text_of("meaning"),
            "k": text_of("usage").rstrip("."),
        }
    return {
        "schema": 1,
        "language": "H",
        "source": "OpenScriptures Hebrew Lexicon, Strong's numbers (github.com/openscriptures/HebrewLexicon), from James Strong's 1890 concordance",
        "license": "see github.com/openscriptures/HebrewLexicon (not confirmed by the RhemaFlow author)",
        "entries": entries,
    }


def cmd_hebrew(args):
    if "--from" in args:
        data = open(args[args.index("--from") + 1], "rb").read()
        print("Reading your copy of HebrewStrong.xml")
    else:
        print("Downloading", HEB_URL)
        print("Licence not confirmed by the RhemaFlow author: read the repository's licence before relying on it.")
        data = urllib.request.urlopen(HEB_URL, timeout=60).read()
    doc = convert_hebrew(data)
    os.makedirs(DEST, exist_ok=True)
    out = os.path.join(DEST, "hebrew.json")
    with open(out, "w", encoding="utf-8") as f:
        json.dump(doc, f, ensure_ascii=False, separators=(",", ":"))
    print(f"{len(doc['entries'])} Hebrew entries -> {out}\nRestart RhemaFlow to use it.")


def cmd_tagged(args):
    if not args:
        sys.exit("give the path of an OSIS file")
    text = open(args[0], encoding="utf-8").read()
    if "<osis" not in text:
        sys.exit("That is not an OSIS file (no <osis> found).")
    tagged = len(re.findall(r'lemma="[^"]*strong:[GH]\d+', text))
    verses = len(re.findall(r"<verse\b", text))
    if tagged == 0:
        sys.exit("No Strong's numbers (lemma=\"strong:G1234\") found: this Bible is not word-tagged.")
    os.makedirs(DEST, exist_ok=True)
    out = os.path.join(DEST, "kjv-strongs.xml")
    with open(out, "w", encoding="utf-8") as f:
        f.write(text)
    print(f"{verses} verse tags, {tagged} tagged words -> {out}\nRestart RhemaFlow (or reopen the Strong's panel) to use it.")


def cmd_status(_):
    for name, what in [("hebrew.json", "Hebrew dictionary"), ("kjv-strongs.xml", "Strong's-tagged KJV")]:
        p = os.path.join(DEST, name)
        print(f"{what:22} {'installed (' + str(os.path.getsize(p) // 1024) + ' KB)' if os.path.exists(p) else 'not installed'}   {p}")
    print("Greek dictionary       built into the app (CC0)")


if __name__ == "__main__":
    if len(sys.argv) < 2 or sys.argv[1] not in ("hebrew", "tagged", "status"):
        print(__doc__)
        sys.exit(2)
    {"hebrew": cmd_hebrew, "tagged": cmd_tagged, "status": cmd_status}[sys.argv[1]](sys.argv[2:])
