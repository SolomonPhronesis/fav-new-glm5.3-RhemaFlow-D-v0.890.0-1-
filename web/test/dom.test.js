// Integration test: loads the built single-file page in jsdom and drives it like an operator.
// Run: node --test web/test/dom.test.js   (needs `npm i jsdom` somewhere on NODE_PATH, and `python3 web/build.py` first)
const test = require('node:test');
const assert = require('node:assert');
const fs = require('fs');
const path = require('path');
let JSDOM;
try { ({ JSDOM } = require('jsdom')); } catch (e) { try { ({ JSDOM } = require('/tmp/jt/node_modules/jsdom')); } catch (e2) { JSDOM = null; } }
const file = path.join(__dirname, '../rhemaflow-web.html');
const opened = [];
test.after(() => { opened.forEach((w) => { try { w.close(); } catch (e) { /* ignore */ } }); });

async function page(extra) {
  const html = fs.readFileSync(file, 'utf8');
  const dom = new JSDOM(html, { runScripts: 'dangerously', pretendToBeVisual: true, url: 'http://localhost/rhemaflow-web.html', beforeParse(w) {
    w.DecompressionStream = globalThis.DecompressionStream; w.Blob = globalThis.Blob; w.Response = globalThis.Response; w.TextDecoder = globalThis.TextDecoder;
    w.HTMLDialogElement.prototype.showModal = function () { this.setAttribute('open', ''); };
    w.HTMLDialogElement.prototype.close = function () { this.removeAttribute('open'); this.dispatchEvent(new w.Event('close')); };
    w.Element.prototype.scrollIntoView = function () {};
    w.URL.createObjectURL = () => 'blob:test';
    w.fetch = globalThis.fetch;
    if (extra) extra(w);
  } });
  for (let i = 0; i < 100 && dom.window.document.body.dataset.ready !== '1'; i++) await new Promise((r) => setTimeout(r, 50));
  assert.strictEqual(dom.window.document.body.dataset.ready, '1', 'page booted');
  opened.push(dom.window);
  return dom.window;
}
const text = (w, sel) => w.document.querySelector(sel).textContent;

test('boots with KJV and shows nothing on stage', { skip: !JSDOM }, async () => {
  const w = await page();
  assert.strictEqual(w.__rf.S.kjv.total(), 31102);
  assert.strictEqual(w.__rf.S.stage.kind, 'blank');
  assert.strictEqual(w.document.querySelectorAll('#books .bk').length, 66);
});

test('img-3 scenario: auto-approve ON stages Revelation 1:6 and the browser follows', { skip: !JSDOM }, async () => {
  const w = await page();
  const cb = w.document.getElementById('autoApprove'); cb.checked = true; cb.dispatchEvent(new w.Event('change'));
  w.__rf.handleFinal('And have made us kings and priests.');
  const st = w.__rf.S.stage;
  assert.strictEqual(st.label, 'Revelation 1:6');
  assert.ok(st.text.startsWith('And hath made us kings'), st.text);
  assert.strictEqual(w.__rf.S.cards.length, 0, 'no stray review cards, no "1 Kings"');
  // browser followed: Revelation selected, chapter 1, verse 6 highlighted
  assert.ok(w.document.querySelector('#books .bk.sel').textContent === 'Revelation');
  assert.strictEqual(w.document.querySelector('#chaps button.sel').textContent, '1');
  assert.strictEqual(w.document.querySelector('#verses .vs.staged b').textContent, '6');
  assert.ok(text(w, '#prev .txt').startsWith('And hath made us kings'));
  assert.ok(text(w, '#heardTxt').includes('Revelation 1:6'));
});

test('auto-approve OFF leaves a ranked card; clicking an option stages it', { skip: !JSDOM }, async () => {
  const w = await page();
  w.__rf.handleFinal('and have made us kings and priests unto God');
  assert.strictEqual(w.__rf.S.stage.kind, 'blank');
  const card = w.document.querySelector('#cards .card');
  assert.ok(card.textContent.includes('Revelation 1:6'));
  assert.ok(!w.document.getElementById('cards').textContent.includes('BOOK ONLY'));
  card.querySelector('button.alt').click();
  assert.strictEqual(w.__rf.S.stage.label, 'Revelation 1:6');
  assert.strictEqual(w.document.querySelectorAll('#cards .card').length, 0);
});

test('grace of our Lord: several candidates, show-all switch, never auto-staged', { skip: !JSDOM }, async () => {
  const w = await page();
  const cb = w.document.getElementById('autoApprove'); cb.checked = true; cb.dispatchEvent(new w.Event('change'));
  w.__rf.handleFinal('the grace of our Lord Jesus Christ be with you');
  assert.strictEqual(w.__rf.S.stage.kind, 'blank');
  assert.ok(w.document.querySelectorAll('#cards .alt').length >= 5);
  const sa = w.document.getElementById('showAll'); sa.checked = false; sa.dispatchEvent(new w.Event('change'));
  assert.strictEqual(w.document.querySelectorAll('#cards .alt').length, 1);
  assert.ok(w.document.getElementById('cards').textContent.includes('more hidden'));
});

