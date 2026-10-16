/* RhemaFlow web - core logic (no DOM). Runs in the browser and in Node (tests).
 * A port of the desktop app's rules: reference parsing, quotation matching with
 * KJV "hath"/"have" tolerance, ranked candidates, full-text search, voice commands,
 * auto-approve, date-named files and the detailed transcript. */
(function (root, factory) {
  if (typeof module === 'object' && module.exports) module.exports = factory();
  else root.RF = factory();
})(typeof self !== 'undefined' ? self : this, function () {
  'use strict';

  // ---------------------------------------------------------------- Bible object
  /** data = {names:[66], books:[ [ [verse,...], ...chapters ], ...66 ]} */
  function makeBible(data) {
    const b = { names: data.names, books: data.books, _flat: null };
    b.chapterCount = (bk) => (b.books[bk] ? b.books[bk].length : 0);
    b.verseCount = (bk, ch) => (b.books[bk] && b.books[bk][ch - 1] ? b.books[bk][ch - 1].length : 0);
    b.verse = (bk, ch, v) => (b.books[bk] && b.books[bk][ch - 1] && b.books[bk][ch - 1][v - 1]) || '';
    b.exists = (bk, ch, v) => !!b.verse(bk, ch, v);
    b.passage = (bk, ch, from, to) => { const out = []; for (let v = from; v <= Math.max(from, to); v++) { const t = b.verse(bk, ch, v); if (t) out.push(t); } return out.join(' '); };
    b.total = () => b.books.reduce((n, bk) => n + bk.reduce((m, c) => m + c.length, 0), 0);
    b.flat = () => {
      if (!b._flat) {
        const arr = [];
        b.books.forEach((bk, bi) => bk.forEach((ch, ci) => ch.forEach((t, vi) => arr.push({ b: bi, c: ci + 1, v: vi + 1, s: ' ' + normalizeWords(t).join(' ') + ' ', n: 0 }))));
        arr.forEach((x) => { x.n = x.s.split(' ').length - 2; });
        b._flat = arr;
      }
      return b._flat;
    };
    return b;
  }

  // ---------------------------------------------------------------- text helpers
  const MODERN = { hath: 'have', hast: 'have', doth: 'does', dost: 'does', saith: 'says', shalt: 'shall', wilt: 'will', art: 'are', thee: 'you', thou: 'you', thy: 'your', thine: 'your', ye: 'you', wherefore: 'therefore' };
  function normalizeWords(text) {
    const out = [];
    for (const w of String(text).toLowerCase().split(/[^a-z0-9]+/)) if (w) out.push(MODERN[w] || w);
    return out;
  }
  function squash(s) { return String(s).toLowerCase().replace(/[^a-z0-9]/g, ''); }
  function lev(a, b) {
    const m = a.length, n = b.length; if (!m) return n; if (!n) return m;
    let prev = Array.from({ length: n + 1 }, (_, i) => i);
    for (let i = 1; i <= m; i++) { const cur = [i]; for (let j = 1; j <= n; j++) cur[j] = Math.min(prev[j] + 1, cur[j - 1] + 1, prev[j - 1] + (a[i - 1] === b[j - 1] ? 0 : 1)); prev = cur; }
    return prev[n];
  }
  function similarity(a, b) { const L = Math.max(a.length, b.length); return L ? 1 - lev(a, b) / L : 1; }

  const UNITS = { zero: 0, one: 1, two: 2, three: 3, four: 4, five: 5, six: 6, seven: 7, eight: 8, nine: 9, ten: 10, eleven: 11, twelve: 12, thirteen: 13, fourteen: 14, fifteen: 15, sixteen: 16, seventeen: 17, eighteen: 18, nineteen: 19, twenty: 20, thirty: 30, forty: 40, fifty: 50, sixty: 60, seventy: 70, eighty: 80, ninety: 90 };
  const TENS = new Set(['twenty', 'thirty', 'forty', 'fifty', 'sixty', 'seventy', 'eighty', 'ninety']);
  /** Read a number at tokens[i]: a digit token, or English words in proper grammar:
   *  "one hundred and nineteen", "twenty eight", "seventeen". "five seventeen" is TWO numbers.
   *  -> [value, tokensUsed] | null */
  function readNumber(tokens, i) {
    const t0 = tokens[i];
    if (t0 === undefined) return null;
    if (/^\d+$/.test(t0)) return [parseInt(t0, 10), 1];
    let k = i, val = 0, any = false;
    const isUnit = (t) => t in UNITS && UNITS[t] >= 1 && UNITS[t] <= 9;
    if (isUnit(t0) && tokens[i + 1] === 'hundred') { val = UNITS[t0] * 100; k = i + 2; any = true; }
    else if (t0 === 'hundred') { val = 100; k = i + 1; any = true; }
    if (any && tokens[k] === 'and' && tokens[k + 1] in UNITS) k++;
    const t = tokens[k];
    if (t !== undefined && t in UNITS) {
      if (TENS.has(t)) { val += UNITS[t]; k++; if (tokens[k] !== undefined && isUnit(tokens[k])) { val += UNITS[tokens[k]]; k++; } any = true; }
      else if (!(any && UNITS[t] === 0)) { val += UNITS[t]; k++; any = true; }
    }
    return any ? [val, k - i] : null;
  }

  function tokenize(text) {
    let t = String(text).toLowerCase().replace(/(\d)\s*:\s*(\d)/g, '$1 colon $2').replace(/:/g, ' colon ');
    // "Genesis 1-1" (how speech engines write 1:1) is chapter-verse; "3:16-18" / "verse 16-18" is a range
    t = t.replace(/(\d+)\s*[-\u2013\u2014]\s*(\d+)/g, (m, a, b, off) => (/colon|verse|verses/.test(t.slice(Math.max(0, off - 24), off)) ? `${a} to ${b}` : `${a} colon ${b}`));
    return t.replace(/['\u2019]/g, '').replace(/[^a-z0-9]+/g, ' ').trim().split(/\s+/).filter(Boolean);
  }

  // ---------------------------------------------------------------- books
  const ALIASES = {
    'Psalms': ['psalm', 'psalms', 'salm'], 'Song of Solomon': ['song of solomon', 'song of songs', 'songs of solomon', 'songs', 'canticles'],
    'Revelation': ['revelation', 'revelations', 'revelation of john'], 'Ecclesiastes': ['ecclesiastes', 'eclesiastes'],
    'Philippians': ['philippians', 'phillipians', 'filipians'], 'Ephesians': ['ephesians', 'efesians'], 'Philemon': ['philemon', 'phylemon'],
    'Habakkuk': ['habakkuk', 'habakuk'], 'Deuteronomy': ['deuteronomy', 'duteronomy'], 'Colossians': ['colossians', 'colosians'],
    'Thessalonians': [], 'Obadiah': ['obadiah'], 'Zephaniah': ['zephaniah'], 'Zechariah': ['zechariah'], 'Nahum': ['nahum'], 'Hebrews': ['hebrews'], 'James': ['james'],
  };
  const ORD = { 1: ['1', 'first', '1st', 'i', 'one'], 2: ['2', 'second', '2nd', 'ii', 'two'], 3: ['3', 'third', '3rd', 'iii', 'three'] };
  function buildBookIndex(names) {
    const entries = []; // {book, num, key (squashed, no number), words}
    names.forEach((nm, bi) => {
      const m = nm.match(/^([123])\s+(.*)$/);
      const num = m ? parseInt(m[1], 10) : 0, base = m ? m[2] : nm;
      const forms = new Set([base.toLowerCase(), ...(ALIASES[base] || [])]);
      forms.forEach((f) => entries.push({ book: bi, num, key: squash(f), words: f.split(' ') }));
    });
    return entries;
  }

  /** Match a book at tokens[i]. -> {book, len, score, ambiguous?} | null  (score 1 = exact) */
  function matchBook(entries, tokens, i, allowPrefix) {
    if (tokens[i] === undefined) return null;
    const single = (e, s) => {
      const t = tokens[s]; if (t === undefined) return 0;
      if (t === e.key) return 1;
      if (allowPrefix && t.length >= 3 && e.key.startsWith(t)) return 0.9; // typed abbreviation: rev, 1 sam, matt
      if (t.length >= 5) { const sc = similarity(t, e.key); return sc >= (t.length >= 8 ? 0.7 : 0.8) ? sc : 0; }
      return 0;
    };
    const at = (s, wantNum) => {
      let best = null;
      for (const e of entries) {
        if (e.num !== wantNum) continue;
        let sc = 0, len = 0;
        if (e.words.length > 1) { if (tokens.slice(s, s + e.words.length).join(' ') === e.words.join(' ')) { sc = 1; len = e.words.length; } }
        else { sc = single(e, s); len = 1; }
        if (sc && (!best || sc > best.score)) best = { book: e.book, len: s - i + len, score: sc };
      }
      return best;
    };
    for (const n of [1, 2, 3]) if (ORD[n].includes(tokens[i]) && tokens[i + 1] !== undefined) { const m = at(i + 1, n); if (m) return m; }
    const m = at(i, 0); if (m) return m;
    // "kings", "samuel", "thessalonians": a numbered book spoken without its number
    let amb = null;
    for (const e of entries) if (e.num && e.words.length === 1) { const sc = single(e, i); if (sc) { const s2 = sc === 1 ? 0.76 : 0.6; if (!amb || s2 > amb.score) amb = { book: e.book, len: 1, score: s2, ambiguous: true }; } }
    return amb;
  }

  // ---------------------------------------------------------------- reference parser
  const FILLER = new Set(['chapter', 'chapters', 'ch', 'verse', 'verses', 'vs', 'colon', 'the', 'of', 'in', 'at']);
  /** Understand spoken scripture references. ctx = {last:{book,chapter}}.
   *  -> {detections:[{kind,book,chapter,from,to,conf,note}], confirms:[...], errors:[msg]} */
  function parseReferences(bible, entries, text, ctx, allowPrefix) {
    const tokens = tokenize(text);
    const out = { detections: [], confirms: [], errors: [] };
    let i = 0;
    while (i < tokens.length) {
      const bm = matchBook(entries, tokens, i, allowPrefix);
      if (!bm) { i++; continue; }
      let j = i + bm.len;
      while (j < tokens.length && (tokens[j] === 'chapter' || tokens[j] === 'chapters' || tokens[j] === 'ch')) j++;
      const chap = readNumber(tokens, j);
      const bookName = bible.names[bm.book];
      if (!chap) {
        const conf = bm.score >= 0.86 ? bm.score * 0.8 : bm.score * 0.7;
        if (tokens.length > 6 || (bm.score < 0.86 && tokens.length > 5)) { i += bm.len; continue; } // a word in a sentence that merely looks like a book
        out.confirms.push({ kind: 'book', book: bm.book, chapter: 0, from: 0, to: 0, conf, note: bm.score >= 0.86 ? 'book only \u2014 no chapter heard' : 'uncertain book \u2014 no chapter heard' });
        i += bm.len; continue;
      }
      const chapter = chap[0];
      j += chap[1];
      const cc = bible.chapterCount(bm.book);
      if (chapter < 1 || chapter > cc) { out.errors.push(`${bookName} has only ${cc} chapter${cc === 1 ? '' : 's'}`); i = j; continue; }
      let verseKw = false;
      while (j < tokens.length && (tokens[j] === 'verse' || tokens[j] === 'verses' || tokens[j] === 'vs' || tokens[j] === 'colon')) { verseKw = true; j++; }
      let from = 0, to = 0;
      const v1 = readNumber(tokens, j);
      if (v1 && (verseKw || true)) {
        from = v1[0]; j += v1[1];
        if (tokens[j] === 'to' || tokens[j] === 'through' || tokens[j] === 'thru' || tokens[j] === 'until') {
          const v2 = readNumber(tokens, j + 1);
          if (v2) { to = v2[0]; j += 1 + v2[1]; }
        }
        if (!to) to = from;
      }
      const vc = bible.verseCount(bm.book, chapter);
      const conf0 = bm.score >= 0.999 ? 0.97 : 0.7 + 0.25 * bm.score;
      if (!from) {
        out.confirms.push({ kind: 'chapter', book: bm.book, chapter, from: 1, to: 1, conf: Math.min(conf0, 0.72), note: 'chapter only \u2014 verse 1 shown if approved' });
      } else if (from > vc) {
        out.errors.push(`${bookName} ${chapter} has only ${vc} verse${vc === 1 ? '' : 's'}`);
      } else {
        if (to > vc) to = vc;
        if (to < from) to = from;
        const det = { kind: 'explicit', book: bm.book, chapter, from, to, conf: bm.ambiguous ? Math.min(conf0, 0.7) : conf0, note: '' };
        out.detections.push(det);
      }
      i = j;
    }
    // no book spoken: "chapter four", "chapter 4 verse 2", "verse 6", "verse six to nine" use the current book / chapter
    if (!out.detections.length && !out.confirms.length && !out.errors.length && ctx && ctx.last && tokens.length <= 9) {
      const { book } = ctx.last; let chapter = ctx.last.chapter;
      let k = tokens.findIndex((t) => t === 'chapter' || t === 'chapters');
      let gotChapter = false;
      if (k >= 0) { const c = readNumber(tokens, k + 1); if (c) { chapter = c[0]; gotChapter = true; k = k + 1 + c[1]; } }
      const vk = tokens.findIndex((t, i) => i >= (gotChapter ? k : 0) && (t === 'verse' || t === 'verses'));
      if (gotChapter || vk >= 0) {
        const cc = bible.chapterCount(book);
        if (chapter < 1 || chapter > cc) out.errors.push(`${bible.names[book]} has only ${cc} chapters`);
        else {
          const vc = bible.verseCount(book, chapter);
          let from = 0, to = 0;
          if (vk >= 0) { const v = readNumber(tokens, vk + 1); if (v) { from = v[0]; let j = vk + 1 + v[1]; if (tokens[j] === 'to' || tokens[j] === 'through') { const v2 = readNumber(tokens, j + 1); if (v2) to = v2[0]; } to = to || from; } }
          if (!from && gotChapter) out.confirms.push({ kind: 'chapter', book, chapter, from: 1, to: 1, conf: 0.72, note: 'chapter only \u2014 verse 1 shown if approved' });
          else if (from > vc) out.errors.push(`${bible.names[book]} ${chapter} has only ${vc} verses`);
          else if (from) out.detections.push({ kind: 'verse', book, chapter, from, to: Math.max(from, Math.min(to, vc)), conf: gotChapter ? 0.9 : 0.9, note: gotChapter ? 'chapter in the current book' : 'verse in the current chapter' });
        }
      }
    }
    return out;
  }

  // ---------------------------------------------------------------- quotation matching
  function lcsLen(a, b) {
    let prev = new Array(b.length + 1).fill(0);
    for (let i = 0; i < a.length; i++) { const cur = new Array(b.length + 1).fill(0); for (let j = 0; j < b.length; j++) cur[j + 1] = a[i] === b[j] ? prev[j] + 1 : Math.max(cur[j], prev[j + 1]); prev = cur; }
    return prev[b.length];
  }
  function longestRun(a, b) {
    let best = 0, prev = new Array(b.length + 1).fill(0);
    for (let i = 0; i < a.length; i++) { const cur = new Array(b.length + 1).fill(0); for (let j = 0; j < b.length; j++) if (a[i] === b[j]) { cur[j + 1] = prev[j] + 1; if (cur[j + 1] > best) best = cur[j + 1]; } prev = cur; }
    return best;
  }
  function scoreFragment(heard, vw, off) {
    const take = Math.min(off + heard.length + 1, vw.length);
    const lead = vw.slice(off, take);
    const lcs = lcsLen(heard, lead), run = longestRun(heard, lead);
    const p = lcs / heard.length, r = lcs / Math.max(1, lead.length);
    let f1 = p + r > 0 ? (2 * p * r) / (p + r) : 0;
    f1 *= lcs === 0 ? 0 : 0.7 + 0.3 * run / lcs;
    if (off > 0) f1 *= 0.92;
    return Math.min(99, Math.round(f1 * 100));
  }
  /** Ranked verses a spoken fragment could be (>= 4 words). -> [{book,chapter,verse,pct}] best first */
  function rankQuotation(bible, text, maxResults) {
    maxResults = maxResults || 8;
    const words = normalizeWords(text);
    if (words.length < 4) return [];
    const flat = bible.flat();
    const maxStart = Math.min(6, words.length - 4);
    let cands = null, bestStart = 0;
    outer: for (let s = 0; s <= maxStart; s++) {
      const rem = words.length - s; if (rem < 4) break;
      for (let n = Math.min(rem, 8); n >= 4; n--) {
        const phrase = ' ' + words.slice(s, s + n).join(' ') + ' ';
        const hits = [];
        for (const x of flat) { const pos = x.s.indexOf(phrase); if (pos >= 0) { hits.push({ x, off: x.s.slice(0, pos).split(' ').length - 1 }); if (hits.length >= 400) break; } }
        if (hits.length) { cands = hits; bestStart = s; break outer; }
      }
    }
    if (!cands) return [];
    const heard = words.slice(bestStart, bestStart + 24);
    const res = cands.map(({ x, off }) => {
      const vw = x.s.trim().split(' ');
      return { book: x.b, chapter: x.c, verse: x.v, pct: scoreFragment(heard, vw, Math.max(0, off)) };
    });
    res.sort((a, b) => b.pct - a.pct || a.book - b.book || a.chapter - b.chapter || a.verse - b.verse);
    return res.slice(0, maxResults);
  }

  // ---------------------------------------------------------------- full-text search
  const POPULAR = [['Genesis', 1, 1], ['Genesis', 1, 27], ['Exodus', 20, 3], ['Joshua', 1, 9], ['Psalms', 23, 1], ['Psalms', 46, 1], ['Psalms', 119, 105], ['Proverbs', 3, 5], ['Proverbs', 3, 6], ['Isaiah', 40, 31], ['Isaiah', 41, 10], ['Isaiah', 53, 5], ['Jeremiah', 29, 11], ['Matthew', 6, 33], ['Matthew', 11, 28], ['Matthew', 28, 19], ['Matthew', 28, 20], ['John', 1, 1], ['John', 3, 16], ['John', 14, 6], ['John', 10, 10], ['Romans', 3, 23], ['Romans', 5, 8], ['Romans', 6, 23], ['Romans', 8, 28], ['Romans', 10, 9], ['Romans', 12, 2], ['1 Corinthians', 13, 4], ['2 Corinthians', 5, 17], ['Galatians', 5, 22], ['Ephesians', 2, 8], ['Ephesians', 2, 9], ['Philippians', 4, 13], ['Philippians', 4, 6], ['Hebrews', 11, 1], ['Hebrews', 11, 6], ['1 John', 1, 9], ['Revelation', 1, 6], ['Revelation', 3, 20]];
  /** "god so loved" -> John 3:16 first. -> [{book,chapter,verse,pct}] */
  function searchText(bible, query, limit) {
    limit = limit || 40;
    const q = normalizeWords(query);
    if (!q.length) return [];
    const popular = new Set(POPULAR.map(([n, c, v]) => `${bible.names.indexOf(n)}:${c}:${v}`));
    const nws = q.map((w) => ` ${w} `), npre = q.map((w) => ` ${w}`);
    const phrase = ` ${q.join(' ')} `, phrasePre = ` ${q.join(' ')}`;
    const T = q.length, hits = [];
    for (const x of bible.flat()) {
      let found = 0, lastPos = -1, inOrder = true, firstPos = 1e9;
      for (let i = 0; i < T; i++) {
        let p = x.s.indexOf(nws[i]);
        if (p < 0 && i === T - 1) p = x.s.indexOf(npre[i]);
        if (p >= 0) { found++; firstPos = Math.min(firstPos, p); if (lastPos >= 0 && p < lastPos) inOrder = false; lastPos = p; }
      }
      if (!found || (T >= 3 && found * 100 < T * 60) || (T <= 2 && found < T)) continue;
      const hasPhrase = x.s.includes(phrase) || x.s.includes(phrasePre);
      let score = hasPhrase ? 100 : found === T && inOrder ? 88 : found === T ? 80 : 55 + 25 * (found / T);
      score -= Math.min(9, Math.max(1, x.n) / 12);
      if (firstPos === 0) score += 2;
      if (popular.has(`${x.b}:${x.c}:${x.v}`)) score += 6;
      const pct = Math.round(Math.max(0, Math.min(99, score)));
      if (pct >= 40) hits.push({ book: x.b, chapter: x.c, verse: x.v, pct });
    }
    hits.sort((a, b) => b.pct - a.pct || a.book - b.book || a.chapter - b.chapter || a.verse - b.verse);
    return hits.slice(0, limit);
  }

  // ---------------------------------------------------------------- the one search box
  function bookQueryForms(q) {
    const t = q.trim().toLowerCase(), forms = [squash(t)];
    for (const [w, d] of [['first ', '1'], ['second ', '2'], ['third ', '3'], ['1st ', '1'], ['2nd ', '2'], ['3rd ', '3'], ['iii ', '3'], ['ii ', '2'], ['i ', '1']]) if (t.startsWith(w)) forms.push(squash(d + t.slice(w.length)));
    return forms.filter(Boolean);
  }
  /** -> [{type:'book',book}|{type:'chapter',book,chapter}|{type:'verse',book,chapter,from,to,pct,text}] */
  function search(bible, entries, query, maxVerses) {
    const q = String(query || '').trim(); if (!q) return [];
    const out = [], hasDigit = /\d/.test(q), forms = bookQueryForms(q);
    if (hasDigit) {
      const r = parseReferences(bible, entries, q.replace(/\./g, ' '), null, true);
      const d = r.detections[0] || r.confirms.find((c) => c.chapter > 0);
      if (d && d.kind === 'chapter') out.push({ type: 'chapter', book: d.book, chapter: d.chapter });
      else if (d) out.push({ type: 'verse', book: d.book, chapter: d.chapter, from: d.from, to: d.to, pct: 100, text: bible.passage(d.book, d.chapter, d.from, d.to) });
    }
    const bookLike = forms.some((f) => bible.names.some((n) => squash(n).startsWith(f)));
    if (bookLike) bible.names.forEach((n, b) => { if (forms.some((f) => squash(n).startsWith(f))) out.push({ type: 'book', book: b }); });
    if (!out.length && !hasDigit) {
      const r = parseReferences(bible, entries, q, null, true);
      [...r.confirms, ...r.detections].forEach((d) => { if (d.kind === 'book' && !out.some((o) => o.type === 'book' && o.book === d.book)) out.push({ type: 'book', book: d.book }); });
    }
    const letters = (q.match(/[a-z]/gi) || []).length;
    if (letters >= 3 && !(hasDigit && out.some((o) => o.type === 'verse'))) {
      for (const h of searchText(bible, q, maxVerses || 40)) {
        if (out.some((o) => o.type === 'verse' && o.book === h.book && o.chapter === h.chapter && o.from === h.verse)) continue;
        out.push({ type: 'verse', book: h.book, chapter: h.chapter, from: h.verse, to: h.verse, pct: h.pct, text: bible.verse(h.book, h.chapter, h.verse) });
      }
    }
    return out;
  }

  // ---------------------------------------------------------------- the whole utterance
  /** Decide what one finished utterance means.
   *  -> {stage:[detections to stage now], cards:[review cards], errors:[], commands:[names], quote: ...} */
  function interpret(bible, entries, text, ctx, opts) {
    opts = Object.assign({ floor: 0.75, autoApprove: false }, opts || {});
    const res = { stage: [], cards: [], errors: [], commands: [] };
    const refs = parseReferences(bible, entries, text, ctx);
    const hasRef = refs.detections.length || refs.confirms.some((c) => c.chapter > 0) || refs.errors.length;
    res.commands = matchCommands(text, !!hasRef);
    for (const d of refs.detections) (d.conf >= opts.floor ? res.stage : res.cards).push(d);
    res.cards.push(...refs.confirms.filter((c) => c.kind !== 'book' || !refs.detections.length));
    res.errors.push(...refs.errors);
    const weakOnly = res.cards.length > 0 && res.cards.every((c) => c.kind === 'book' && c.conf < 0.6);
    if (!res.stage.length && (!res.cards.length || weakOnly) && !res.errors.length && !res.commands.length) {
      const ranked = rankQuotation(bible, text, 8).filter((h) => h.pct >= 50);
      if (ranked.length) {
        const top = ranked[0], rest = ranked.slice(1);
        const card = { kind: 'quote', book: top.book, chapter: top.chapter, from: top.verse, to: top.verse, pct: top.pct, conf: rest.length ? Math.min(0.74, top.pct / 100) : 0.62, alts: rest.map((h) => ({ book: h.book, chapter: h.chapter, from: h.verse, to: h.verse, pct: h.pct })), note: rest.length ? `quotation match \u2014 ${ranked.length} possible verses, best first` : 'quotation match \u2014 never auto-staged' };
        if (weakOnly) res.cards.length = 0;
        if (shouldAutoApprove(card, opts.autoApprove)) { card.auto = true; res.stage.push(card); } else res.cards.push(card);
      }
    }
    return res;
  }
  /** Same rule as the desktop app. */
  function shouldAutoApprove(card, enabled) {
    if (!enabled || card.kind !== 'quote' || !card.chapter) return false;
    if (!card.alts || !card.alts.length) return card.pct >= 70;
    const second = card.alts[0].pct;
    return card.pct >= 75 && card.pct - second >= 12;
  }

  // ---------------------------------------------------------------- voice commands
  const SHORT_MAX = 9, MAX_GAP = 3;
  const CMD = [
    // [command, short-only?, phrases]
    ['stop_reading', false, ['stop reading', 'stop the reading', 'cancel reading', 'stop read aloud']],
    ['stop_reading', true, ['stop talking', 'stop speaking', 'be quiet', 'quiet please', 'stop it', 'thats enough']],
    ['read', false, ['read it', 'read that', 'read this', 'read the verse', 'read aloud']],
    ['read', true, ['read it aloud', 'read out loud', 'read to us', 'please read', 'start reading', 'read the scripture']],
    ['exit_fullscreen', false, ['exit full screen', 'exit fullscreen', 'close full screen', 'leave full screen', 'normal screen']],
    ['fullscreen', false, ['full screen', 'fullscreen']],
    ['open_stage', true, ['open stage', 'open the stage', 'show stage', 'show the stage', 'open projector', 'open the screen']],
    ['close_stage', true, ['close stage', 'close the stage', 'hide stage', 'hide the stage', 'close projector']],
    ['clear', false, ['clear the screen', 'clear screen', 'clear the stage', 'clear stage', 'close the screen', 'take it off']],
    ['clear', true, ['blank screen', 'go blank', 'clear it', 'clear this', 'clear that', 'remove it', 'hide the verse', 'take it down']],
    ['blackout', true, ['go black', 'black screen', 'blackout', 'black out']],
    ['zoom_400', true, ['max zoom', 'maximum zoom', 'zoom max', 'biggest text']],
    ['zoom_200', true, ['half max zoom', 'half max', 'half zoom', 'zoom half', 'medium zoom']],
    ['zoom_100', false, ['reset zoom', 'normal size', 'reset text']],
    ['zoom_in', false, ['zoom in', 'text bigger', 'bigger text', 'make it bigger']],
    ['zoom_in', true, ['bigger font', 'larger text', 'increase size', 'enlarge text']],
    ['zoom_out', false, ['zoom out', 'text smaller', 'smaller text', 'make it smaller']],
    ['zoom_out', true, ['smaller font', 'reduce size', 'shrink text']],
    ['last', false, ['last scripture', 'that again', 'it again', 'show again', 'same verse', 'back to that']],
    ['next_verse', false, ['next verse', 'next one']],
    ['next_verse', true, ['next line', 'go forward', 'following verse', 'next please', 'verse forward', 'forward one verse', 'one verse forward', 'go next', 'and next']],
    ['next_chapter', false, ['next chapter']],
    ['next_chapter', true, ['following chapter', 'chapter forward', 'forward one chapter', 'chapter after']],
    ['prev_chapter', false, ['previous chapter', 'last chapter', 'back chapter']],
    ['prev_chapter', true, ['prior chapter', 'go back a chapter', 'chapter before', 'one chapter back', 'preceding chapter']],
    ['prev_verse', false, ['previous verse', 'last verse', 'prior verse', 'go back']],
    ['prev_verse', true, ['back one verse', 'one verse back', 'previous one', 'previous please', 'go previous', 'preceding verse', 'verse before']],
    ['prev_scripture', false, ['previous scripture', 'previous passage', 'scripture before that', 'the one before', 'one before that', 'the scripture before', 'earlier scripture', 'go to the previous scripture', 'back to the previous scripture', 'before that one']],
    ['prev_scripture', true, ['go back to the last scripture', 'the last one', 'the one before that', 'back one scripture', 'the previous scripture']],
    ['next_scripture', false, ['next scripture', 'scripture after that', 'the one after', 'forward scripture']],
    ['next_scripture', true, ['the next scripture', 'go forward a scripture', 'scripture forward']],
    ['media_play', true, ['play video', 'play the video', 'start video', 'play it']],
    ['media_pause', true, ['pause video', 'pause the video', 'pause it']],
    ['media_restart', true, ['restart video', 'play again', 'from the start']],
    ['first_verse', true, ['first verse', 'top of chapter', 'start of chapter']],
    ['approve', true, ['approve', 'approve it', 'approve that', 'accept it', 'show it', 'put it up', 'put it on screen', 'thats the one', 'go with that']],
    ['dismiss', true, ['dismiss', 'dismiss it', 'dismiss that', 'reject it', 'not that one', 'wrong verse', 'wrong one', 'never mind', 'skip it']],
    ['pick_1', true, ['option one', 'option 1', 'first option', 'first match']],
    ['pick_2', true, ['option two', 'option 2', 'second option', 'second match']],
    ['pick_3', true, ['option three', 'option 3', 'third option', 'third match']],
    ['plan_next', true, ['next in the plan', 'next in plan', 'next planned', 'plan next']],
    ['auto_on', true, ['auto approve on', 'turn on auto approve']],
    ['auto_off', true, ['auto approve off', 'turn off auto approve', 'wait for approval']],
  ];
  // A single word that is the WHOLE utterance. "Next." / "Previous." are how people actually
  // drive navigation (the speech engine returns exactly that). They only do anything while a
  // verse is on the stage, so a stray "next" in a sermon is harmless.
  const BARE = { exit: 'exit_fullscreen', escape: 'exit_fullscreen', quiet: 'stop_reading', silence: 'stop_reading', hush: 'stop_reading', enough: 'stop_reading', approve: 'approve', accept: 'approve', dismiss: 'dismiss', reject: 'dismiss', blank: 'clear', clear: 'clear', blackout: 'blackout', fullscreen: 'fullscreen', next: 'next_verse', forward: 'next_verse', previous: 'prev_verse', prior: 'prev_verse', back: 'prev_verse', again: 'last', read: 'read', reading: 'read', preview: 'prev_verse', previews: 'prev_verse', nex: 'next_verse', nexts: 'next_verse' };
  // words that may surround a command without changing it ("ok next", "next please", "and next")
  const CMD_FILLER = new Set(['please', 'now', 'ok', 'okay', 'and', 'then', 'just', 'yes', 'so', 'go', 'on', 'thanks', 'thank', 'you', 'uh', 'um', 'the']);
  const ORDER = (() => { const v = []; CMD.forEach((e, ei) => e[2].forEach((p, pi) => v.push([ei, pi, p.split(' ').length]))); return v.sort((a, b) => b[2] - a[2] || a[0] - b[0] || a[1] - b[1]); })();
  function findPhrase(tokens, used, phrase) {
    outer: for (let i = 0; i < tokens.length; i++) {
      if (used[i] || tokens[i] !== phrase[0]) continue;
      const hit = [i]; let prev = i;
      for (let k = 1; k < phrase.length; k++) {
        let found = -1;
        for (let j = prev + 1; j <= Math.min(prev + MAX_GAP, tokens.length - 1); j++) if (!used[j] && tokens[j] === phrase[k]) { found = j; break; }
        if (found < 0) continue outer;
        hit.push(found); prev = found;
      }
      return hit;
    }
    return null;
  }
  /** "zoom 300", "zoom to three hundred percent" -> 300 (60..400) */
  function zoomPercent(tokens) {
    const z = tokens.indexOf('zoom'); if (z < 0) return null;
    let j = z + 1; while (j < tokens.length && ['to', 'at', 'level', 'by', 'of', 'set', 'on', 'size'].includes(tokens[j])) j++;
    const clean = []; for (let k = j; k < Math.min(tokens.length, j + 6); k++) if (tokens[k] !== 'percent') clean.push(tokens[k]);
    const n = readNumber(clean, 0); if (!n) return null;
    let v = n[0]; if (v >= 1 && v <= 4) v *= 100; if (v < 50 || v > 500) return null;
    return Math.max(60, Math.min(400, v));
  }
  const NAV_MAX = 6;
  const NAV = new Set(['next_verse', 'prev_verse', 'next_chapter', 'prev_chapter', 'last', 'first_verse', 'prev_scripture', 'next_scripture']);
  const cmdTokens = (text) => String(text).toLowerCase().replace(/-/g, ' ').replace(/['\u2019]/g, '').replace(/[^a-z0-9 ]+/g, ' ').trim().split(/\s+/).filter(Boolean);
  /** Run-together words ("nextverse") and small mishearings ("netverse", "nex verse") of a WHOLE short
   *  utterance. Compares the utterance, spaces removed, to every phrase, spaces removed. */
  function fuzzyWhole(tokens) {
    if (!tokens.length || tokens.length > 4) return null;
    const u = tokens.join(''); if (u.length < 6) return null;
    let best = null;
    CMD.forEach((e) => e[2].forEach((p) => {
      const q = p.replace(/ /g, ''); if (q.length < 6) return;
      const sc = u === q ? 1 : similarity(u, q);
      const thr = q.length >= 10 ? 0.84 : 0.88;
      if (sc >= thr && (!best || sc > best.score)) best = { cmd: e[0], score: sc };
    }));
    return best;
  }
  /** Detailed result: {cmds, pure}. pure = the whole utterance was command words (safe to act on early). */
  function commandsDetailed(text, hasRef) {
    const tokens = cmdTokens(text);
    if (!tokens.length) return { cmds: [], pure: false };
    const core = tokens.filter((t) => !CMD_FILLER.has(t));
    if (core.length === 1) {
      if (BARE[core[0]]) return { cmds: [BARE[core[0]]], pure: true };
      if (core[0].length >= 7) { // "forwards", "approved": one slipped letter; short words must be exact (black != blank)
        let bw = null; for (const w of Object.keys(BARE)) if (w.length >= 7) { const sc = similarity(core[0], w); if (sc >= 0.8 && (!bw || sc > bw.score)) bw = { w, score: sc }; }
        if (bw) return { cmds: [BARE[bw.w]], pure: true };
      }
    }
    const shortOk = tokens.length <= SHORT_MAX, out = [], used = new Array(tokens.length).fill(false);
    if (shortOk) { const z = zoomPercent(tokens); if (z) { out.push('zoom_' + z); tokens.forEach((t, i) => { if (t === 'zoom' || /^\d+$/.test(t) || t in UNITS || t === 'hundred' || t === 'percent' || t === 'to') used[i] = true; }); } }
    for (const [ei, pi] of ORDER) {
      const e = CMD[ei]; if (e[1] && !shortOk) continue;
      const hit = findPhrase(tokens, used, e[2][pi].split(' '));
      if (hit) { hit.forEach((i) => { used[i] = true; }); if (!out.includes(e[0])) out.push(e[0]); }
    }
    let cmds = out;
    if (!cmds.length) { const f = fuzzyWhole(core); if (f) { cmds = [f.cmd]; tokens.forEach((t, i) => { used[i] = true; }); } }
    // navigation is only ever a short utterance ("next verse"), never inside a sentence about a verse
    if (hasRef || tokens.length > NAV_MAX) cmds = cmds.filter((c) => !NAV.has(c));
    const pure = cmds.length > 0 && tokens.every((t, i) => used[i] || CMD_FILLER.has(t));
    return { cmds, pure };
  }
  /** Names of commands in an utterance. hasRef: a scripture reference was also heard (it wins over navigation). */
  function matchCommands(text, hasRef) { return commandsDetailed(text, hasRef).cmds; }
  /** Commands to act on from an UNFINISHED (interim) result: only when every word is a command word. */
  function pureCommands(text) { const r = commandsDetailed(text, false); return r.pure ? r.cmds : []; }
  function isBareStop(text) { const t = tokenize(text).filter((w) => !['please', 'now', 'it'].includes(w)); return t.length === 1 && ['stop', 'cancel', 'pause'].includes(t[0]); }

  const CMD_LABEL = { stop_reading: 'stop reading', read: 'read aloud', exit_fullscreen: 'exit full screen', fullscreen: 'full screen', open_stage: 'open stage window', close_stage: 'close stage window', clear: 'clear the screen', blackout: 'blackout', zoom_400: 'zoom 400% (max)', zoom_200: 'zoom 200% (half max)', zoom_100: 'zoom 100%', zoom_in: 'zoom in', zoom_out: 'zoom out', last: 'show the last scripture again', next_verse: 'next verse', prev_verse: 'previous verse', next_chapter: 'next chapter', prev_chapter: 'previous chapter', first_verse: 'first verse of the chapter', approve: 'approve the first review card', prev_scripture: 'previous scripture (history back)', next_scripture: 'next scripture (history forward, else next verse)', media_play: 'play the video', media_pause: 'pause the video', media_restart: 'restart the video', dismiss: 'dismiss the first review card', pick_1: 'pick option 1', pick_2: 'pick option 2', pick_3: 'pick option 3', plan_next: 'next scripture in the sermon plan', auto_on: 'auto-approve ON', auto_off: 'auto-approve OFF', help: 'help' };
  const cmdLabel = (n) => (/^zoom_\d+$/.test(n) && !CMD_LABEL[n] ? 'zoom ' + n.slice(5) + '%' : CMD_LABEL[n] || n);
  /** Everything you can say, grouped by what it does, for the in-app list. */
  function commandList() {
    const g = new Map();
    CMD.forEach((e) => { if (!g.has(e[0])) g.set(e[0], []); e[2].forEach((p) => g.get(e[0]).push({ p, short: e[1] })); });
    Object.entries(BARE).forEach(([w, c]) => { if (!g.has(c)) g.set(c, []); g.get(c).push({ p: w + ' (on its own)', short: false }); });
    ['zoom 300', 'zoom to 250', 'zoom three hundred percent', 'zoom 150'].forEach((p) => { if (!g.has('zoom_300')) g.set('zoom_300', []); g.get('zoom_300').push({ p, short: true }); });
    return [...g.entries()].map(([name, phrases]) => ({ name, label: cmdLabel(name), phrases }));
  }

  // ---------------------------------------------------------------- Bible importer (same layouts as the desktop app)
  function cleanVerse(v) { return String(v).replace(/\{[^}]*\}/g, '').replace(/<[^>]*>/g, '').replace(/\s+/g, ' ').trim(); }
  function importBible(text, canonNames) {
    const t = String(text).replace(/^\ufeff/, '').trim();
    let books;
    if (t.startsWith('{') || t.startsWith('[')) {
      const v = JSON.parse(t);
      const arr = Array.isArray(v) ? v : v.books;
      if (!Array.isArray(arr)) throw new Error('JSON has no "books" list');
      books = arr.map((b) => b.chapters.map((ch) => Array.isArray(ch) ? ch.map(cleanVerse)
        : ch.verses.slice().sort((x, y) => x.verse - y.verse).map((x) => cleanVerse(x.text))));
    } else {
      books = canonNames.map(() => []);
      t.split(/\r?\n/).forEach((line, n) => {
        line = line.trim(); if (!line || line[0] === '#') return;
        let cols = line.split('\t'), bk, ch, vs, tx;
        if (cols.length === 4) [bk, ch, vs, tx] = cols; else if (cols.length === 2) { const m = cols[0].match(/^(.*)\s+(\d+):(\d+)$/); if (!m) throw new Error(`line ${n + 1}: cannot read reference`); [bk, ch, vs, tx] = [m[1], m[2], m[3], cols[1]]; } else throw new Error(`line ${n + 1}: expected 4 tab-separated columns`);
        const bi = canonNames.findIndex((c) => squash(c) === squash(bk)); if (bi < 0) throw new Error(`line ${n + 1}: unknown book "${bk}"`);
        const c = parseInt(ch, 10), vv = parseInt(vs, 10);
        while (books[bi].length < c) books[bi].push([]);
        const arr = books[bi][c - 1]; while (arr.length < vv) arr.push(''); arr[vv - 1] = cleanVerse(tx);
      });
    }
    if (books.length !== 66) throw new Error(`found ${books.length} books, a Bible needs 66`);
    let total = 0; books.forEach((b) => b.forEach((c) => { if (!c.length) throw new Error('a chapter is empty'); total += c.length; }));
    if (total < 30000 || total > 32500) throw new Error(`${total} verses; a full Bible has about 31,100`);
    return { names: canonNames, books };
  }

  // ---------------------------------------------------------------- dates, names and documents
  const WEEKDAYS = ['Sunday', 'Monday', 'Tuesday', 'Wednesday', 'Thursday', 'Friday', 'Saturday'];
  const MONTHS = ['January', 'February', 'March', 'April', 'May', 'June', 'July', 'August', 'September', 'October', 'November', 'December'];
  function ordinal(d) { const s = d % 100 >= 11 && d % 100 <= 13 ? 'th' : ({ 1: 'st', 2: 'nd', 3: 'rd' }[d % 10] || 'th'); return d + s; }
  const p2 = (n) => String(n).padStart(2, '0');
  function longDate(d) { return `${WEEKDAYS[d.getDay()]} ${ordinal(d.getDate())} ${MONTHS[d.getMonth()]} ${d.getFullYear()}`; }
  function timeHMS(d) { return `${p2(d.getHours())}:${p2(d.getMinutes())}:${p2(d.getSeconds())}`; }
  function time12(d) { const h = d.getHours(); return `${h % 12 === 0 ? 12 : h % 12}:${p2(d.getMinutes())}:${p2(d.getSeconds())} ${h < 12 ? 'AM' : 'PM'}`; }
  function timeFile(d) { return `${p2(d.getHours())}h${p2(d.getMinutes())}m${p2(d.getSeconds())}s`; }
  function utcLabel(d) { const o = -d.getTimezoneOffset(); return `UTC${o < 0 ? '-' : '+'}${p2(Math.floor(Math.abs(o) / 60))}:${p2(Math.abs(o) % 60)}`; }
  function sameDay(a, b) { return a.getFullYear() === b.getFullYear() && a.getMonth() === b.getMonth() && a.getDate() === b.getDate(); }
  function hms(ms) { const s = Math.floor(ms / 1000); return `${p2(Math.floor(s / 3600))}:${p2(Math.floor(s / 60) % 60)}:${p2(s % 60)}`; }
  function humanDuration(ms) { const s = Math.floor(ms / 1000), h = Math.floor(s / 3600), m = Math.floor(s / 60) % 60, x = s % 60; return h ? `${h} h ${p2(m)} min ${p2(x)} s` : m ? `${m} min ${p2(x)} s` : `${x} s`; }
  function safeName(s) { return String(s || '').replace(/[^\p{L}\p{N} _'-]/gu, '-').replace(/\s+/g, ' ').replace(/^[- ]+|[- ]+$/g, '').slice(0, 60); }
  function folderName(start) { return longDate(start); }
  function finalBase(start, end, title) {
    const t = safeName(title), tp = t ? ' - ' + t : '';
    const endPart = sameDay(start, end) ? timeFile(end) : `${longDate(end)} ${timeFile(end)}`;
    return `${longDate(start)}${tp} - ${timeFile(start)} to ${endPart}`;
  }
  const clockAt = (start, ms) => new Date(start.getTime() + Math.floor(ms / 1000) * 1000);
  const linePrefix = (start, ms) => `[${hms(ms)} | ${timeHMS(clockAt(start, ms))}]`;
  const SRC = (s) => (/^voice/.test(s) ? 'spoken reference' : /^quotation/.test(s) ? 'quoted words' : s === 'confirm' ? 'review card' : s === 'nav' ? 'next/previous' : /^(manual|browser)$/.test(s) ? 'clicked by hand' : s === 'plan' ? 'sermon plan' : s === 'history' ? 'previous/next scripture' : s === 'again' ? 'shown again' : 'other');
  const GAP = 30000;
  /** meta = {title,church,version,start:Date,end:Date,duration,translation,engine,mic,recording,autoStaged,approved,dismissed} */
  function toTxt(meta, lines, cites) {
    const L = [];
    L.push(longDate(meta.start), 'RHEMAFLOW WEB SERVICE TRANSCRIPT', '='.repeat(68));
    L.push(`Day and date  : ${longDate(meta.start)}`);
    L.push(`Start time    : ${timeHMS(meta.start)} (${time12(meta.start)}) ${utcLabel(meta.start)}`);
    L.push(`End time      : ${sameDay(meta.start, meta.end) ? '' : longDate(meta.end) + ', '}${timeHMS(meta.end)} (${time12(meta.end)}) ${utcLabel(meta.end)}`);
    L.push(`Length        : ${humanDuration(meta.duration)} (${hms(meta.duration)})`);
    if (meta.title) L.push(`Title         : ${meta.title}`);
    if (meta.church) L.push(`Church        : ${meta.church}`);
    L.push(`Software      : ${meta.version}`, `Speech engine : ${meta.engine}`, `Bible shown   : ${meta.translation}`);
    if (meta.recording) L.push(`Recording     : ${meta.recording}`);
    const words = lines.reduce((n, l) => n + l.text.split(/\s+/).filter(Boolean).length, 0);
    L.push('', 'TOTALS', '------', `Transcript lines : ${lines.length}`, `Words heard      : ${words}`, `Scriptures shown : ${cites.length}`);
    const by = {}; cites.forEach((c) => { const k = SRC(c.source); by[k] = (by[k] || 0) + 1; });
    Object.keys(by).forEach((k) => L.push(`   ${k.padEnd(18)}: ${by[k]}`));
    L.push(`Staged automatically: ${meta.autoStaged || 0}    Review cards approved: ${meta.approved || 0}    dismissed: ${meta.dismissed || 0}`);
    L.push('', 'SCRIPTURE INDEX  [time into the service | clock time]', '-'.repeat(52));
    if (!cites.length) L.push('(no Scripture was shown)');
    cites.forEach((c) => { L.push(`${linePrefix(meta.start, c.ms)}  ${c.label}  (${SRC(c.source)})`); if (c.text) L.push('      ' + c.text.replace(/\s+/g, ' ')); });
    L.push('', 'TRANSCRIPT  [time into the service | clock time]', '-'.repeat(52));
    let prev = null;
    lines.forEach((l) => { if (prev !== null && l.ms - prev >= GAP) L.push(`            ... quiet for ${humanDuration(l.ms - prev)} (${timeHMS(clockAt(meta.start, prev))} to ${timeHMS(clockAt(meta.start, l.ms))}) ...`); L.push(`${linePrefix(meta.start, l.ms)} ${l.text.trim()}`); prev = l.ms; });
    L.push('', `--- end of transcript: ${longDate(meta.end)} at ${timeHMS(meta.end)} ---`);
    return L.join('\n') + '\n';
  }
  function srtTime(ms) { return `${p2(Math.floor(ms / 3600000))}:${p2(Math.floor(ms / 60000) % 60)}:${p2(Math.floor(ms / 1000) % 60)},${String(ms % 1000).padStart(3, '0')}`; }
  function toSrt(lines, totalMs) {
    let n = 0, s = '';
    lines.forEach((l, i) => {
      const text = l.text.trim(); if (!text) return; n++;
      const want = Math.min(8000, Math.max(1500, text.split(/\s+/).length * 400));
      const next = lines[i + 1] ? lines[i + 1].ms : Math.max(totalMs, l.ms + want);
      const end = Math.max(l.ms + 1000, Math.min(l.ms + want, next - 80));
      s += `${n}\n${srtTime(l.ms)} --> ${srtTime(end)}\n${text}\n\n`;
    });
    return s;
  }
  function toMarkdown(meta, lines, cites) {
    const esc = (t) => String(t).replace(/\|/g, '/').replace(/\n/g, ' ');
    const M = [`# ${longDate(meta.start)} \u2014 ${meta.title || 'Service'}`, ''];
    if (meta.church) M.push(`**${meta.church}**`, '');
    M.push('## Service details', '', '| | |', '|---|---|', `| Day and date | ${longDate(meta.start)} |`, `| Started | ${timeHMS(meta.start)} (${time12(meta.start)}) ${utcLabel(meta.start)} |`, `| Ended | ${timeHMS(meta.end)} (${time12(meta.end)}) |`, `| Length | ${humanDuration(meta.duration)} (${hms(meta.duration)}) |`, `| Speech engine | ${esc(meta.engine)} |`, `| Bible shown | ${meta.translation} |`, `| Scriptures shown | ${cites.length} |`, '');
    M.push('## Scripture index', '');
    if (!cites.length) M.push('_No Scripture was shown during this service._', '');
    else { M.push('| # | Into service | Clock | Scripture | How | Text |', '|---|---|---|---|---|---|'); cites.forEach((c, i) => M.push(`| ${i + 1} | ${hms(c.ms)} | ${timeHMS(clockAt(meta.start, c.ms))} | ${esc(c.label)} | ${SRC(c.source)} | ${esc(c.text || '')} |`)); M.push(''); }
    M.push('## Transcript', ''); let prev = null;
    lines.forEach((l) => { if (prev !== null && l.ms - prev >= GAP) M.push(`_\u2026 quiet for ${humanDuration(l.ms - prev)} \u2026_`, ''); M.push(`**${linePrefix(meta.start, l.ms)}** ${l.text.trim()}`, ''); prev = l.ms; });
    M.push('---', `_End of transcript \u2014 ${longDate(meta.end)} at ${timeHMS(meta.end)}._`);
    return M.join('\n') + '\n';
  }


  // ---------------------------------------------------------------- scripture history (previous / next scripture)
  /** Every scripture shown, in order. Back / forward walk it like a browser; nothing is thrown away.
   *  Next/previous VERSE inside the same chapter at the newest entry moves that entry instead of adding one. */
  function makeHistory(cap) {
    cap = cap || 200;
    const h = { entries: [], pos: 0 };
    const same = (a, b) => a.book === b.book && a.chapter === b.chapter && a.from === b.from && a.to === b.to;
    h.push = (p, label, ms, source, nav) => {
      const atHead = !h.entries.length || h.pos === h.entries.length - 1, last = h.entries[h.entries.length - 1];
      if (last && atHead && same(last.passage, p)) { h.pos = h.entries.length - 1; return; }
      if (last && nav && atHead && last.passage.book === p.book && last.passage.chapter === p.chapter) { last.passage = p; last.label = label; last.source = source; return; }
      h.entries.push({ passage: p, label, ms: ms || 0, source });
      if (h.entries.length > cap) h.entries.shift();
      h.pos = h.entries.length - 1;
    };
    h.canBack = () => h.entries.length > 0 && h.pos > 0;
    h.canForward = () => h.entries.length > 0 && h.pos + 1 < h.entries.length;
    h.back = () => (h.canBack() ? h.entries[--h.pos] : null);
    h.forward = () => (h.canForward() ? h.entries[++h.pos] : null);
    h.jump = (i) => (i >= 0 && i < h.entries.length ? h.entries[(h.pos = i)] : null);
    h.positionLabel = () => (h.entries.length ? `${h.pos + 1} of ${h.entries.length}` : 'empty');
    return h;
  }

  // ---------------------------------------------------------------- stage colours
  const COLOR_PRESETS = [['Black / white', '#000000', '#ffffff'], ['Deep navy', '#0a1634', '#ffffff'], ['Dark green', '#082818', '#f0faf0'], ['Maroon', '#400a14', '#fff0eb'], ['Charcoal', '#22242a', '#f5f5f5'], ['Royal purple', '#261046', '#faf5ff'], ['Warm paper', '#faf4e6', '#1e1810'], ['White / black', '#ffffff', '#0a0a0a'], ['Black / yellow', '#000000', '#ffe650']];
  const hexToRgb = (h) => { const m = /^#?([0-9a-f]{6})$/i.exec(String(h).trim()); if (!m) return [0, 0, 0]; const n = parseInt(m[1], 16); return [(n >> 16) & 255, (n >> 8) & 255, n & 255]; };
  const isLight = (hex) => { const [r, g, b] = hexToRgb(hex); return 0.299 * r + 0.587 * g + 0.114 * b > 150; };
  /** Text colour that stays readable if only the background is changed. */
  const autoText = (bgHex) => (isLight(bgHex) ? '#14100a' : '#ffffff');
  function luminance(hex) { const f = (v) => { v /= 255; return v <= 0.03928 ? v / 12.92 : Math.pow((v + 0.055) / 1.055, 2.4); }; const [r, g, b] = hexToRgb(hex); return 0.2126 * f(r) + 0.7152 * f(g) + 0.0722 * f(b); }
  const contrast = (a, b) => { const x = luminance(a), y = luminance(b); return (Math.max(x, y) + 0.05) / (Math.min(x, y) + 0.05); };

  // ---------------------------------------------------------------- slides (announcements, lyrics) and the countdown
  /** Blank line = next slide. A first line ending in ":" labels the slide ("Chorus:"). */
  function slidesFromText(text) {
    return String(text || '').replace(/\r/g, '').split(/\n\s*\n/).map((b) => b.trim()).filter(Boolean).map((b) => {
      const lines = b.split('\n'); let label = '';
      if (lines.length > 1 && /^[^:]{1,30}:$/.test(lines[0].trim())) label = lines.shift().trim().slice(0, -1);
      return { label, text: lines.join('\n').trim() };
    });
  }
  /** "Service starts in 04:59" style text for a countdown. */
  function countdownText(msLeft) { const s = Math.max(0, Math.ceil(msLeft / 1000)); const m = Math.floor(s / 60), x = s % 60; return `${p2(m)}:${p2(x)}`; }

  // ---------------------------------------------------------------- second screen
  /** window.open feature string that puts a pop-up exactly on a screen from the Window Management API. */
  function screenFeatures(sc) {
    const left = sc.availLeft !== undefined ? sc.availLeft : sc.left || 0, top = sc.availTop !== undefined ? sc.availTop : sc.top || 0;
    const w = sc.availWidth || sc.width || 1280, h = sc.availHeight || sc.height || 720;
    return `popup=yes,left=${Math.round(left)},top=${Math.round(top)},width=${Math.round(w)},height=${Math.round(h)}`;
  }
  /** Extended screen first, otherwise the only one. */
  function pickScreen(screens) { if (!screens || !screens.length) return null; return screens.find((s) => s.isPrimary === false) || screens.find((s) => !s.isPrimary) || screens[0]; }

  function passageLabel(bible, p) { const n = bible.names[p.book]; if (!p.chapter) return n; if (!p.from) return `${n} ${p.chapter}`; return p.to > p.from ? `${n} ${p.chapter}:${p.from}-${p.to}` : `${n} ${p.chapter}:${p.from}`; }
  /** Pull every scripture reference out of pasted sermon notes, in order. */
  function planFromText(bible, entries, text) {
    const out = [];
    text.split(/\r?\n/).forEach((line) => { if (!line.trim()) return; const r = parseReferences(bible, entries, line, null); [...r.detections, ...r.confirms.filter((c) => c.chapter > 0 && c.kind !== 'quote')].forEach((d) => { const p = { book: d.book, chapter: d.chapter, from: d.kind === 'chapter' ? 0 : d.from, to: d.kind === 'chapter' ? 0 : d.to }; if (p.from && bible.exists(p.book, p.chapter, p.from)) out.push({ passage: p, label: passageLabel(bible, p) }); }); });
    return out;
  }

  return { makeBible, normalizeWords, tokenize, buildBookIndex, matchBook, parseReferences, rankQuotation, searchText, search, interpret, shouldAutoApprove, matchCommands, commandsDetailed, pureCommands, commandList, cmdLabel, isBareStop, importBible, scoreFragment, readNumber, longDate, timeHMS, time12, timeFile, utcLabel, hms, humanDuration, safeName, folderName, finalBase, linePrefix, toTxt, toSrt, toMarkdown, ordinal, passageLabel, planFromText, zoomPercent, makeHistory, COLOR_PRESETS, hexToRgb, isLight, autoText, contrast, slidesFromText, countdownText, screenFeatures, pickScreen };
});
