// Renders a program's manual from its own repository: programs.json gives each
// program's `docs` folder, which holds index.json (the table of contents) and
// one Markdown file per page. URL: docs.html?p=<program id>#<page slug>

const params = new URLSearchParams(location.search);
const $ = (id) => document.getElementById(id);
const els = {
  select: $("program-select"),
  search: $("docs-search"),
  nav: $("docs-nav"),
  doc: $("doc"),
  pager: $("doc-pager"),
  toc: $("docs-toc"),
  edit: $("doc-edit"),
  side: $("docs-side"),
};

let program = null; // the programs.json entry
let manual = null; // its index.json
let pages = []; // flat list of { slug, title, section }
const cache = new Map(); // slug -> markdown

function rawBase(p) {
  // ?docsBase=<url> previews a manual from elsewhere (for example a local server).
  const preview = params.get("docsBase");
  if (preview) return preview.replace(/\/?$/, "/");
  const repo = p.repo.replace(/\/$/, "").replace("https://github.com/", "");
  return `https://raw.githubusercontent.com/${repo}/main/${p.docs.replace(/^\/|\/$/g, "")}/`;
}

function el(tag, attrs = {}, ...children) {
  const node = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs)) {
    if (k === "class") node.className = v;
    else node.setAttribute(k, v);
  }
  for (const c of children) if (c != null) node.append(c);
  return node;
}

async function load(slug) {
  if (!cache.has(slug)) {
    const res = await fetch(rawBase(program) + `${slug}.md`, { cache: "no-cache" });
    if (!res.ok) throw new Error(`This page isn't available (${res.status}).`);
    cache.set(slug, await res.text());
  }
  return cache.get(slug);
}

function slugify(text) {
  return text.toLowerCase().replace(/[^\w\s-]/g, "").trim().replace(/\s+/g, "-");
}

// ------------------------------------------------------------ rendering

function renderNav(active) {
  els.nav.replaceChildren();
  for (const section of manual.sections) {
    els.nav.append(el("div", { class: "nav-section" }, section.title));
    for (const p of section.pages) {
      els.nav.append(el("a", { href: `#${p.slug}`, class: p.slug === active ? "active" : "" }, p.title));
    }
  }
}

