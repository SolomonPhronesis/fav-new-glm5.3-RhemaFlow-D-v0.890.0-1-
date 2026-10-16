/* RhemaFlow Web service worker (v0.4). Own code.
 *
 * What it does, in plain words:
 *  - the first time the page opens over http(s), it keeps a copy of the app (the page already contains the whole
 *    KJV, so the app works with no internet after that);
 *  - Bible versions fetched from the bibles/ folder are kept the first time they are fetched;
 *  - it NEVER swaps itself for a newer version while the app is open: a new build waits until every RhemaFlow
 *    window is closed (so nothing can change under an operator in the middle of a service).
 */
const BUILD = '__BUILD__';
const SHELL = 'rf-shell-' + BUILD;
const DATA = 'rf-bibles-v1';
const SHELL_FILES = ['./', 'index.html', 'manifest.webmanifest', 'icons/icon-192.png', 'icons/icon-512.png', 'icons/icon-maskable-512.png', 'icons/apple-touch-icon.png'];

/** Where should a request be answered from? 'shell' (kept copy first), 'bible' (kept after first fetch),
 *  'network' (leave to the browser), 'skip' (not ours). Pure, so it is tested without a browser. */
function routeFor(href, mode, method, scope) {
  if (method !== 'GET') return 'skip';
  const u = new URL(href);
  const s = new URL(scope);
  if (u.origin !== s.origin) return 'skip';
  const rel = u.pathname.slice(s.pathname.length);
  if (rel === 'sw.js') return 'skip';
  if (mode === 'navigate') return 'shell';
  if (rel === '' || rel === 'index.html' || rel === 'manifest.webmanifest' || rel.startsWith('icons/')) return 'shell';
  if (/^bibles\/[^/]+\.json$/.test(rel)) return 'bible';
  return 'network';
}

if (typeof module !== 'undefined') module.exports = { routeFor };

if (typeof self !== 'undefined' && self.addEventListener && typeof caches !== 'undefined') {
  self.addEventListener('install', (e) => {
    // no skipWaiting(): see the note at the top
    e.waitUntil(caches.open(SHELL).then((c) => c.addAll(SHELL_FILES)));
  });
  self.addEventListener('activate', (e) => {
    e.waitUntil(
      caches.keys().then((keys) => Promise.all(keys.filter((k) => k.startsWith('rf-shell-') && k !== SHELL).map((k) => caches.delete(k)))).then(() => self.clients.claim())
    );
  });
  self.addEventListener('fetch', (e) => {
    const kind = routeFor(e.request.url, e.request.mode, e.request.method, self.registration.scope);
    if (kind === 'shell') e.respondWith(fromShell(e.request));
    else if (kind === 'bible') e.respondWith(fromBibles(e.request));
  });

  async function fromShell(req) {
    const c = await caches.open(SHELL);
    const hit = await c.match(req.mode === 'navigate' ? 'index.html' : req, { ignoreSearch: true });
    if (hit) return hit;
    try {
      return await fetch(req);
    } catch (err) {
      const fallback = await c.match('index.html');
      if (fallback) return fallback;
      throw err;
    }
  }

  async function fromBibles(req) {
    const c = await caches.open(DATA);
    const hit = await c.match(req);
    if (hit) return hit;
    const res = await fetch(req);
    if (res.ok) c.put(req, res.clone());
    return res;
  }
}
