#!/usr/bin/env python3
"""Turn Ulrik Sandborg-Petersen's Strong's Greek Dictionary XML (CC0, public domain) into the small
JSON file RhemaFlow embeds (crates/rf-strongs/data/greek.json).

  curl -L -o strongsgreek.xml https://raw.githubusercontent.com/morphgnt/strongs-dictionary-xml/master/strongsgreek.xml
  python3 tools/make_strongs_greek.py strongsgreek.xml crates/rf-strongs/data/greek.json

Source licence (that repository's README, read 2026-10-08): "Released under the Creative Commons
CC0 waiver ... Ulrik Sandborg-Petersen has waived all copyright and related or neighboring rights".
The 1890 text underneath is James Strong's, public domain.
"""
import json
import re
import sys
import unicodedata
import xml.etree.ElementTree as ET


def flat(el):
    """Plain text of an element: <greek unicode=..> becomes the Greek, <strongsref strongs=..> the number."""
    out = [el.text or ""]
    for c in el:
        if c.tag == "greek":
            out.append(c.get("unicode", ""))
        elif c.tag == "strongsref":
            out.append(("G" if c.get("language") == "GREEK" else "H") + c.get("strongs", "").lstrip("0"))
        elif c.tag == "pronunciation":
            out.append(c.get("strongs", ""))
        else:
            out.append(flat(c))
        out.append(c.tail or "")
    return re.sub(r"\s+", " ", "".join(out)).strip()


def text_of(e, tag):
    x = e.find(tag)
    return flat(x) if x is not None else ""


def main(src, dst):
    root = ET.parse(src).getroot()
    entries = {}
    for e in root.iter("entry"):
        num = "G" + e.get("strongs").lstrip("0")
        g = e.find("greek")
        p = e.find("pronunciation")
        entries[num] = {
            "l": unicodedata.normalize("NFC", g.get("unicode", "")) if g is not None else "",
            "t": g.get("translit", "") if g is not None else "",
            "p": p.get("strongs", "") if p is not None else "",
            "v": text_of(e, "strongs_derivation"),
            "d": text_of(e, "strongs_def"),
            "k": re.sub(r"^[:\-\s]+|[\s.]+$", "", text_of(e, "kjv_def")),
        }
    doc = {
        "schema": 1,
        "language": "G",
        "source": "Strong's Greek Dictionary in XML with real Greek, Ulrik Sandborg-Petersen (github.com/morphgnt/strongs-dictionary-xml), from James Strong's 1890 Exhaustive Concordance",
        "license": "CC0 1.0 (public domain dedication)",
        "entries": entries,
    }
    with open(dst, "w", encoding="utf-8") as f:
        json.dump(doc, f, ensure_ascii=False, separators=(",", ":"))
    print(f"{len(entries)} Greek entries -> {dst}")


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
