#!/usr/bin/env bun
// Static documentation only: no compiler execution, proof checking, or remote fetches.
import { createHash } from 'node:crypto';
import { existsSync, mkdirSync, readFileSync, readdirSync, realpathSync, statSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

export const BUN_VERSION = '1.3.10'; // Same Bun as the reviewed nixpkgs input and Pages workflow.
const ROOT = fileURLToPath(new URL('../', import.meta.url));
const REPO = 'https://github.com/OnixResearch/noble-lang';
const ORIGIN = 'https://noble-lang.org';
const escape = text => String(text).replace(/[&<>"']/g, char => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[char]);
const digest = bytes => createHash('sha256').update(bytes).digest('hex');
const encodePath = file => file.split('/').map(encodeURIComponent).join('/');
const allowedTags = new Set('p h1 h2 h3 h4 h5 h6 a pre code strong em del blockquote ul ol li table thead tbody tr th td hr br input img'.split(' '));

export function repositoryFile(root, file) {
  if (path.posix.isAbsolute(file) || file === '..' || file.startsWith('../') || file.includes('\\') || /[\x00-\x1f\x7f]/.test(file)) {
    throw new Error(`path outside repository: ${file}`);
  }
  const absolute = realpathSync(path.join(root, file));
  if (absolute !== realpathSync(root) && !absolute.startsWith(realpathSync(root) + path.sep)) {
    throw new Error(`symlink outside repository: ${file}`);
  }
  return absolute;
}

function sourceUrl(root, file, revision, fragment = '') {
  const kind = statSync(repositoryFile(root, file)).isDirectory() ? 'tree' : 'blob';
  return `${REPO}/${kind}/${revision}/${encodePath(file)}${fragment}`;
}

export function resolveLink(href, { root, source, routes, revision, links }) {
  if (/[\x00-\x20\x7f\\]/.test(href) || href.startsWith('//')) throw new Error(`unsafe link in ${source}: ${href}`);
  if (/^[a-z][a-z0-9+.-]*:/i.test(href)) {
    if (!/^(https?:|mailto:)/i.test(href)) throw new Error(`unsafe link scheme in ${source}: ${href}`);
    return href;
  }
  const hash = href.indexOf('#');
  const fragment = hash < 0 ? '' : href.slice(hash);
  const beforeHash = hash < 0 ? href : href.slice(0, hash);
  const question = beforeHash.indexOf('?');
  const query = question < 0 ? '' : beforeHash.slice(question);
  const pathname = question < 0 ? beforeHash : beforeHash.slice(0, question);
  const decoded = decodeURIComponent(pathname);
  if (decoded.startsWith('/')) throw new Error(`absolute repository link in ${source}: ${href}`);
  const file = decoded ? path.posix.normalize(path.posix.join(path.posix.dirname(source), decoded)) : source;
  repositoryFile(root, file);
  const route = routes.get(file);
  if (route) {
    links.push({ source, target: route, anchor: decodeURIComponent(fragment.slice(1)) });
    return route + query + fragment;
  }
  return sourceUrl(root, file, revision, query + fragment);
}

export async function renderMarkdown(markdown, context) {
  // Use the pinned GFM parser, not regex substitutions over Markdown. Rewriting the
  // resulting elements leaves examples, nested lists, entities and code untouched.
  const html = Bun.markdown.html(markdown, { headings: { ids: true }, tagFilter: true });
  const ids = new Set();
  const rewritten = await new HTMLRewriter().on('*', {
    element(element) {
      const tag = element.tagName;
      if (!allowedTags.has(tag)) {
        element.removeAndKeepContent();
        return;
      }
      const attributes = Object.fromEntries(element.attributes);
      for (const name of Object.keys(attributes)) element.removeAttribute(name);
      if (/^h[1-6]$/.test(tag) && attributes.id) {
        if (ids.has(attributes.id)) throw new Error(`duplicate heading: ${attributes.id}`);
        ids.add(attributes.id);
        element.setAttribute('id', attributes.id);
      }
      if (tag === 'a' && attributes.href !== undefined) {
        element.setAttribute('href', resolveLink(attributes.href, context));
        if (attributes.title) element.setAttribute('title', attributes.title);
      }
      if (tag === 'img') {
        // Repository images are source links, not remote executable subresources.
        throw new Error(`images need an explicit local asset in ${context.source}`);
      }
      if (tag === 'code' && /^language-[a-z0-9_-]+$/i.test(attributes.class ?? '')) element.setAttribute('class', attributes.class);
      if (tag === 'ol' && /^\d+$/.test(attributes.start ?? '')) element.setAttribute('start', attributes.start);
      if ((tag === 'th' || tag === 'td') && /^(left|right|center)$/.test(attributes.align ?? '')) element.setAttribute('align', attributes.align);
      if (tag === 'input') {
        element.setAttribute('type', 'checkbox');
        element.setAttribute('disabled', '');
        if (attributes.checked !== undefined) element.setAttribute('checked', '');
        element.setAttribute('aria-label', attributes.checked === undefined ? 'Not completed' : 'Completed');
      }
    },
  }).transform(new Response(html)).text();
  const headings = [...rewritten.matchAll(/<h([23]) id="([^"]+)">([\s\S]*?)<\/h\1>/g)].map(([, level, id, label]) => ({ level, id, label }));
  return { html: rewritten, ids, headings };
}

function shell({ title, route, content, navigation, css, source }) {
  return `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>${escape(title)} · Noble</title>
<meta name="description" content="Noble: typed programs as immutable values, bounded WebAssembly implementation, and canonical draft specifications.">
<link rel="canonical" href="${ORIGIN}${route}">
<link rel="stylesheet" href="${css}">
</head>
<body>
<a class="skip" href="#main">Skip to content</a>
<header class="site-header"><a class="wordmark" href="/">Noble<span class="wordmark-dot">.</span></a>
<nav aria-label="Main"><a href="/spec/"${route.startsWith('/spec/') ? ' aria-current="page"' : ''}>Specification</a><a href="${REPO}/blob/main/README.md#what-works-today">Status</a><a href="${REPO}">GitHub ↗</a></nav></header>
<div class="draft-banner"><strong>Working draft · not stable</strong><span>Specifications include planned features, not blanket implementation or proof claims.</span></div>
${navigation ? `<div class="docs-layout"><aside class="sidebar" aria-label="Specification family">${navigation}</aside>` : ''}
<main id="main"${navigation ? ' class="document"' : ''}>${content}</main>${navigation ? '</div>' : ''}
<footer><a href="/">Noble</a><span>Draft language. Bounded profiles. Explicit claim boundaries.</span>${source ? `<a href="${source}">View canonical source ↗</a>` : `<a href="${REPO}/blob/main/specs/ROADMAP.md">Roadmap ↗</a>`}</footer>
</body>
</html>
`;
}

function familyNavigation(documents, current) {
  return `<a class="sidebar-title" href="/spec/">Canonical draft family</a><ul>${documents.map(doc => `<li><a href="${doc.route}"${current === doc.route ? ' aria-current="page"' : ''}>${escape(doc.label)}</a></li>`).join('')}</ul>`;
}

function readmeSection(readme, heading) {
  const start = readme.indexOf(`## ${heading}\n`);
  if (start < 0) throw new Error(`missing README section: ${heading}`);
  const lines = readme.slice(start + heading.length + 4).split('\n');
  let fence;
  const section = [];
  for (const line of lines) {
    // Match the existing Cairn adapter's fence discipline: shell comments in an
    // example are not Markdown headings, and a shorter fence does not close it.
    const delimiter = line.match(/^ {0,3}(`{3,}|~{3,})(.*)$/);
    if (fence) {
      if (delimiter && delimiter[1][0] === fence[0] && delimiter[1].length >= fence.length && !delimiter[2].trim()) fence = undefined;
    } else if (delimiter) fence = delimiter[1];
    else if (/^ {0,3}#{1,3} /.test(line)) break;
    section.push(line);
  }
  return section.join('\n').trim();
}

export async function buildSite({ root = ROOT, out, revision = 'main' }) {
  if (Bun.version !== BUN_VERSION) throw new Error(`site requires pinned Bun ${BUN_VERSION}, got ${Bun.version}`);
  if (!/^(?:[a-f0-9]{40}|main)$/.test(revision)) throw new Error('revision must be a full Git commit or main for a local preview');
  if (!out) throw new Error('build requires --out with a fresh output directory');
  out = path.resolve(out);
  if (existsSync(out) && readdirSync(out).length) throw new Error(`output must be empty: ${out}`);
  const family = JSON.parse(readFileSync(path.join(root, 'specs/spec-family.json'), 'utf8'));
  const routes = new Map();
  const documents = family.normative_documents.map(doc => {
    if (!/^[a-z][a-z0-9-]*$/.test(doc.cairn_id)) throw new Error('invalid canonical spec slug');
    const file = `.cairn/specs/${doc.cairn_id}/spec.md`;
    if (path.posix.normalize(path.posix.join('specs', doc.path)) !== file) throw new Error(`noncanonical spec: ${doc.path}`);
    const route = `/spec/${doc.cairn_id}/`;
    if (routes.has(file)) throw new Error(`duplicate canonical spec: ${file}`);
    routes.set(file, route);
    routes.set(`specs/${doc.compatibility_path}`, route);
    const markdown = readFileSync(repositoryFile(root, file), 'utf8');
    const title = markdown.match(/^# (.+)$/m)?.[1];
    if (!title) throw new Error(`missing spec title: ${file}`);
    const label = doc.cairn_id === 'language' ? 'Language' : doc.cairn_id === 'core-bootstrap' ? 'Core-Bootstrap' : doc.cairn_id.split('-').map(word => word[0].toUpperCase() + word.slice(1)).join(' ');
    return { file, route, title, label, markdown, id: doc.id };
  });
  // Keep the introductory order explicit; all other family members retain manifest order.
  documents.sort((a, b) => (a.label === 'Language' ? -2 : a.label === 'Core-Bootstrap' ? -1 : 0) - (b.label === 'Language' ? -2 : b.label === 'Core-Bootstrap' ? -1 : 0));
  const stylesheet = readFileSync(path.join(root, 'tools/site.css'));
  const css = `/assets/site-${digest(stylesheet).slice(0, 16)}.css`;
  const links = [];
  const pages = new Map();
  const sourceDigests = Object.fromEntries(documents.map(doc => [doc.file, digest(doc.markdown)]));
  for (const doc of documents) {
    const rendered = await renderMarkdown(doc.markdown, { root, source: doc.file, routes, revision, links });
    const toc = `<details class="toc"><summary>On this page</summary><ul>${rendered.headings.filter(h => h.level === '2').map(h => `<li><a href="#${h.id}">${h.label}</a></li>`).join('')}</ul></details>`;
    pages.set(doc.route, { ids: rendered.ids, html: shell({ title: doc.title, route: doc.route, css, source: sourceUrl(root, doc.file, revision), navigation: familyNavigation(documents, doc.route), content: `<div class="eyebrow">${escape(doc.id)} · ${escape(family.revision)}</div><p class="source-note">Canonical Cairn text, rendered directly. Historical status statements below are preserved; consult <a href="${sourceUrl(root, 'README.md', revision, '#what-works-today')}">current implementation scope</a> and the <a href="${sourceUrl(root, 'specs/STATUS.json', revision)}">status ledger</a> separately.</p>${toc}<article>${rendered.html}</article>` }) });
  }
  const readme = readFileSync(path.join(root, 'README.md'), 'utf8');
  sourceDigests['README.md'] = digest(readme);
  sourceDigests['specs/spec-family.json'] = digest(readFileSync(path.join(root, 'specs/spec-family.json')));
  const introEnd = readme.indexOf('\n## Programs as values\n');
  if (introEnd < 0) throw new Error('README introduction changed; review the landing-page extraction');
  const introMarkdown = readme.slice(0, introEnd).replace(/^# Noble\n/, '');
  const context = { root, source: 'README.md', routes, revision, links };
  const intro = await renderMarkdown(introMarkdown, context);
  const quickstart = await renderMarkdown(readmeSection(readme, 'Try it'), context);
  const content = `<section class="hero"><div class="eyebrow">A typed concatenative language → WebAssembly</div><h1>Programs.<br>As values.</h1><div class="intro">${intro.html}</div><div class="actions"><a class="button" href="/spec/language/">Read the draft spec <span>→</span></a><a class="button secondary" href="#try-it">Try the CLI ↓</a></div><pre class="hero-code"><code>40 2 quote [ + ] compose run</code></pre><p class="caption">Capture. Compose. Run. The result is <code>42</code>.</p></section>
<section class="scope" aria-labelledby="scope-title"><div><div class="eyebrow">What works today</div><h2 id="scope-title">A bounded core.<br>Not a blanket claim.</h2></div><div><p>Ordinary <code>run</code> and <code>session</code> select resource-free <strong>Core-Bootstrap</strong>: wrapping I64 arithmetic, data and stack operations, checked branches, first-class Programs, and nonrecursive definitions.</p><p>Modules, contracts, evidence companions, components, local services, and numeric graphs have separate opt-in or acceptance scopes. General libraries, recursive definitions, capability modules, portable package identities, and general distributed services remain open.</p><p>There is no browser playground here. Application contracts are optional; model proofs do not establish compiler implementation refinement.</p><p><a href="${sourceUrl(root, 'README.md', revision, '#what-works-today')}">Current scope and limitations ↗</a> · <a href="/spec/core-bootstrap/">Core-Bootstrap contract →</a></p></div></section>
<section class="try" id="try-it"><div class="eyebrow">Run it locally</div><h2>Try the CLI</h2>${quickstart.html}</section>
<section class="next"><h2>Read with the boundaries in view.</h2><div class="cards"><a href="/spec/"><strong>Draft specification →</strong><span>All ${documents.length} canonical Cairn family documents.</span></a><a href="${sourceUrl(root, 'specs/README.md', revision)}"><strong>Family & status ↗</strong><span>Implementation, retained acceptance, and open obligations.</span></a><a href="${sourceUrl(root, 'specs/ROADMAP.md', revision)}"><strong>Roadmap ↗</strong><span>Delivery order and scoped milestone evidence.</span></a></div></section>`;
  pages.set('/', { ids: new Set(['try-it', 'scope-title']), html: shell({ title: 'Typed programs as values', route: '/', css, content }) });
  // README fragment-only links refer to the full README, not to the distilled page.
  // resolveLink leaves those on GitHub because README is not in the route map.
  pages.set('/spec/', { ids: new Set(), html: shell({ title: 'Canonical draft specification', route: '/spec/', css, navigation: familyNavigation(documents), source: sourceUrl(root, '.cairn/specs', revision), content: `<div class="eyebrow">${escape(family.revision)}</div><h1>Canonical draft specification</h1><p class="lede">Start with the language contract, then the finite Core-Bootstrap subset.</p><p>This family contains designs and open requirements, not a stable language release. Rendering a document does not establish conformance, implementation completion, or a soundness proof.</p><p>The text comes directly from <code>.cairn/specs/</code>, never the generated compatibility views in <code>specs/*.md</code>. Source links identify the published revision. The <a href="${sourceUrl(root, 'specs/README.md', revision)}">family guide</a>, <a href="${sourceUrl(root, 'specs/STATUS.json', revision)}">status ledger</a>, and <a href="${sourceUrl(root, 'specs/ROADMAP.md', revision)}">roadmap</a> record separate implementation and evidence boundaries.</p><ol class="family-list">${documents.map(doc => `<li><a href="${doc.route}">${escape(doc.label)}</a><span>${escape(doc.id)}</span></li>`).join('')}</ol>` }) });
  for (const link of links) {
    if (!pages.has(link.target) || (link.anchor && !pages.get(link.target).ids.has(link.anchor))) {
      throw new Error(`unresolved published anchor in ${link.source}: ${link.target}#${link.anchor}`);
    }
  }
  const output = new Map();
  for (const [route, page] of pages) output.set(route.slice(1) + 'index.html', page.html);
  output.set('404.html', shell({ title: 'Page not found', route: '/404.html', css, content: '<section class="not-found"><div class="eyebrow">404</div><h1>Page not found.</h1><p>Return to the <a href="/">landing page</a> or <a href="/spec/">canonical draft family</a>.</p></section>' }));
  output.set(css.slice(1), stylesheet);
  // Pages _redirects cannot match hostnames. Import this supported account-level
  // Bulk Redirect list and enable its rule separately; a deploy cannot apply it.
  output.set('cloudflare-bulk-redirects.csv', 'www.noble-lang.org/,https://noble-lang.org/,301,TRUE,FALSE,TRUE,TRUE\n');
  output.set('_headers', `/*\n  Content-Security-Policy: default-src 'none'; style-src 'self'; img-src 'self'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'\n  X-Content-Type-Options: nosniff\n  Referrer-Policy: strict-origin-when-cross-origin\n  Permissions-Policy: camera=(), microphone=(), geolocation=()\n  Cache-Control: public, max-age=0, must-revalidate\n`);
  output.set('build.json', JSON.stringify({ schema: 'noble-static-site/v1', revision, bun: BUN_VERSION, canonical_sources: sourceDigests, routes: [...pages.keys()] }, null, 2) + '\n');
  for (const [file, bytes] of output) {
    const destination = path.join(out, file);
    mkdirSync(path.dirname(destination), { recursive: true });
    writeFileSync(destination, bytes);
  }
  return { pages: pages.size + 1, specifications: documents.length, checked_links: links.length, out };
}

function options(args) {
  const result = {};
  for (let i = 0; i < args.length; i += 2) {
    if (!args[i].startsWith('--') || !args[i + 1]) throw new Error('expected --name value');
    result[args[i].slice(2)] = args[i + 1];
  }
  return result;
}

function serveSite(directory, port) {
  const root = realpathSync(directory);
  const server = Bun.serve({
    hostname: '127.0.0.1', port,
    fetch(request) {
      if (!['GET', 'HEAD'].includes(request.method)) return new Response('Method not allowed', { status: 405 });
      try {
        const url = new URL(request.url);
        const relative = decodeURIComponent(url.pathname).slice(1);
        let file = repositoryFile(root, relative);
        if (statSync(file).isDirectory()) {
          if (!url.pathname.endsWith('/')) return Response.redirect(new URL(url.pathname + '/' + url.search, url), 308);
          file = path.join(file, 'index.html');
        }
        return new Response(request.method === 'HEAD' ? null : Bun.file(file), { headers: { 'Content-Type': Bun.file(file).type } });
      } catch {
        return new Response(request.method === 'HEAD' ? null : Bun.file(path.join(root, '404.html')), { status: 404, headers: { 'Content-Type': 'text/html' } });
      }
    },
  });
  console.log(`Noble static preview: ${server.url}`);
}

if (import.meta.main) {
  try {
    const [command, ...args] = process.argv.slice(2);
    const opts = options(args);
    if (command === 'build' && Object.keys(opts).every(key => ['out', 'revision'].includes(key))) {
      console.log(JSON.stringify(await buildSite({ out: opts.out, revision: opts.revision })));
    } else if (command === 'serve' && opts.dir && Object.keys(opts).every(key => ['dir', 'port'].includes(key))) {
      const port = Number(opts.port ?? 4173);
      if (!Number.isInteger(port) || port < 1 || port > 65535) throw new Error('invalid port');
      serveSite(opts.dir, port);
    } else throw new Error('usage: bun tools/site.mjs build --out FRESH_DIR [--revision COMMIT] | serve --dir OUTPUT [--port 4173]');
  } catch (error) {
    console.error(error.message);
    process.exitCode = 1;
  }
}
