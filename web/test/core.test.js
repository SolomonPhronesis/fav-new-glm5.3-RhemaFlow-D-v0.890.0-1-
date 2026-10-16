const test = require('node:test');
const assert = require('node:assert');
const fs = require('fs');
const path = require('path');
const R = require('../src/core.js');
const kjv = R.makeBible(JSON.parse(fs.readFileSync(path.join(__dirname, '../data/kjv.json'), 'utf8')));
const IDX = R.buildBookIndex(kjv.names);
const name = (b) => kjv.names[b];
const ref = (p) => `${name(p.book)} ${p.chapter}:${p.from}${p.to > p.from ? '-' + p.to : ''}`;
const parse = (t, ctx) => R.parseReferences(kjv, IDX, t, ctx || null);

test('spoken references in many shapes', () => {
  const cases = [
    ['Let us turn to John chapter 3 verse 16.', 'John 3:16'],
    ['Revelation 1:6', 'Revelation 1:6'],
    ['Revelations chapter 1 verse 6.', 'Revelation 1:6'],
    ['first corinthians thirteen four', '1 Corinthians 13:4'],
    ['second corinthians five seventeen', '2 Corinthians 5:17'],
    ['second Corinthians chapter five verse seventeen', '2 Corinthians 5:17'],
    ['Psalm 23 verse 1', 'Psalms 23:1'],
    ['Romans eight twenty eight', 'Romans 8:28'],
    ['Genesis 1-1', 'Genesis 1:1'],
    ['John 3 verse 16 to 18', 'John 3:16-18'],
    ['look at Hebrews chapter eleven verse one', 'Hebrews 11:1'],
    ['song of solomon 2 verse 4', 'Song of Solomon 2:4'],
    ['Psalm one hundred and nineteen verse one hundred and five', 'Psalms 119:105'],
  ];
  for (const [said, want] of cases) {
    const r = parse(said);
    assert.ok(r.detections.length, `${said} -> ${JSON.stringify(r)}`);
    assert.strictEqual(ref(r.detections[0]), want, said);
  }
});

test('chapter only and errors are honest', () => {
  const r = parse('turn to John chapter 3');
  assert.strictEqual(r.confirms[0].kind, 'chapter');
  assert.deepStrictEqual(parse('1 Corinthians 5 17').errors, ['1 Corinthians 5 has only 13 verses']);
  assert.deepStrictEqual(parse('first corinthians five seventeen').errors, ['1 Corinthians 5 has only 13 verses'], '5 and 17 are two numbers, not 22');
  assert.deepStrictEqual(parse('Jude chapter 4').errors, ['Jude has only 1 chapter']);
});

test('a bare verse number uses the current chapter', () => {
  const r = parse('Verse 6.', { last: { book: 65, chapter: 1 } });
  assert.strictEqual(ref(r.detections[0]), 'Revelation 1:6');
});

test('hath and have are the same word', () => {
  assert.deepStrictEqual(R.normalizeWords('And HATH made thee'), ['and', 'have', 'made', 'you']);
});

test('kings and priests is Revelation 1:6 and a clear winner over 5:10', () => {
  for (const said of ['and have made us kings and priests', 'and have made us kings and priests unto god', 'And have made those kings and priests unto God and his father.', 'Relations and have made us kings and priests.']) {
    const h = R.rankQuotation(kjv, said, 5);
    assert.deepStrictEqual([h[0].book, h[0].chapter, h[0].verse], [65, 1, 6], said);
    if (!/relations|those/i.test(said)) {
      const second = h[1] ? h[1].pct : 0;
      assert.ok(h[0].pct >= 75 && h[0].pct - second >= 12, `${said}: ${JSON.stringify(h)}`);
    }
  }
});