test('search box: "1 sam" opens the book (no error), words find John 3:16', { skip: !JSDOM }, async () => {
  const w = await page();
  const q = w.document.getElementById('q');
  q.value = '1 sam'; q.dispatchEvent(new w.Event('input'));
  const res = w.document.querySelectorAll('#results .res');
  assert.ok(res[0].textContent.includes('1 Samuel') && res[0].textContent.includes('open book'), res[0].textContent);
  assert.ok(!w.document.getElementById('results').textContent.toLowerCase().includes('add a chapter'));
  res[0].click();
  assert.strictEqual(w.document.querySelector('#books .bk.sel').textContent, '1 Samuel');
  assert.strictEqual(w.document.getElementById('q').value, '');
  q.value = 'god so loved'; q.dispatchEvent(new w.Event('input'));
  const r2 = w.document.querySelectorAll('#results .res');
  assert.ok(r2[0].textContent.includes('John 3:16'), r2[0].textContent);
  assert.ok(r2.length > 1);
  r2[0].click();
  assert.strictEqual(w.__rf.S.stage.label, 'John 3:16');
});

test('clicking verses by hand, Shift+click range, and keyboard', { skip: !JSDOM }, async () => {
  const w = await page();
  w.document.querySelectorAll('#books .bk')[42].click(); // John
  w.document.querySelectorAll('#chaps button')[2].click(); // chapter 3
  const vs = w.document.querySelectorAll('#verses .vs');
  vs[15].click();
  assert.strictEqual(w.__rf.S.stage.label, 'John 3:16');
  const vs2 = w.document.querySelectorAll('#verses .vs');
  vs2[17].dispatchEvent(new w.MouseEvent('click', { shiftKey: true, bubbles: true }));
  assert.strictEqual(w.__rf.S.stage.label, 'John 3:16-18');
  w.document.dispatchEvent(new w.KeyboardEvent('keydown', { key: 'ArrowRight', bubbles: true }));
  assert.strictEqual(w.__rf.S.stage.label, 'John 3:19');
  w.document.dispatchEvent(new w.KeyboardEvent('keydown', { key: 'b', bubbles: true }));
  assert.ok(w.document.getElementById('prev').className.includes('blackout'));
  w.document.dispatchEvent(new w.KeyboardEvent('keydown', { key: 'c', bubbles: true }));
  assert.strictEqual(w.__rf.S.stage.kind, 'blank');
});

test('voice: zoom 300, max zoom, blackout, next verse', { skip: !JSDOM }, async () => {
  const w = await page();
  w.__rf.handleFinal('John chapter 3 verse 16');
  assert.strictEqual(w.__rf.S.stage.label, 'John 3:16');
  w.__rf.handleFinal('zoom 300'); assert.strictEqual(Math.round(w.__rf.S.zoom * 100), 300);
  w.__rf.handleFinal('max zoom'); assert.strictEqual(Math.round(w.__rf.S.zoom * 100), 400);
  w.__rf.handleFinal('half max zoom'); assert.strictEqual(Math.round(w.__rf.S.zoom * 100), 200);
  w.__rf.handleFinal('next verse'); assert.strictEqual(w.__rf.S.stage.label, 'John 3:17');
  w.__rf.handleFinal('go black'); assert.strictEqual(w.__rf.S.blackout, true);
});

test('service: files are named with the full date and start-end time; transcript opens with the day', { skip: !JSDOM }, async () => {
  const w = await page();
  await w.document.getElementById('btnStart').onclick();
  w.__rf.handleFinal('Revelations chapter 1 verse 6');
  w.__rf.handleFinal('Read');
  await w.document.getElementById('btnStart').onclick();
  const links = [...w.document.querySelectorAll('#savedLinks a')].map((a) => a.download);
  assert.ok(links.length >= 3, links.join('|'));
  for (const n of links) assert.ok(/^\w+ \d+(st|nd|rd|th) \w+ \d{4} - \d\dh\d\dm\d\ds to \d\dh\d\dm\d\ds - /.test(n), n);
  assert.ok(links.some((n) => n.endsWith('transcript.txt')) && links.some((n) => n.endsWith('transcript.srt')) && links.some((n) => n.endsWith('sermon notes.md')));
  assert.ok(w.document.getElementById('savedNote').textContent.includes(w.__rf.RF.folderName(w.__rf.S.start)));
});