async function show(slug) {
  const page = pages.find((p) => p.slug === slug) || pages[0];
  renderNav(page.slug);
  els.side.classList.remove("open");
  els.search.value = "";
  document.title = `${page.title} · ${program.name} manual`;
  els.edit.replaceChildren(
    el("a", { href: `${program.repo}/blob/main/${program.docs.replace(/^\/|\/$/g, "")}/${page.slug}.md` }, "Edit this page on GitHub"),
  );
  let md;
  try {
    md = await load(page.slug);
  } catch (e) {
    els.doc.replaceChildren(el("h1", {}, page.title), el("p", { class: "muted" }, e.message));
    els.toc.replaceChildren();
    els.pager.replaceChildren();
    return;
  }

  const html = DOMPurify.sanitize(marked.parse(md));
  els.doc.innerHTML = html;

  // Links between pages ("painting.md#brushes") stay inside the viewer; images
  // resolve against the docs folder.
  for (const a of els.doc.querySelectorAll("a[href]")) {
    const href = a.getAttribute("href");
    const m = href.match(/^([\w-]+)\.md(#[\w-]+)?$/);
    if (m) a.setAttribute("href", `#${m[1]}${m[2] ? `:${m[2].slice(1)}` : ""}`);
    else if (href.startsWith("#")) a.setAttribute("href", `#${page.slug}:${href.slice(1)}`);
    else if (/^https?:/.test(href)) a.setAttribute("rel", "noopener");
  }
  for (const img of els.doc.querySelectorAll("img[src]")) {
    const src = img.getAttribute("src");
    if (!/^(https?:|data:)/.test(src)) img.src = rawBase(program) + src;
    img.loading = "lazy";
  }

  // Heading anchors and the "On this page" list.
  const heads = [...els.doc.querySelectorAll("h2, h3")];
  for (const h of heads) h.id = slugify(h.textContent);
  els.toc.replaceChildren(
    ...(heads.length ? [el("h4", {}, "On this page")] : []),
    ...heads
      .filter((h) => h.tagName === "H2")
      .map((h) => el("a", { href: `#${page.slug}:${h.id}` }, h.textContent)),
  );

  // Previous / next.
  const i = pages.indexOf(page);
  els.pager.replaceChildren();
  if (pages[i - 1]) els.pager.append(el("a", { href: `#${pages[i - 1].slug}` }, el("small", {}, "Previous"), pages[i - 1].title));
  if (pages[i + 1]) els.pager.append(el("a", { class: "next", href: `#${pages[i + 1].slug}` }, el("small", {}, "Next"), pages[i + 1].title));
}

// "#painting:brushes" = page "painting", heading "brushes".
async function route() {
  const [slug, anchor] = decodeURIComponent(location.hash.slice(1)).split(":");
  const current = els.doc.dataset.slug;
  if (slug !== current || !slug) {
    await show(slug);
    els.doc.dataset.slug = slug || pages[0].slug;
    if (!anchor) window.scrollTo(0, 0);
  }
  if (anchor) document.getElementById(anchor)?.scrollIntoView();
}

// ------------------------------------------------------------ search

let searchToken = 0;
async function search(query) {
  const q = query.trim().toLowerCase();
  if (!q) {
    renderNav(els.doc.dataset.slug);
    return;
  }
  const token = ++searchToken;
  await Promise.all(pages.map((p) => load(p.slug).catch(() => "")));
  if (token !== searchToken) return;
  const hits = [];
  for (const p of pages) {
    const text = (cache.get(p.slug) || "").replace(/[#*_`>|]/g, " ").replace(/\s+/g, " ");
    const at = text.toLowerCase().indexOf(q);
    if (p.title.toLowerCase().includes(q) || at >= 0) {
      const start = Math.max(0, at - 40);
      const snippet = at >= 0 ? text.slice(start, at + q.length + 60) : "";
      hits.push({ p, snippet, at: at - start });
    }
  }
  els.nav.replaceChildren(
    hits.length ? el("div", { class: "nav-section" }, `${hits.length} result${hits.length > 1 ? "s" : ""}`) : el("p", { class: "muted" }, "No matches."),
    ...hits.map(({ p, snippet, at }) => {
      const small = el("small");
      if (snippet) {
        small.append((at > 0 ? "…" : "") + snippet.slice(0, at), el("mark", {}, snippet.slice(at, at + q.length)), snippet.slice(at + q.length) + "…");
      }
      return el("a", { class: "search-hit", href: `#${p.slug}` }, el("b", {}, p.title), small);
    }),
  );
}

// ------------------------------------------------------------ start

async function start() {
  let programs;
  try {
    programs = (await (await fetch("programs.json", { cache: "no-cache" })).json()).programs.filter((p) => p.docs);
  } catch {
    els.doc.replaceChildren(el("p", { class: "muted" }, "The program list couldn't be loaded."));
    return;
  }
  if (!programs.length) {
    els.doc.replaceChildren(el("p", { class: "muted" }, "No manuals yet."));
    return;
  }
  program = programs.find((p) => p.id === params.get("p")) || programs[0];
  for (const p of programs) {
    const opt = el("option", { value: p.id }, `${p.name} manual`);
    if (p === program) opt.selected = true;
    els.select.append(opt);
  }
  els.select.addEventListener("change", () => {
    location.href = `docs.html?p=${encodeURIComponent(els.select.value)}`;
  });

  try {
    manual = await (await fetch(rawBase(program) + "index.json", { cache: "no-cache" })).json();
  } catch {
    els.doc.replaceChildren(el("p", { class: "muted" }, `The ${program.name} manual couldn't be loaded.`));
    return;
  }
  pages = manual.sections.flatMap((s) => s.pages.map((p) => ({ ...p, section: s.title })));

  window.addEventListener("hashchange", route);
  els.search.addEventListener("input", () => search(els.search.value));
  $("docs-menu").addEventListener("click", () => els.side.classList.toggle("open"));
  route();
}

start();
