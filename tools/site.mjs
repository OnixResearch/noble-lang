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

const FAVICON = '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 32 32"><rect width="32" height="32" rx="7" fill="#0a5c54"/><circle cx="22" cy="22" r="4" fill="#d4f77e"/></svg>\n';
const APPENDIX_A = '# Appendix A \u2014 Worked examples (informative)';
const APPENDIX_B = '# Appendix B \u2014 Source traceability (informative)';

function fenceLine(line, fence) {
  const delimiter = line.match(/^ {0,3}(`{3,}|~{3,})(.*)$/);
  if (fence) {
    if (delimiter && delimiter[1][0] === fence[0] && delimiter[1].length >= fence.length && !delimiter[2].trim()) return { fence: undefined, searchable: false };
    return { fence, searchable: false };
  }
  if (delimiter) return { fence: delimiter[1], searchable: false };
  return { fence: undefined, searchable: true };
}

// Inclusive of the start line, exclusive of the end line. Markers inside fences do not count.
export function sliceBetween(markdown, start, end) {
  const lines = markdown.split('\n');
  let fence;
  let startLine = -1;
  let endLine = -1;
  for (let i = 0; i < lines.length; i++) {
    const step = fenceLine(lines[i], fence);
    fence = step.fence;
    if (!step.searchable) continue;
    if (startLine < 0 && lines[i].startsWith(start)) startLine = i;
    else if (startLine >= 0 && lines[i].startsWith(end)) {
      endLine = i;
      break;
    }
  }
  if (startLine < 0) throw new Error(`missing section start: ${start}`);
  if (endLine < 0) throw new Error(`missing section end: ${end}`);
  return lines.slice(startLine, endLine).join('\n').trim();
}

// Drop Cairn's generated scenario-link regions. Fenced examples that mention the markers stay literal.
export function stripScenarioRegions(markdown) {
  const start = '<!-- cairn:scenario-links:start -->';
  const end = '<!-- cairn:scenario-links:end -->';
  const lines = markdown.split('\n');
  const kept = [];
  let fence;
  let skipping = false;
  for (const line of lines) {
    const step = fenceLine(line, fence);
    if (skipping) {
      // A region can contain fences. Ignore them until the closer, which is never inside a fence we started before the region.
      if (!fence && line.trim() === end) skipping = false;
      continue;
    }
    fence = step.fence;
    if (!step.searchable) {
      kept.push(line);
      continue;
    }
    if (line.trim() === start) {
      skipping = true;
      continue;
    }
    kept.push(line);
  }
  if (skipping) throw new Error('unclosed Cairn scenario-link region');
  return kept.join('\n').trim();
}

function demoteHeadings(markdown) {
  let fence;
  return markdown.split('\n').map(line => {
    const step = fenceLine(line, fence);
    fence = step.fence;
    if (!step.searchable) return line;
    return line.replace(/^(#{1,5})( )/, (_, marks, space) => `${marks}#${space}`);
  }).join('\n');
}

function shell({ title, description = 'Noble is a statically typed concatenative language that compiles to WebAssembly. This site publishes the draft specification and the repository docs.', route, content, navigation, navigationLabel = 'Pages', css, source, narrow = false }) {
  const item = (href, label) => {
    const on = route === href || (href !== '/' && route.startsWith(href));
    return `<a href="${href}"${on ? ' aria-current="page"' : ''}>${label}</a>`;
  };
  const mainClass = navigation ? ' class="document"' : narrow ? ' class="narrow"' : '';
  return `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>${escape(title)} · Noble</title>
<meta name="description" content="${escape(description)}">
<meta name="theme-color" content="#f4f1e8" media="(prefers-color-scheme: light)">
<meta name="theme-color" content="#10181b" media="(prefers-color-scheme: dark)">
<link rel="icon" href="/favicon.svg" type="image/svg+xml">
<link rel="canonical" href="${ORIGIN}${route}">
<link rel="stylesheet" href="${css}">
</head>
<body>
<a class="skip" href="#main">Skip to content</a>
<header class="site-header"><a class="wordmark" href="/"${route === '/' ? ' aria-current="page"' : ''}>Noble<span class="wordmark-dot">.</span></a>
<nav aria-label="Main">${item('/guide/', 'Guide')}${item('/start/', 'Start')}${item('/docs/', 'Docs')}${item('/spec/', 'Specification')}<a href="${REPO}">GitHub</a></nav></header>
<div class="draft-banner"><strong>Working draft · not stable</strong><span>Specifications include planned features, not blanket implementation or proof claims.</span></div>
${navigation ? `<div class="docs-layout"><aside class="sidebar" aria-label="${escape(navigationLabel)}">${navigation}</aside>` : ''}
<main id="main"${mainClass}>${content}</main>${navigation ? '</div>' : ''}
<footer><a href="/">Noble</a><span>Draft language. Bounded profiles. Explicit claim boundaries.</span><a href="/playground/">Playground follow-up</a>${source ? `<a href="${source}">View source</a>` : `<a href="${REPO}">GitHub</a>`}</footer>
</body>
</html>
`;
}

