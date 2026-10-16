#!/usr/bin/env python3
"""Redraw docs/comparison.png from docs/comparison_data.json.
   python3 tools/make_comparison.py [project_root]      (needs: pip install matplotlib)
Put YOUR numbers in the JSON (measured on your Mac) and rerun to update the picture."""
import json, os, sys
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt

root = sys.argv[1] if len(sys.argv) > 1 else "."
data = json.load(open(os.path.join(root, "docs/comparison_data.json")))
C_A, C_B, C_C = "#d9822b", "#2b6cb0", "#2f9e6b"   # R2T2, RhemaFlow today, RhemaFlow + R2T2
fig, ax = plt.subplots(2, 2, figsize=(17, 12.5))
fig.suptitle("R2T2 vs RhemaFlow - what is published, what we measured, what is a guess", fontsize=17, fontweight="bold")

# 1) published WER (mean of the nine datasets)
pub = data["published_wer_percent"]["systems"]
means = sorted(((k, sum(v) / len(v)) for k, v in pub.items()), key=lambda kv: kv[1])
a = ax[0][0]
names = [m[0] for m in means][::-1]
vals = [m[1] for m in means][::-1]
cols = [C_A if n.startswith("R2T2") else ("#9aa5b1" if "rewrites" in n or "pseudo" in n else "#5b6b7a") for n in names]
a.barh(names, vals, color=cols)
for i, v in enumerate(vals):
    a.text(v + 0.3, i, f"{v:.1f}%  ({100 - v:.1f}% right)", va="center", fontsize=9)
lo, hi = data["rhemaflow_tiny_en_wer_estimate_percent"]
a.axvspan(lo, hi, color=C_B, alpha=0.18)
a.set_xlim(0, 27)
a.set_xlabel("average word error % over 9 English test sets (lower is better)\nblue band = RhemaFlow today (tiny.en), ESTIMATE 9-14% - not measured on these sets")
a.set_title("1. Published accuracy (vendor's table, clean/meeting/TED audio)\nR2T2 = 93.9% right. Grey bars rewrite text as they go.", fontsize=11)

# 2) measured, ours
m = {k: v for k, v in data["rhemaflow_measured_accuracy_percent"].items() if not k.startswith("_")}
b = ax[0][1]
labels = list(m.keys())[::-1]
vv = [m[k] for k in labels]
cc = [("#c0392b" if "old front" in k else C_B) for k in labels]
b.barh(labels, vv, color=cc)
for i, v in enumerate(vv):
    b.text(v + 0.8, i, f"{v:.1f}%", va="center", fontsize=9)
b.set_xlim(0, 112)
b.set_xlabel("words correct %  (MEASURED, one 11-second clip, Whisper tiny.en)")
b.set_title("2. What we measured on our side (red = before the fixes)\nOne clip, one speaker: this does NOT prove 93%", fontsize=11)

# 3) scorecard
sc = data["scorecard"]
c = ax[1][0]
n = len(sc)
h = 0.26
for j, (col, name) in enumerate(zip((C_A, C_B, C_C), data["contenders"])):
    ys = [i + (j - 1) * h for i in range(n)]
    sc_vals = [s["scores"][j] for s in sc]
    bars = c.barh(ys, sc_vals, height=h, color=col, label=name)
    for bar, s in zip(bars, sc):
        if s["tags"][j] == "E":
            bar.set_hatch("///")
            bar.set_alpha(0.65)
c.set_yticks(range(n))
c.set_yticklabels([f'{s["category"]}  (x{s["weight"]})' for s in sc], fontsize=8.5)
c.invert_yaxis()
c.set_xlim(0, 5.4)
c.set_xlabel("score 1 (poor) to 5 (great)   |   hatched = estimate / not verified")
c.legend(loc="upper center", bbox_to_anchor=(0.5, -0.12), ncol=3, fontsize=8.5)
c.set_title("3. Scorecard by category (x = how much it matters for a church)", fontsize=11)

# 4) ranking
d = ax[1][1]
tot_w = sum(s["weight"] for s in sc)
def total(j, only=None):
    items = [s for s in sc if (only is None or s["category"] in only)]
    w = sum(s["weight"] for s in items)
    return sum(s["weight"] * s["scores"][j] for s in items) / w * 20  # 0-100
church = [total(j) for j in range(3)]
ears_only = ["Word accuracy, clean speech", "Words appear live, never rewritten", "Delay after you speak", "Background noise / music", "Accents", "Many languages"]
ears = [total(j, ears_only) for j in range(3)]
xs = range(3)
d.bar([x - 0.2 for x in xs], ears, 0.4, color=[C_A, C_B, C_C], alpha=0.55, label="ears only (transcription quality)")
d.bar([x + 0.2 for x in xs], church, 0.4, color=[C_A, C_B, C_C], label="for a church on an M1 (everything)")
for x, v in zip(xs, ears):
    d.text(x - 0.2, v + 1, f"{v:.0f}", ha="center", fontsize=10)
for x, v in zip(xs, church):
    d.text(x + 0.2, v + 1, f"{v:.0f}", ha="center", fontsize=10, fontweight="bold")
d.set_xticks(list(xs))
d.set_xticklabels([x.replace(" (", "\n(") for x in data["contenders"]], fontsize=9)
d.set_ylim(0, 110)
d.set_ylabel("weighted score out of 100")
d.set_title("4. Ranking. Light bar = pure listening quality, solid bar = whole job", fontsize=11)
d.legend(loc="upper left", fontsize=8.5)

plt.tight_layout(rect=[0, 0, 1, 0.965])
out = os.path.join(root, "docs/comparison.png")
plt.savefig(out, dpi=130)
print("wrote", out)
print("ears-only :", [round(x, 1) for x in ears])
print("whole job :", [round(x, 1) for x in church])