test('interpret: auto-approve stages kings-and-priests; off waits on a card; weak "1 Kings" never blocks', () => {
  for (const said of ['and have made us kings and priests', 'And have made us kings and priests unto God.', 'Relations and have made us kings and priests.']) {
    const on = R.interpret(kjv, IDX, said, null, { autoApprove: true });
    assert.strictEqual(on.stage.length, 1, said + JSON.stringify(on));
    assert.strictEqual(ref(on.stage[0]), 'Revelation 1:6');
    const off = R.interpret(kjv, IDX, said, null, { autoApprove: false });
    assert.strictEqual(off.stage.length, 0);
    assert.ok(off.cards.some((c) => c.kind === 'quote'), said);
    assert.ok(!off.cards.some((c) => c.kind === 'book'), said + ' has a book card: ' + JSON.stringify(off.cards));
  }
  const noise = R.interpret(kjv, IDX, 'and have made those kings entreeced', null, {});
  assert.ok(!noise.cards.some((c) => c.kind === 'book'));
});

test('grace of our Lord lists every candidate and waits for a person even with auto-approve', () => {
  const r = R.interpret(kjv, IDX, 'the grace of our lord jesus christ be with you', null, { autoApprove: true });
  assert.strictEqual(r.stage.length, 0);
  const q = r.cards.find((c) => c.kind === 'quote');
  assert.ok(q.alts.length >= 4);
  assert.ok(q.alts.every((a, i, arr) => i === 0 || arr[i - 1].pct >= a.pct));
});

test('a unique quote like John 3:16 is auto-approved', () => {
  const r = R.interpret(kjv, IDX, 'for god so loved the world', null, { autoApprove: true });
  assert.strictEqual(ref(r.stage[0]), 'John 3:16');
});

test('full-text search: god so loved -> John 3:16 first, others offered', () => {
  for (const q of ['god so loved', 'for god so loved the world', 'so loved the world', 'God so lov']) {
    const h = R.searchText(kjv, q, 10);
    assert.deepStrictEqual([h[0].book, h[0].chapter, h[0].verse], [42, 3, 16], q);
  }
  const names = R.searchText(kjv, 'kings and priests', 10).map((h) => `${name(h.book)} ${h.chapter}:${h.verse}`);
  assert.ok(names.includes('Revelation 1:6') && names.includes('Revelation 5:10'), names.join());
  assert.deepStrictEqual(R.searchText(kjv, 'zzzqqq', 5), []);
});

test('the one search box: books, chapters, references, words; no errors for "1 sam"', () => {
  assert.deepStrictEqual(R.search(kjv, IDX, '1 sam')[0], { type: 'book', book: 8 });
  assert.deepStrictEqual(R.search(kjv, IDX, '1sam')[0], { type: 'book', book: 8 });
  assert.deepStrictEqual(R.search(kjv, IDX, 'first samuel')[0], { type: 'book', book: 8 });
  assert.deepStrictEqual(R.search(kjv, IDX, 'john')[0], { type: 'book', book: 42 });
  assert.deepStrictEqual(R.search(kjv, IDX, 'rev')[0], { type: 'book', book: 65 });
  assert.deepStrictEqual(R.search(kjv, IDX, 'john 3')[0], { type: 'chapter', book: 42, chapter: 3 });
  const v = R.search(kjv, IDX, 'rev 1:6')[0];
  assert.deepStrictEqual([v.type, v.book, v.chapter, v.from, v.pct], ['verse', 65, 1, 6, 100]);
  const w = R.search(kjv, IDX, 'god so loved');
  assert.deepStrictEqual([w[0].book, w[0].chapter, w[0].from], [42, 3, 16]);
  assert.ok(!w.some((x) => x.type === 'book'));
  assert.ok(R.search(kjv, IDX, 'revelations').some((x) => x.type === 'book' && x.book === 65));
  assert.deepStrictEqual(R.search(kjv, IDX, '   '), []);
});