function familyNavigation(documents, current) {
  return `<a class="sidebar-title" href="/spec/">Canonical draft family</a><ul>${documents.map(doc => `<li><a href="${doc.route}"${current === doc.route ? ' aria-current="page"' : ''}>${escape(doc.label)}</a></li>`).join('')}</ul>`;
}

function docsNavigation(notes, current) {
  return `<a class="sidebar-title" href="/docs/">Repository notes</a><ul>${notes.map(note => `<li><a href="${note.route}"${current === note.route ? ' aria-current="page"' : ''}>${escape(note.title)}</a></li>`).join('')}</ul>`;
}

function tocNavigation(headings, href, label, subheads = false) {
  const visible = headings.filter(heading => heading.level === '2' || (heading.level === '3' && (subheads || /^A\.\d/.test(heading.label.replace(/<[^>]+>/g, '')))));
  return `<a class="sidebar-title" href="${href}">${escape(label)}</a><ul>${visible.map(heading => `<li><a class="depth-${heading.level}" href="#${heading.id}">${heading.label}</a></li>`).join('')}</ul>`;
}

function mergeParts(parts) {
  const ids = new Set();
  const headings = [];
  for (const part of parts) {
    for (const id of part.ids) {
      if (ids.has(id)) throw new Error(`duplicate heading on one page: ${id}`);
      ids.add(id);
    }
    headings.push(...part.headings);
  }
  return { ids, headings, html: parts.map(part => part.html).join('\n') };
}

