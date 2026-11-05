// Runs a built page's inline search script (seite/search-script.html) under
// Node with a minimal DOM shim, against the built index in `dist/`.
//
//   node run.js <dist dir> <page.html> <queries.json>
//
// queries.json: [{"q": "text", "keys": ["ArrowDown", ...]}, ...]
// Prints JSON: one entry per query with the rendered results (href, title,
// marked words, snippet), the status text, the active result after the keys,
// the fetched URLs and the bytes downloaded for that query.
'use strict';
const fs = require('fs');
const path = require('path');

const [dist, pageFile, queriesFile] = process.argv.slice(2);
const html = fs.readFileSync(pageFile, 'utf8');
const scripts = [...html.matchAll(/<script>([\s\S]*?)<\/script>/g)].map((m) => m[1]);
const source = scripts.find((s) => s.includes("getElementById('search-input')"));
if (!source) {
  console.log(JSON.stringify({ error: 'no search script' }));
  process.exit(0);
}

class Node_ {
  constructor(tag) {
    this.tagName = tag ? tag.toUpperCase() : '#text';
    this.children = [];
    this.attrs = {};
    this.style = {};
    this.listeners = {};
    this.parentNode = null;
    this._text = '';
    this.className = '';
    const cls = new Set();
    this.classList = {
      add: (c) => cls.add(c),
      remove: (c) => cls.delete(c),
      contains: (c) => cls.has(c),
    };
  }
  get textContent() {
    if (this.tagName === '#text') return this._text;
    return this.children.map((c) => c.textContent).join('');
  }
  set textContent(v) {
    if (this.tagName === '#text') this._text = String(v);
    else {
      this.children = [];
      if (v !== '' && v != null) {
        const t = new Node_();
        t._text = String(v);
        this.appendChild(t);
      }
    }
  }
  set innerHTML(_) {
    throw new Error('innerHTML must not be used');
  }
  appendChild(c) {
    c.parentNode = this;
    this.children.push(c);
    return c;
  }
  insertBefore(c, ref) {
    c.parentNode = this;
    const i = ref ? this.children.indexOf(ref) : -1;
    if (i < 0) this.children.push(c);
    else this.children.splice(i, 0, c);
    return c;
  }
  get nextSibling() {
    if (!this.parentNode) return null;
    const s = this.parentNode.children;
    return s[s.indexOf(this) + 1] || null;
  }
  setAttribute(k, v) {
    this.attrs[k] = String(v);
    if (k === 'id') this.id = String(v);
  }
  getAttribute(k) {
    return k in this.attrs ? this.attrs[k] : null;
  }
  removeAttribute(k) {
    delete this.attrs[k];
  }
  addEventListener(type, fn) {
    (this.listeners[type] = this.listeners[type] || []).push(fn);
  }
  dispatch(type, extra) {
    const ev = Object.assign({ type, preventDefault() {} }, extra || {});
    (this.listeners[type] || []).forEach((fn) => fn(ev));
  }
  getElementsByTagName(tag) {
    const out = [];
    const walk = (n) =>
      n.children.forEach((c) => {
        if (c.tagName === tag.toUpperCase()) out.push(c);
        walk(c);
      });
    walk(this);
    return out;
  }
}

const body = new Node_('body');
const input = body.appendChild(new Node_('input'));
input.id = 'search-input';
input.value = '';
const results = body.appendChild(new Node_('div'));
results.id = 'search-results';
results.setAttribute('aria-live', 'polite');
// The page's own `data-collections` (from `search_collections`), unless the
// test overrides it.
const fromPage = html.match(/id="search-input"[^>]*data-collections="([^"]*)"/);
const dataCollections = process.env.SEARCH_COLLECTIONS || (fromPage && fromPage[1]);
if (dataCollections) input.setAttribute('data-collections', dataCollections);

let fetched = [];
const allFetched = [];
global.document = {
  getElementById: (id) => (id === 'search-input' ? input : id === 'search-results' ? results : null),
  createElement: (t) => new Node_(t),
  createTextNode: (t) => {
    const n = new Node_();
    n._text = String(t);
    return n;
  },
};
global.location = { href: '' };
const cacheModes = {};
global.fetch = (url, opts) => {
  const clean = url.split('?')[0];
  cacheModes[clean] = (opts && opts.cache) || 'default';
  const file = path.join(dist, decodeURIComponent(clean));
  fetched.push(clean);
  allFetched.push(clean);
  if (!fs.existsSync(file)) return Promise.resolve({ ok: false, status: 404 });
  const text = fs.readFileSync(file, 'utf8');
  fetched.bytes = (fetched.bytes || 0) + Buffer.byteLength(text);
  return Promise.resolve({ ok: true, json: () => Promise.resolve(JSON.parse(text)) });
};

new Function(source)();

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

function describe() {
  return results.children
    .filter((c) => c.tagName === 'A')
    .map((a) => {
      const strong = a.children.find((c) => c.tagName === 'STRONG');
      const snippet = a.children.find((c) => c.className === 'result-snippet');
      const marks = a.getElementsByTagName('mark').map((m) => m.textContent);
      return {
        href: a.href,
        title: strong ? strong.textContent : '',
        snippet: snippet ? snippet.textContent : '',
        marks,
        active: a.classList.contains('active'),
      };
    });
}

(async () => {
  const queries = JSON.parse(fs.readFileSync(queriesFile, 'utf8'));
  const out = [];
  input.dispatch('focus');
  await sleep(20);
  for (const q of queries) {
    fetched = [];
    input.value = q.q;
    input.dispatch('input');
    await sleep(250);
    for (const key of q.keys || []) input.dispatch('keydown', { key });
    const status = results.parentNode.children.find((c) => c.getAttribute('role') === 'status');
    out.push({
      q: q.q,
      results: describe(),
      noResults: results.children.some((c) => c.className === 'no-results'),
      status: status ? status.textContent : null,
      activedescendant: input.getAttribute('aria-activedescendant'),
      location: global.location.href,
      fetched: fetched.slice(),
      allFetched: allFetched.slice(),
      cacheModes: Object.assign({}, cacheModes),
      bytes: fetched.bytes || 0,
    });
    global.location.href = '';
  }
  console.log(JSON.stringify(out));
})();