test('voice commands', () => {
  const m = (t, ref) => R.matchCommands(t, !!ref);
  assert.deepStrictEqual(m('full screen'), ['fullscreen']);
  assert.deepStrictEqual(m('exit full screen'), ['exit_fullscreen']);
  assert.deepStrictEqual(m('exit'), ['exit_fullscreen']);
  assert.deepStrictEqual(m('open stage'), ['open_stage']);
  assert.deepStrictEqual(m('close the stage window'), ['close_stage']);
  assert.deepStrictEqual(m('zoom 300'), ['zoom_300']);
  assert.deepStrictEqual(m('zoom to three hundred percent'), ['zoom_300']);
  assert.deepStrictEqual(m('max zoom'), ['zoom_400']);
  assert.deepStrictEqual(m('half max zoom'), ['zoom_200']);
  assert.deepStrictEqual(m('zoom in'), ['zoom_in']);
  assert.deepStrictEqual(m('approve it'), ['approve']);
  assert.deepStrictEqual(m('option two'), ['pick_2']);
  assert.ok(m('stop reading').includes('stop_reading'));
  assert.ok(R.isBareStop('Stop.'));
  assert.ok(!R.isBareStop('stop reading'));
  assert.deepStrictEqual(m('next verse'), ['next_verse']);
  assert.deepStrictEqual(m('John 3 next verse', true), [], 'a reference beats navigation');
  const long = 'and you know brother when the Lord says to clear it out of your life and wipe screen clean in your heart this morning';
  assert.deepStrictEqual(m(long), [], 'short phrases never fire mid-sermon');
  assert.deepStrictEqual(m('show it again'), ['last']);
  assert.deepStrictEqual(m('go black'), ['blackout'], 'black is not blank');
  assert.deepStrictEqual(m('go blank'), ['clear']);
});

test('dates, names and the detailed transcript (the user\'s own example)', () => {
  const start = new Date(2026, 9, 4, 1, 27, 43), end = new Date(2026, 9, 4, 2, 15, 10);
  assert.strictEqual(R.longDate(start), 'Sunday 4th October 2026');
  assert.strictEqual(R.timeFile(start), '01h27m43s');
  assert.strictEqual(R.time12(start), '1:27:43 AM');
  assert.strictEqual(R.folderName(start), 'Sunday 4th October 2026');
  assert.strictEqual(R.finalBase(start, end, ''), 'Sunday 4th October 2026 - 01h27m43s to 02h15m10s');
  assert.strictEqual(R.finalBase(start, end, 'Faith: that moves'), 'Sunday 4th October 2026 - Faith- that moves - 01h27m43s to 02h15m10s');
  const ords = { 1: '1st', 2: '2nd', 3: '3rd', 4: '4th', 11: '11th', 12: '12th', 13: '13th', 21: '21st', 22: '22nd', 23: '23rd', 31: '31st' };
  for (const [n, s] of Object.entries(ords)) assert.strictEqual(R.ordinal(+n), s);
  const lines = [{ ms: 63000, text: 'Revelations chapter 1 verse 6.' }, { ms: 66000, text: 'Read.' }, { ms: 130000, text: 'And hath made us kings and priests.' }];
  const cites = [{ ms: 65000, label: 'Revelation 1:6', source: 'voice', text: 'And hath made us kings' }];
  const meta = { title: 'Faith', church: 'Grace', version: 'RhemaFlow Web 0.1', start, end, duration: 47 * 60000 + 27000, translation: 'KJV', engine: 'Web Speech API', mic: '', recording: 'x.webm', autoStaged: 1, approved: 0, dismissed: 0 };
  const t = R.toTxt(meta, lines, cites);
  assert.strictEqual(t.split('\n')[0], 'Sunday 4th October 2026');
  assert.ok(t.includes('Start time    : 01:27:43 (1:27:43 AM)'));
  assert.ok(t.includes('Length        : 47 min 27 s (00:47:27)'));
  assert.ok(t.includes('[00:01:03 | 01:28:46] Revelations chapter 1 verse 6.'));
  assert.ok(t.includes('quiet for 1 min 04 s'));
  const md = R.toMarkdown(meta, lines, cites);
  assert.ok(md.startsWith('# Sunday 4th October 2026 \u2014 Faith'));
  const srt = R.toSrt(lines, 90000);
  assert.ok(srt.startsWith('1\n00:01:03,000 --> '));
});