function documentationNotes(root) {
  const directory = repositoryFile(root, 'docs');
  if (!statSync(directory).isDirectory()) throw new Error('docs is not a directory');
  return readdirSync(directory).filter(name => name.endsWith('.md')).sort().map(name => {
    if (!/^[a-z0-9][a-z0-9.-]*\.md$/.test(name)) throw new Error(`unexpected docs filename: ${name}`);
    const file = `docs/${name}`;
    const markdown = readFileSync(repositoryFile(root, file), 'utf8');
    const title = markdown.match(/^# (.+)$/m)?.[1]?.trim();
    if (!title) throw new Error(`missing docs title: ${file}`);
    return { file, markdown, title, route: `/docs/${name.slice(0, -3)}/` };
  });
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
    pages.set(doc.route, { ids: rendered.ids, html: shell({ title: doc.title, description: `${doc.title}. Canonical Noble draft specification ${doc.id}.`, route: doc.route, css, source: sourceUrl(root, doc.file, revision), navigation: familyNavigation(documents, doc.route), navigationLabel: 'Specification family', content: `<div class="eyebrow">${escape(doc.id)} · ${escape(family.revision)}</div><p class="source-note">Canonical Cairn text, rendered directly. Historical status statements below are preserved; consult <a href="/start/">current implementation scope</a> and the <a href="${sourceUrl(root, 'specs/STATUS.json', revision)}">status ledger</a> separately. A shorter reading guide is on <a href="/guide/">the guide</a>.</p>${toc}<article>${rendered.html}</article>` }) });
  }
  const readme = readFileSync(path.join(root, 'README.md'), 'utf8');
  const language = documents.find(doc => doc.file === '.cairn/specs/language/spec.md');
  if (!language) throw new Error('language specification missing from the canonical family');
  sourceDigests['README.md'] = digest(readme);
  sourceDigests['specs/spec-family.json'] = digest(readFileSync(path.join(root, 'specs/spec-family.json')));
  const introEnd = readme.indexOf('\n## Programs as values\n');
  if (introEnd < 0) throw new Error('README introduction changed; review the landing-page extraction');
  const readmeContext = { root, source: 'README.md', routes, revision, links };
  const specContext = { root, source: language.file, routes, revision, links };
  const notes = documentationNotes(root);
  for (const note of notes) {
    routes.set(note.file, note.route);
    sourceDigests[note.file] = digest(note.markdown);
  }
  const intro = await renderMarkdown(readme.slice(0, introEnd).replace(/^# Noble\n+/, ''), readmeContext);
  const programs = await renderMarkdown(sliceBetween(readme, '## Programs as values', '## Try it'), readmeContext);
  const quickstart = await renderMarkdown(sliceBetween(readme, '## Try it', '### Editor transport reports'), readmeContext);
  const wasmtime = await renderMarkdown(sliceBetween(readme, '**Host-local Wasmtime source.**', '### Independent Mathlib comparison for implemented I64 arithmetic'), readmeContext);
  const sessions = await renderMarkdown(sliceBetween(readme, '### Persistent sessions', '### Experimental calculator (opt-in; acceptance not established)'), readmeContext);
  const works = await renderMarkdown(sliceBetween(readme, '## What works today', '## Verification'), readmeContext);
  const develop = await renderMarkdown(sliceBetween(readme, '## Working on Noble', '### Building and publishing the draft website'), readmeContext);
  const syntax = await renderMarkdown(stripScenarioRegions(sliceBetween(language.markdown, '## 3. Expression syntax and source units', '## 4. Values, stacks, and type notation')), specContext);
  const values = await renderMarkdown(stripScenarioRegions(sliceBetween(language.markdown, '## 4. Values, stacks, and type notation', '## 6. Program construction and execution')), specContext);
  const construction = await renderMarkdown(stripScenarioRegions(sliceBetween(language.markdown, '## 6. Program construction and execution', '## 7. Stack operations, data, and control')), specContext);
  const examples = await renderMarkdown(stripScenarioRegions(sliceBetween(language.markdown, '## A.1 Construction is not invocation', '## A.3 Runtime capture')), specContext);
  const appendix = await renderMarkdown(demoteHeadings(stripScenarioRegions(sliceBetween(language.markdown, APPENDIX_A, APPENDIX_B))), specContext);
  const specPage = pages.get('/spec/language/');
  const excerptNote = part => {
    const id = part.headings[0]?.id;
    if (!id || !specPage.ids.has(id)) throw new Error(`guide excerpt heading is not on the language spec page: ${id ?? '(none)'}`);
    links.push({ source: language.file, target: '/spec/language/', anchor: id });
    return `<p class="source-note">Excerpt from the <a href="/spec/language/#${encodeURIComponent(id)}">language specification</a>. Wording unchanged.</p>`;
  };
  const readmeNote = `<p class="source-note">From the <a href="${sourceUrl(root, 'README.md', revision, '#programs-as-values')}">README</a>. Wording unchanged.</p>`;
  const homeParts = mergeParts([programs, examples]);
  const worksId = works.headings.find(heading => heading.level === '2')?.id;
  if (!worksId) throw new Error('missing What works today heading');
  links.push({ source: 'README.md', target: '/start/', anchor: worksId });
  const home = `<section class="hero"><div class="eyebrow">A typed concatenative language, compiled to WebAssembly</div><h1>Programs.<br>As values.</h1><div class="intro">${intro.html}</div><div class="actions"><a class="button" href="/guide/">Read the guide</a><a class="button secondary" href="/start/">Install the CLI</a><a class="button secondary" href="/spec/language/">Draft spec</a></div><pre class="hero-code"><code>40 2 quote [ + ] compose run</code></pre><p class="caption">README example. A successful run reports one I64 value of 42.</p></section>
<section class="explain">${readmeNote}${programs.html}</section>
<section class="examples">${excerptNote(examples)}${examples.html}</section>
<section class="scope" aria-labelledby="scope-title"><div><div class="eyebrow">What works today</div><h2 id="scope-title">A bounded core.</h2></div><div><p>Ordinary <code>run</code> and <code>session</code> use the resource-free <strong>Core-Bootstrap</strong> profile. Their host words are <code>test.emit</code> and <code>test.abort</code>. Resources and component features use separate entry points.</p><p>Recursive definitions, local bindings, general libraries and capability modules, portable package identities, and general distributed services remain open. Application contracts are optional. Model proofs do not establish compiler implementation refinement.</p><p><a href="/start/#${encodeURIComponent(worksId)}">Scope table from the README</a> · <a href="/spec/core-bootstrap/">Core-Bootstrap contract</a> · <a href="/playground/">No browser playground</a></p></div></section>
<section class="next"><h2>Read the draft with the boundaries in view.</h2><div class="cards"><a href="/guide/"><strong>Language guide</strong><span>README and specification sections, wording unchanged.</span></a><a href="/start/"><strong>Install the CLI</strong><span>Nix build, one session, and the current scope table.</span></a><a href="/spec/"><strong>Draft specification</strong><span>All ${documents.length} canonical Cairn documents.</span></a><a href="/docs/"><strong>Design notes</strong><span>The Markdown files in <code>docs/</code>.</span></a></div></section>`;
  homeParts.ids.add('scope-title');
  pages.set('/', { ids: homeParts.ids, html: shell({ title: 'Typed programs as values', description: 'Noble is a statically typed concatenative language that compiles to WebAssembly. Programs are immutable values with stack types and host effects.', route: '/', css, content: home }) });
  const startParts = mergeParts([quickstart, wasmtime, sessions, works, develop]);
  pages.set('/start/', { ids: startParts.ids, html: shell({ title: 'Getting started', description: 'Install Noble from the repository README and see the current bounded scope.', route: '/start/', css, navigation: tocNavigation(startParts.headings, '/start/', 'On this page', true), navigationLabel: 'On this page', source: sourceUrl(root, 'README.md', revision), content: `<div class="eyebrow">From the README</div><h1>Getting started</h1><p class="lede">These sections are the repository README: how to build the CLI, the host Wasmtime prerequisite, one session, what works today, and how to work on the compiler. Editor reports, numeric graphs, and the rest of the README stay in the <a href="${sourceUrl(root, 'README.md', revision)}">repository</a>.</p><article>${startParts.html}</article>` }) });
  const guidePieces = [
    { note: readmeNote, part: programs },
    { note: excerptNote(syntax), part: syntax },
    { note: excerptNote(values), part: values },
    { note: excerptNote(construction), part: construction },
    { note: excerptNote(appendix), part: appendix },
  ];
  const guideParts = mergeParts(guidePieces.map(piece => piece.part));
  const guideBody = guidePieces.map(piece => `${piece.note}\n${piece.part.html}`).join('\n');
  pages.set('/guide/', { ids: guideParts.ids, html: shell({ title: 'Language guide', description: 'A Noble language guide extracted from the README and the draft language specification.', route: '/guide/', css, navigation: tocNavigation(guideParts.headings, '/guide/', 'On this page'), navigationLabel: 'On this page', source: sourceUrl(root, language.file, revision), content: `<div class="eyebrow">Reading guide · not a second specification</div><h1>Language guide</h1><p class="lede">Assembled from the README and from SPEC-0001. The words in each section are those sources. Cairn scenario-link regions are omitted here and remain on the <a href="/spec/language/">specification page</a>. Where the specification describes a planned or illustrative feature, that status is the specification's own.</p><article>${guideBody}</article>` }) });
  for (const note of notes) {
    const rendered = await renderMarkdown(note.markdown, { root, source: note.file, routes, revision, links });
    const toc = `<details class="toc"><summary>On this page</summary><ul>${rendered.headings.filter(heading => heading.level === '2').map(heading => `<li><a href="#${heading.id}">${heading.label}</a></li>`).join('')}</ul></details>`;
    pages.set(note.route, { ids: rendered.ids, html: shell({ title: note.title, description: `${note.title}. A Noble repository design note from ${note.file}.`, route: note.route, css, navigation: docsNavigation(notes, note.route), navigationLabel: 'Repository notes', source: sourceUrl(root, note.file, revision), content: `<div class="eyebrow">Repository design note</div><p class="source-note">Rendered from <code>${escape(note.file)}</code>. This note is not the language specification.</p>${toc}<article>${rendered.html}</article>` }) });
  }
  // README fragment-only links refer to the full README, not to a distilled page.
  // resolveLink leaves those on GitHub because README is not in the route map.
  pages.set('/spec/', { ids: new Set(), html: shell({ title: 'Canonical draft specification', description: 'The canonical Noble draft specification family, rendered from .cairn/specs.', route: '/spec/', css, navigation: familyNavigation(documents), navigationLabel: 'Specification family', source: sourceUrl(root, '.cairn/specs', revision), content: `<div class="eyebrow">${escape(family.revision)}</div><h1>Canonical draft specification</h1><p class="lede">Start with the <a href="/spec/language/">language contract</a>, then the finite <a href="/spec/core-bootstrap/">Core-Bootstrap</a> subset. For a shorter path through the same words, use the <a href="/guide/">language guide</a>.</p><p>This family contains designs and open requirements, not a stable language release. Rendering a document does not establish conformance, implementation completion, or a soundness proof.</p><p>The text comes directly from <code>.cairn/specs/</code>, never the generated compatibility views in <code>specs/*.md</code>. Source links identify the published revision. The <a href="${sourceUrl(root, 'specs/README.md', revision)}">family guide</a>, <a href="${sourceUrl(root, 'specs/STATUS.json', revision)}">status ledger</a>, and <a href="${sourceUrl(root, 'specs/ROADMAP.md', revision)}">roadmap</a> record separate implementation and evidence boundaries.</p><ol class="family-list">${documents.map(doc => `<li><a href="${doc.route}">${escape(doc.label)}</a><span>${escape(doc.id)}</span></li>`).join('')}</ol>` }) });
  pages.set('/docs/', { ids: new Set(), html: shell({ title: 'Design notes', description: 'Design notes from the Noble docs directory. They are not the language specification.', route: '/docs/', css, navigation: docsNavigation(notes), navigationLabel: 'Repository notes', source: sourceUrl(root, 'docs', revision), content: `<div class="eyebrow">docs/</div><h1>Design notes</h1><p class="lede">Every Markdown file under <code>docs/</code>, rendered from this revision. These notes are not the canonical specification. Start with the <a href="/guide/">language guide</a> and the <a href="/spec/language/">language specification</a>.</p>${notes.length ? `<ol class="family-list">${notes.map(note => `<li><a href="${note.route}">${escape(note.title)}</a><span>${escape(note.file)}</span></li>`).join('')}</ol>` : '<p>This revision has no Markdown files in <code>docs/</code>.</p>'}` }) });
  const repl = sourceUrl(root, 'README.md', revision, '#guarded-live-repl-opt-in-partial');
  pages.set('/playground/', { ids: new Set(), html: shell({ title: 'Browser playground', description: 'noble-lang.org does not compile or run Noble in the browser. A playground is later work.', route: '/playground/', css, narrow: true, source: `${REPO}/issues/137`, content: `<div class="eyebrow">Follow-up · not in this deploy</div><h1>No browser playground</h1><p class="lede">This site does not compile or run Noble in the browser.</p><p>The compiler is the native <code>noble</code> command. A <a href="${sourceUrl(root, 'README.md', revision, '#try-it')}">run</a> uses the pinned Node/V8, wasm-tools, and Binaryen paths in <a href="${sourceUrl(root, 'crates/noble-cli/src/core/runtime/config.json', revision)}">the CLI configuration</a>. Component routes also need the host Wasmtime source named in the flake. This repository does not publish a WebAssembly build of that compiler.</p><p>Programs compile <em>to</em> WebAssembly. That artifact is the guest, not a compiler a page can load.</p><p>A <a href="${repl}">guarded live path</a> exists as a resident Node/V8 process with in-process Wasm emission. It is not a static page, and it does not make unrestricted host effects available here.</p><p>Follow-up, recorded as phase 3 of <a href="${REPO}/issues/137">issue #137</a>: a later playground could run bounded Core-Bootstrap examples as Wasm, without implying that the live REPL or unrestricted host effects are available. That needs a separate bounded compiler build. It is not part of this site.</p><p><a href="/start/">Install the CLI</a> · <a href="/spec/core-bootstrap/">Core-Bootstrap contract</a></p>` }) });
  for (const link of links) {
    if (!pages.has(link.target) || (link.anchor && !pages.get(link.target).ids.has(link.anchor))) {
      throw new Error(`unresolved published anchor in ${link.source}: ${link.target}#${link.anchor}`);
    }
  }
  const output = new Map();
  for (const [route, page] of pages) output.set(route.slice(1) + 'index.html', page.html);
  output.set('404.html', shell({ title: 'Page not found', route: '/404.html', css, narrow: true, content: '<section class="not-found"><div class="eyebrow">404</div><h1>Page not found.</h1><p>Return to the <a href="/">landing page</a>, the <a href="/guide/">language guide</a>, or the <a href="/spec/">draft specification</a>.</p></section>' }));
  output.set(css.slice(1), stylesheet);
  output.set('favicon.svg', FAVICON);
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
