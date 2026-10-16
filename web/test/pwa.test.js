// v0.4 PWA. Written BEFORE the code. Needs `python3 web/build.py` to have made web/dist first.
const test = require('node:test');
const assert = require('node:assert');
const fs = require('fs');
const path = require('path');
const dist = path.join(__dirname, '..', 'dist');
const read = (f) => fs.readFileSync(path.join(dist, f), 'utf8');

function pngSize(file) {
  const b = fs.readFileSync(path.join(dist, file));
  assert.strictEqual(b.toString('latin1', 1, 4), 'PNG', file + ' is a PNG');
  return [b.readUInt32BE(16), b.readUInt32BE(20)];
}

test('the deploy folder has everything and nothing private', () => {
  for (const f of ['index.html', 'manifest.webmanifest', 'sw.js', '_headers', 'icons/icon-192.png', 'icons/icon-512.png', 'icons/icon-maskable-512.png', 'icons/apple-touch-icon.png']) {
    assert.ok(fs.existsSync(path.join(dist, f)), f + ' missing');
  }
  for (const f of ['src', 'test', 'build.py', 'data']) assert.ok(!fs.existsSync(path.join(dist, f)), f + ' must not be published');
  assert.ok(fs.statSync(path.join(dist, 'index.html')).size > 1_000_000, 'index.html is the whole app (KJV inside)');
});

test('the manifest can be installed', () => {
  const m = JSON.parse(read('manifest.webmanifest'));
  assert.strictEqual(m.name, 'RhemaFlow');
  assert.ok(m.short_name && m.short_name.length <= 12);
  assert.strictEqual(m.display, 'standalone');
  assert.ok(/^#[0-9a-f]{6}$/i.test(m.theme_color) && /^#[0-9a-f]{6}$/i.test(m.background_color));
  assert.ok(m.start_url && m.scope);
  const sizes = {};
  for (const i of m.icons) {
    assert.ok(fs.existsSync(path.join(dist, i.src)), i.src + ' listed but missing');
    const [w, h] = pngSize(i.src);
    assert.strictEqual(`${w}x${h}`, i.sizes, i.src + ' really is ' + i.sizes);
    sizes[i.sizes + (i.purpose ? ':' + i.purpose : '')] = true;
  }
  assert.ok(sizes['192x192'] && sizes['512x512'] && sizes['512x512:maskable'], Object.keys(sizes).join());
});

test('the page links the manifest and registers the worker only over http(s)', () => {
  const html = read('index.html');
  assert.match(html, /<link rel="manifest" href="manifest\.webmanifest">/);
  assert.match(html, /<meta name="theme-color" content="#0d1017">/);
  assert.match(html, /rel="apple-touch-icon" href="icons\/apple-touch-icon\.png"/);
  assert.match(html, /serviceWorker\.register\(['"]sw\.js['"]/);
  assert.match(html, /\^https\?:\$/, 'not registered from file://');
});

test('the worker caches only files that exist (a missing file would stop it installing)', () => {
  const sw = read('sw.js');
  assert.ok(!sw.includes('__BUILD__'), 'build id was filled in');
  const m = sw.match(/const SHELL_FILES = (\[[^\]]*\])/);
  assert.ok(m, 'SHELL_FILES found');
  const files = JSON.parse(m[1].replace(/'/g, '"'));
  assert.ok(files.length >= 4);
  for (const f of files) {
    const p = f === './' ? 'index.html' : f;
    assert.ok(fs.existsSync(path.join(dist, p)), f + ' is precached but missing');
  }
});

test('a new build gets a new cache name, the same build the same one', () => {
  const a = read('sw.js').match(/const BUILD = '([0-9a-f]+)'/);
  assert.ok(a && a[1].length >= 8, 'build id');
  const html = read('index.html');
  const crypto = require('crypto');
  assert.strictEqual(a[1], crypto.createHash('sha1').update(html).digest('hex').slice(0, 10), 'the id comes from the page itself, so any change to the app changes it');
});

test('routing: what is served from where', () => {
  const sw = require('../src/sw.js');
  const base = 'https://app.example.org/';
  const r = (u, mode = 'no-cors', method = 'GET') => sw.routeFor(new URL(u, base).href, mode, method, base);
  assert.strictEqual(r('/', 'navigate'), 'shell');
  assert.strictEqual(r('/index.html'), 'shell');
  assert.strictEqual(r('/anything-else', 'navigate'), 'shell', 'any page address opens the app');
  assert.strictEqual(r('/icons/icon-192.png'), 'shell');
  assert.strictEqual(r('/manifest.webmanifest'), 'shell');
  assert.strictEqual(r('/bibles/ASV.json'), 'bible');
  assert.strictEqual(r('/bibles/Rotherham.json'), 'bible');
  assert.strictEqual(r('/sw.js'), 'skip', 'the worker file itself is never cached by itself');
  assert.strictEqual(r('/api/anything'), 'network');
  assert.strictEqual(r('https://other.example.net/x.js'), 'skip', 'other sites are left alone');
  assert.strictEqual(r('https://raw.githubusercontent.com/x/y.json'), 'skip');
  assert.strictEqual(r('/bibles/ASV.json', 'no-cors', 'POST'), 'skip', 'only GET');
});