test('importer reads the other layouts (real ASV file when present)', () => {
  // synthetic: tab layout from KJV
  let tsv = '';
  kjv.names.forEach((n, b) => kjv.books[b].forEach((ch, ci) => ch.forEach((t, vi) => { tsv += `${n}\t${ci + 1}\t${vi + 1}\t${t}\n`; })));
  const t = R.importBible(tsv, kjv.names);
  assert.strictEqual(R.makeBible(t).verse(42, 3, 16), kjv.verse(42, 3, 16));
  assert.throws(() => R.importBible('{"books":[]}', kjv.names), /66/);
  const asv = '/tmp/dl_ASV.json';
  if (fs.existsSync(asv)) {
    const b = R.makeBible(R.importBible(fs.readFileSync(asv, 'utf8'), kjv.names));
    assert.strictEqual(b.total(), 31102);
    assert.ok(b.verse(42, 3, 16).startsWith('For God so loved'));
  }
});

test('sermon plan finds references in pasted notes', () => {
  const plan = R.planFromText(kjv, IDX, 'Intro\nOpen with John 3:16-18\n\nthen Romans 8:28 and\nPsalm 23:1\nno reference here');
  assert.deepStrictEqual(plan.map((p) => p.label), ['John 3:16-18', 'Romans 8:28', 'Psalms 23:1']);
});

test('the user\'s 5 Oct transcript: "Next." "Previous." "Netverse." all work', () => {
  const m = (t) => R.matchCommands(t, false);
  for (const [said, want] of [['Next.', 'next_verse'], ['Next verse.', 'next_verse'], ['Netverse.', 'next_verse'], ['nextverse', 'next_verse'], ['nex verse', 'next_verse'], ['Previous.', 'prev_verse'], ['Previews.', 'prev_verse'], ['Back.', 'prev_verse'], ['ok next please', 'next_verse'], ['Previous chapter.', 'prev_chapter'], ['Next chapter', 'next_chapter'], ['Read.', 'read'], ['Clear.', 'clear'], ['Again.', 'last']]) {
    assert.deepStrictEqual(m(said), [want], said);
  }
  // not commands: ordinary speech
  for (const said of ['Next week we will meet at nine', 'the next verse says something wonderful about grace', 'we should not look back on the past', 'I will read the whole chapter now to everyone here']) {
    assert.deepStrictEqual(m(said), [], said);
  }
});

test('pureCommands only acts on interim text made entirely of command words', () => {
  assert.deepStrictEqual(R.pureCommands('next verse'), ['next_verse']);
  assert.deepStrictEqual(R.pureCommands('next'), ['next_verse']);
  assert.deepStrictEqual(R.pureCommands('next verse and then we will'), []);
  assert.deepStrictEqual(R.pureCommands('John 3 verse 16'), []);
  assert.deepStrictEqual(R.pureCommands('zoom 300'), ['zoom_300']);
});

test('chapter and verse without a book use the current book', () => {
  const ctx = { last: { book: 0, chapter: 1 } };
  const one = (t) => R.parseReferences(kjv, IDX, t, ctx);
  let r = one('chapter two verse four'); assert.strictEqual(ref(r.detections[0]), 'Genesis 2:4');
  r = one('verse six to nine'); assert.strictEqual(ref(r.detections[0]), 'Genesis 1:6-9');
  r = one('chapter three'); assert.strictEqual(r.confirms[0].kind, 'chapter');
  assert.deepStrictEqual(one('chapter nine hundred').errors, ['Genesis has only 50 chapters']);
  assert.deepStrictEqual(one('verse 99').errors, ['Genesis 1 has only 31 verses']);
});

test('the command list for the dialog covers every group and has no empty entries', () => {
  const l = R.commandList();
  assert.ok(l.length >= 25);
  for (const g of l) { assert.ok(g.label && g.phrases.length, g.name); }
  assert.ok(l.find((g) => g.name === 'next_verse').phrases.some((p) => p.p.startsWith('next (on its own)')));
});