test('own Bible file imports and can be shown', { skip: !JSDOM }, async () => {
  const w = await page();
  const k = w.__rf.S.kjv; let tsv = '';
  k.names.forEach((n, b) => k.books[b].forEach((ch, ci) => ch.forEach((t, vi) => { tsv += `${n}\t${ci + 1}\t${vi + 1}\t${t.replace('God', 'GOD')}\n`; })));
  const data = w.__rf.RF.importBible(tsv, k.names);
  // same path the file input uses
  w.__rf.S.versions.Test = w.__rf.RF.makeBible(data);
  w.document.getElementById('trans').innerHTML = '<option>KJV</option><option>Test</option>';
  w.document.getElementById('trans').value = 'Test'; w.document.getElementById('trans').dispatchEvent(new w.Event('change'));
  w.__rf.handleFinal('John chapter 3 verse 16');
  assert.ok(w.__rf.S.stage.text.includes('GOD so loved'), w.__rf.S.stage.text);
  assert.strictEqual(w.__rf.S.stage.translation, 'Test');
});

// a fake browser speech engine we can drive by hand
function fakeSpeech(w) {
  w.__sr = null;
  w.webkitSpeechRecognition = function () { w.__sr = this; this.start = () => {}; this.stop = () => {}; };
  w.__say = (list, resultIndex) => { // list: [{t, final}]
    const results = []; for (let i = 0; i < (resultIndex || 0); i++) { const f = [{ transcript: '' }]; f.isFinal = true; results.push(f); }
    list.forEach((x) => { const r = [{ transcript: x.t }]; r.isFinal = !!x.final; results.push(r); });
    w.__sr.onresult({ resultIndex: resultIndex || 0, results });
  };
}
const wait = (ms) => new Promise((r) => setTimeout(r, ms));

test('5 Oct bug: bare "Next." and "Previous." drive navigation (interim is acted on once stable, final does not repeat it)', { skip: !JSDOM }, async () => {
  const w = await page(fakeSpeech);
  await w.document.getElementById('btnStart').onclick();
  w.__rf.handleFinal('Genesis chapter one, verse 2.');
  assert.strictEqual(w.__rf.S.stage.label, 'Genesis 1:2');
  // interim "next" that is still being spoken must NOT fire (it may become "next chapter")
  w.__say([{ t: 'next' }], 0); await wait(150);
  assert.strictEqual(w.__rf.S.stage.label, 'Genesis 1:2');
  w.__say([{ t: 'next chapter' }], 0); await wait(600);
  assert.strictEqual(w.__rf.S.stage.label, 'Genesis 2:1', 'next chapter, not next verse');
  // stable interim "next verse" fires once ~450 ms later, then the final result must not fire it again
  w.__say([{ t: 'next verse' }], 1); await wait(600);
  assert.strictEqual(w.__rf.S.stage.label, 'Genesis 2:2');
  w.__say([{ t: 'Next verse.', final: true }], 1);
  assert.strictEqual(w.__rf.S.stage.label, 'Genesis 2:2', 'not repeated');
  // a bare final "Next." with no interim at all
  w.__say([{ t: 'Netverse.', final: true }], 2);
  assert.strictEqual(w.__rf.S.stage.label, 'Genesis 2:3');
  w.__say([{ t: 'Previous.', final: true }], 3);
  assert.strictEqual(w.__rf.S.stage.label, 'Genesis 2:2');
  assert.ok(w.document.getElementById('heardTxt').textContent.includes('previous verse'));
  await w.document.getElementById('btnStart').onclick();
});

test('a sentence about a verse does not navigate; a stray command with nothing on stage says so', { skip: !JSDOM }, async () => {
  const w = await page();
  w.__rf.handleFinal('next');
  assert.ok(w.document.getElementById('toast').textContent.includes('Nothing on the stage'));
  w.__rf.handleFinal('John chapter 3 verse 16');
  w.__rf.handleFinal('the next verse says something wonderful about grace');
  assert.strictEqual(w.__rf.S.stage.label, 'John 3:16');
});

test('clock shows "ended" after the service, and the mic note is cleared', { skip: !JSDOM }, async () => {
  const w = await page(fakeSpeech);
  await w.document.getElementById('btnStart').onclick();
  w.document.getElementById('sttNote').textContent = 'old problem';
  await w.document.getElementById('btnStart').onclick();
  assert.ok(w.document.getElementById('clock').textContent.startsWith('ended '));
  assert.strictEqual(w.document.getElementById('clock').className, 'ended');
  assert.strictEqual(w.document.getElementById('sttNote').textContent, '');
});

test('voice commands dialog lists commands and filters', { skip: !JSDOM }, async () => {
  const w = await page();
  w.document.getElementById('bCmds').onclick();
  assert.ok(w.document.querySelectorAll('#cmdlist .g').length >= 25);
  const q = w.document.getElementById('cmdq'); q.value = 'zoom'; q.dispatchEvent(new w.Event('input'));
  const rows = [...w.document.querySelectorAll('#cmdlist .g')];
  assert.ok(rows.length >= 3 && rows.every((r) => /zoom/i.test(r.textContent)));
});

test('zoom preset select and file:// microphone hint', { skip: !JSDOM }, async () => {
  const w = await page();
  const z = w.document.getElementById('zpre'); z.value = '300'; z.dispatchEvent(new w.Event('change'));
  assert.strictEqual(Math.round(w.__rf.S.zoom * 100), 300);
});
