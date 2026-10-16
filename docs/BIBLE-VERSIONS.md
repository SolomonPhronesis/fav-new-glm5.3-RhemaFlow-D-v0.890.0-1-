# Bible versions: how KJV and the others work (v0.813.0)

## The short answer
* **KJV and BBE are built in**, always offline.
* **Public-domain versions** (ASV, YLT, Darby, Webster, Wycliffe, Douay-Rheims, Rotherham, Noyes, AKJV): in the app, **Translation menu -> Add Bible versions... -> Download**. One-time, about 8 MB each, then offline. (Tested on real ASV, YLT, Darby and BBE files.)
* **Licensed versions (NIV, ESV, NKJV, NLT, NASB, AMP...)** cannot be bundled or scraped; the licence forbids it. Get the text from a source you are licensed to use, then drop the file in the Bibles folder.

## The Bibles folder
macOS: `~/Library/Application Support/RhemaFlow/Bibles/`   (Linux: `~/.local/share/RhemaFlow/Bibles/`)

Put one file per version. The file name is the menu name (`NIV.json` shows as "NIV"). Press **Rescan** in the Bible versions window, or restart. Accepted layouts (checked on load; a bad file is refused with a reason, it can never break the app):

1. `{"books":[{"name":"Genesis","chapters":[["verse 1","verse 2"],...]}, ...]}` (the app's own)
2. `[{"name":"...","chapters":[["v1",...],...]}, ...]` (the thiagobodruk/bible layout; book names are ignored, only the order is used)
3. `{"books":[{"name":"Genesis","chapters":[{"chapter":1,"verses":[{"verse":1,"text":"..."}]}]}]}` (scrollmapper)
4. Text lines `Genesis<TAB>1<TAB>1<TAB>In the beginning...` (or `Genesis 1:1<TAB>text`)

Must be a complete 66-book Bible (about 31,100 verses). `{...}` notes and `<...>` tags are stripped.

## API (for licensed versions): tools/fetch_bible.py
```
export API_BIBLE_KEY=your-key            # free key: https://scripture.api.bible  (read its terms)
python3 tools/fetch_bible.py list        # versions your key can use
python3 tools/fetch_bible.py api <bibleId> NIV
python3 tools/fetch_bible.py pd ASV      # public domain, no key
```
It writes `NIV.json` into the Bibles folder. **Honest status:** the `pd` path was run and works; the `api` path follows API.Bible's v1 documentation but has NOT been run against the live service (no key here). Try one book first, read the output, and check the licence for how much text you may store and show. Other legitimate sources: your church's Bible-software licence export, or a publisher's own API (the ESV API from Crossway has its own free non-commercial key and rules).

## How matching and showing work with several versions
* The words a preacher says are matched against **KJV** (what is quoted from the pulpit, and the app's structural reference). That is unchanged.
* The verse is **shown** in the version you select. Change version mid-service and the verse on stage redraws in the new text.
* Verse numbering differs slightly in some versions; if a verse is missing in the chosen version the KJV text is used for that verse.

## Web version
Same idea in `web/rhemaflow-web.html`: Bible versions... -> Download (public domain) or choose a file. Stored in the browser (IndexedDB), never uploaded.
