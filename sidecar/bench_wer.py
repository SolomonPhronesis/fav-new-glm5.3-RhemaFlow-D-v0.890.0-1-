#!/usr/bin/env python3
"""RhemaFlow ClearSpeech — WER / accuracy benchmark (pure Python, no deps).

Measures word accuracy of a transcript against what was ACTUALLY said.
Accuracy % = 100 - WER%.  WER = (substitutions + deletions + insertions) / words.

Two ways to use it:

1) Compare draft vs corrected (the before/after table):

     python3 sidecar/bench_wer.py compare \
         --ref  reference.txt \
         --hyp-before draft.txt \
         --hyp-after  corrected.txt

   reference.txt = exactly what was said (paste from the Bible passage, or type it)
   draft.txt     = the streaming draft  (Order tab > transcript line BEFORE v0.843,
                   or the HEARD strip text — the "before" engine)
   corrected.txt = the final-pass line from v0.843 (the "after" engine)

2) Score one transcript:

     python3 sidecar/bench_wer.py wer --ref reference.txt --hyp "heard text here"

Self-test:  python3 sidecar/bench_wer.py selftest
"""
import argparse
import re
import sys


_NUMBERS = {
    "zero": "0", "one": "1", "two": "2", "three": "3", "four": "4", "five": "5",
    "six": "6", "seven": "7", "eight": "8", "nine": "9", "ten": "10",
    "eleven": "11", "twelve": "12", "thirteen": "13", "fourteen": "14",
    "fifteen": "15", "sixteen": "16", "seventeen": "17", "eighteen": "18",
    "nineteen": "19", "twenty": "20", "thirty": "30", "forty": "40", "forty": "40",
    "fifty": "50", "sixty": "60", "seventy": "70", "eighty": "80", "ninety": "90",
    "hundred": "100",
}

def normalize(text: str):
    """Lowercase, strip punctuation, number words == digits ("three" == "3") —
    a fair norm for scripture work, where the draft may hear either form."""
    text = text.lower()
    text = re.sub(r"[’‘]", "'", text)
    text = re.sub(r"[^a-z0-9' ]+", " ", text)
    return [_NUMBERS.get(w, w) for w in text.split() if w]


def levenshtein_ops(ref, hyp):
    """Minimum (substitutions, deletions, insertions) turning ref into hyp."""
    n, m = len(ref), len(hyp)
    # dp[i][j] = (cost, subs, dels, ins)
    dp = [[(0, 0, 0, 0)] * (m + 1) for _ in range(n + 1)]
    for i in range(1, n + 1):
        c, s, d, ins = dp[i - 1][0]
        dp[i][0] = (c + 1, s, d + 1, ins)
    for j in range(1, m + 1):
        c, s, d, ins = dp[0][j - 1]
        dp[0][j] = (c + 1, s, d, ins + 1)
    for i in range(1, n + 1):
        for j in range(1, m + 1):
            if ref[i - 1] == hyp[j - 1]:
                dp[i][j] = dp[i - 1][j - 1]
            else:
                sub = (dp[i - 1][j - 1][0] + 1, dp[i - 1][j - 1][1] + 1, dp[i - 1][j - 1][2], dp[i - 1][j - 1][3])
                dele = (dp[i - 1][j][0] + 1, dp[i - 1][j][1], dp[i - 1][j][2] + 1, dp[i - 1][j][3])
                inse = (dp[i][j - 1][0] + 1, dp[i][j - 1][1], dp[i][j - 1][2], dp[i][j - 1][3] + 1)
                dp[i][j] = min(sub, dele, inse, key=lambda t: t[0])
    return dp[n][m][1], dp[n][m][2], dp[n][m][3]


def wer(ref_text: str, hyp_text: str):
    """Returns (wer_pct, accuracy_pct, n_words, subs, dels, ins)."""
    ref, hyp = normalize(ref_text), normalize(hyp_text)
    if not ref:
        return (0.0, 100.0, 0, 0, 0, 0) if not hyp else (100.0, 0.0, 0, 0, 0, len(hyp))
    s, d, i = levenshtein_ops(ref, hyp)
    w = 100.0 * (s + d + i) / len(ref)
    return (round(w, 2), round(100.0 - w, 2), len(ref), s, d, i)


def read(path):
    try:
        return open(path, encoding="utf-8").read()
    except OSError as e:
        sys.exit(f"cannot read {path}: {e}")


def show(label, ref, hyp):
    w, acc, n, s, d, i = wer(ref, hyp)
    print(f"  {label:<22} WER {w:6.2f}%   accuracy {acc:6.2f}%   ({n} ref words; {s} sub / {d} del / {i} ins)")
    return w, acc


def main():
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = p.add_subparsers(dest="cmd", required=True)

    w = sub.add_parser("wer", help="score one transcript")
    w.add_argument("--ref", help="reference text file (what was actually said)")
    w.add_argument("--ref-text", help="…or inline reference text")
    w.add_argument("--hyp", help="hypothesis text file")
    w.add_argument("--hyp-text", help="…or inline hypothesis text")

    c = sub.add_parser("compare", help="before/after table")
    c.add_argument("--ref", required=True)
    c.add_argument("--hyp-before", required=True, help="streaming draft transcript")
    c.add_argument("--hyp-after", required=True, help="ClearSpeech final-pass transcript")

    sub.add_parser("selftest", help="run the built-in checks")

    a = p.parse_args()
    if a.cmd == "wer":
        ref = read(a.ref) if a.ref else (a.ref_text or "")
        hyp = read(a.hyp) if a.hyp else (a.hyp_text or "")
        show("transcript", ref, hyp)
    elif a.cmd == "compare":
        ref = read(a.ref)
        print("ClearSpeech before/after (same audio, two engines):\n")
        wb, _ = show("BEFORE (draft)", ref, read(a.hyp_before))
        wa, _ = show("AFTER (final pass)", ref, read(a.hyp_after))
        print(f"\n  improvement: {round(wb - wa, 2)} WER points  ({round(wb - wa, 2)} accuracy points)")
    else:
        cases = [
            ("turn with me to john chapter three verse sixteen", "turn with me to john chapter 3 verse 16", 0.0),
            ("for god so loved the world", "for god so loved the word", 16.67),
            ("open the gospel of john chapter three", "open the gospel of jon chapter three", 14.29),
        ]
        ok = True
        for ref, hyp, want in cases:
            got, _, _, _, _, _ = wer(ref, hyp)
            if abs(got - want) > 0.51:
                print(f"  FAIL: wer({hyp!r}) = {got}, want {want}")
                ok = False
        print("  selftest:", "OK (3/3)" if ok else "FAILED")
        sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
