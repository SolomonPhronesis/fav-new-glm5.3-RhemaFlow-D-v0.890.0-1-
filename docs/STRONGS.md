# Greek and Hebrew words (Strong's numbers), v0.816.0

Open it with the **Strong's** button (next to Media & slides), or say **"show strongs"**. Or just ask:
**"what is the Greek for love"**, **"hebrew for peace"**, **"strong's for grace"**. The panel opens with the answer.

## The three ways to do it (and what I built)

| | Way | What you get | Needs | Status |
|---|---|---|---|---|
| **1** | **Word by word.** Every word of the verse on the screen is a button. Press "Word" in John 1:1 and you get λόγος (lógos), G3056. | The *exact* original word for *that* word in *that* verse. | A KJV where every word is tagged with its Strong's number (an "OSIS" file). | **Built and tested on sample data. The tagged KJV file itself is not in the zip** (see below). |
| **2** | **English word to original word.** Type or say a word; get the Greek (and, if installed, Hebrew) entries that the King James translators used it for. "word" gives logos then rhema. | Candidates with meaning, number, how to say it. About 50 of the most-asked words (love, word, spirit, faith, grace, peace...) always list the famous one first. | Only the dictionary. Works offline. | **Built, tested, in the zip.** Greek built in. |
| **3** | **Online or your own module.** Ask a website's service, or read the church's own e-Sword / MySword / theWord / SWORD module. | Whatever that source has. | Internet, or the church's own files and their licence. | **Not built.** Way 1's file reader covers a SWORD module exported to OSIS (below). |

Without a tagged file the verse's word buttons still work, but they act like way 2 (they search the English word), so
"Word" would show logos *and* rhema and you pick. With a tagged file they jump straight to the right one.

## What is in the zip, and the licence

* **Greek dictionary, 5,624 entries, built in.** Strong's Greek Dictionary in XML, prepared by Ulrik Sandborg-Petersen from James Strong's 1890 concordance (public domain). Released by him under **CC0** (the "no rights reserved" waiver): github.com/morphgnt/strongs-dictionary-xml. Converted by `tools/make_strongs_greek.py` to `crates/rf-strongs/data/greek.json` (1 MB).
* The Greek JSON in the OpenScriptures repository is marked **CC-BY-SA** (share-alike), so I did **not** use that copy.
* **Not in the zip, because I could not confirm a clear licence from where I work:** the Hebrew dictionary and a tagged KJV.

## Hebrew (optional)

```
python3 tools/fetch_strongs.py hebrew          # downloads OpenScriptures' HebrewStrong.xml, converts it
python3 tools/fetch_strongs.py status
```
It installs `~/.rhemaflow/strongs/hebrew.json` (8,674 entries). **Please read the licence in github.com/openscriptures/HebrewLexicon yourself**: I tested the conversion on the real file but could not confirm its licence.

## A tagged KJV (optional, for way 1)

The CrossWire Bible Society's **KJV** module has Strong's numbers on every word. CrossWire says the KJV text may be used for any purpose (Debian lists it as public domain). I could not download it from where I built this, so I have **not tested this step on the real file**. As far as I know the steps are:

1. Install the free SWORD tools (on a Mac: `brew install sword`) and fetch the module with its `installmgr` (source: CrossWire, module: KJV).
2. Export it as OSIS text with the tool `mod2osis` (`mod2osis KJV > kjv-strongs.xml`).
3. `python3 tools/fetch_strongs.py tagged kjv-strongs.xml`

If any of those names differ on your Mac, the idea is the same: you need a KJV as an OSIS XML file where words look like `<w lemma="strong:G3056">Word</w>`. The reader accepts both common shapes of OSIS verse and zero-padded numbers like `H07225`. It was tested on a small sample I wrote in that shape, **not** on the real module.

## Voice

| Say | Does |
|---|---|
| "show strongs" / "hide strongs" | opens / closes the panel |
| "what is the Greek for love" / "Greek word for word" | opens it and selects the best match (G26 agape; G3056 logos) |
| "hebrew for peace" | same, Hebrew (needs the Hebrew file) |
| "what's the original word for grace" / "strong's for faith" | either language |

These work in short sentences only (up to 9 words). A preacher saying "the Greek word for love is agape" will open the panel on "love": harmless, and often handy. The panel never puts anything on the congregation screen by itself; **Show on stage** does that.

## Not done / honest limits

* Way 1 on the real CrossWire file is untested.
* Only the King James text can be word-tagged this way; other translations have no numbers.
* "KJV renders it as" lists are the 1890 lists. Rankings among the non-first-pick words are by how well the English word matches, not by how often the word is used.
* The look of the panel on a real Mac window is untested (it is tested headlessly: every part draws, nothing needs a click I could not simulate).