test('previous scripture walks the history one at a time (voice + data structure)', () => {
  assert.deepStrictEqual(R.matchCommands('previous scripture', false), ['prev_scripture']);
  assert.deepStrictEqual(R.matchCommands('go back to the previous scripture', false), ['prev_scripture']);
  assert.deepStrictEqual(R.matchCommands('the one before', false), ['prev_scripture']);
  assert.deepStrictEqual(R.matchCommands('next scripture', false), ['next_scripture']);
  assert.deepStrictEqual(R.matchCommands('previous verse', false), ['prev_verse']);
  const h = R.makeHistory(), P = (b, c, v) => ({ book: b, chapter: c, from: v, to: v });
  h.push(P(42, 3, 16), 'John 3:16', 0, 'voice'); h.push(P(44, 8, 28), 'Romans 8:28', 0, 'voice'); h.push(P(18, 23, 1), 'Psalms 23:1', 0, 'voice');
  assert.strictEqual(h.back().label, 'Romans 8:28');
  assert.strictEqual(h.back().label, 'John 3:16', 'and the one before that');
  assert.strictEqual(h.back(), null);
  assert.strictEqual(h.forward().label, 'Romans 8:28');
  h.push(P(65, 1, 6), 'Revelation 1:6', 0, 'voice');
  assert.deepStrictEqual(h.entries.map((e) => e.label), ['John 3:16', 'Romans 8:28', 'Revelation 1:6'].length ? ['John 3:16', 'Romans 8:28', 'Psalms 23:1', 'Revelation 1:6'] : []);
  assert.strictEqual(h.positionLabel(), '4 of 4');
});

test('history: reading along with next verse moves one entry instead of flooding the list', () => {
  const h = R.makeHistory(), P = (c, v) => ({ book: 42, chapter: c, from: v, to: v });
  h.push(P(3, 16), 'John 3:16', 0, 'voice');
  for (let v = 17; v <= 25; v++) h.push(P(3, v), `John 3:${v}`, 0, 'nav', true);
  assert.strictEqual(h.entries.length, 1);
  assert.strictEqual(h.entries[0].label, 'John 3:25');
  h.push(P(4, 1), 'John 4:1', 0, 'nav', true);
  assert.strictEqual(h.entries.length, 2, 'a new chapter is a new entry');
});

test('colours: readable pairs, auto text colour, every preset passes contrast 7:1', () => {
  for (const [name, bg, fg] of R.COLOR_PRESETS) assert.ok(R.contrast(bg, fg) >= 7, `${name} ${R.contrast(bg, fg).toFixed(1)}`);
  assert.strictEqual(R.autoText('#0a1634'), '#ffffff');
  assert.strictEqual(R.autoText('#faf4e6'), '#14100a');
  assert.deepStrictEqual(R.hexToRgb('#ff8000'), [255, 128, 0]);
});

test('slides: blank line = next slide, "Chorus:" labels it; countdown text', () => {
  const sl = R.slidesFromText('Welcome\nto church\n\nChorus:\nAmazing grace\nhow sweet the sound\n\n\n  \nAnnouncements');
  assert.strictEqual(sl.length, 3);
  assert.deepStrictEqual(sl[1], { label: 'Chorus', text: 'Amazing grace\nhow sweet the sound' });
  assert.strictEqual(sl[0].label, '');
  assert.strictEqual(R.countdownText(299001), '05:00');
  assert.strictEqual(R.countdownText(-5), '00:00');
  assert.strictEqual(R.countdownText(61000), '01:01');
});

test('second screen: pick the extended one and place the pop-up exactly on it', () => {
  const screens = [{ isPrimary: true, availLeft: 0, availTop: 25, availWidth: 1440, availHeight: 875 }, { isPrimary: false, availLeft: 1440, availTop: -180, availWidth: 1920, availHeight: 1080 }];
  assert.strictEqual(R.pickScreen(screens), screens[1]);
  assert.strictEqual(R.pickScreen([screens[0]]), screens[0]);
  assert.strictEqual(R.pickScreen([]), null);
  assert.strictEqual(R.screenFeatures(screens[1]), 'popup=yes,left=1440,top=-180,width=1920,height=1080');
});
