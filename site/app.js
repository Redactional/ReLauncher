// Fills the page from programs.json (published with the site) and each
// program's latest GitHub release, so downloads are always the newest official files.

const LAUNCHER_REPO = "https://github.com/Redactional/ReLauncher";
const PLATFORMS = { windows: "Windows", macos: "macOS", linux: "Linux" };

// Which release files belong to which system, best choice first.
const KINDS = [
  { os: "windows", test: /\.(exe|msi)$/i, label: "Installer" },
  { os: "windows", test: /-windows-[^-]+\.zip$/i, label: "Portable .zip" },
  { os: "macos", test: /\.dmg$/i, label: "Disk image (.dmg)" },
  { os: "macos", test: /-macos-[^-]+\.zip$/i, label: "Portable .zip" },
  { os: "linux", test: /\.AppImage$/i, label: "AppImage" },
  { os: "linux", test: /\.deb$/i, label: ".deb package" },
  { os: "linux", test: /-linux-[^-]+\.zip$/i, label: "Portable .zip" },
];

function visitorOS() {
  const p = (navigator.userAgentData?.platform || navigator.platform || navigator.userAgent).toLowerCase();
  if (p.includes("win")) return "windows";
  if (p.includes("mac")) return "macos";
  if (p.includes("linux") || p.includes("x11")) return "linux";
  return null;
}

const OS = visitorOS();

function el(tag, attrs = {}, ...children) {
  const node = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs)) {
    if (k === "class") node.className = v;
    else node.setAttribute(k, v);
  }
  for (const c of children) if (c != null) node.append(c);
  return node;
}

function size(bytes) {
  return bytes > 1 << 20 ? `${(bytes / (1 << 20)).toFixed(0)} MB` : `${Math.max(1, Math.round(bytes / 1024))} KB`;
}

/** Files of a release grouped by system: { windows: [{label, name, url, size}], ... } */
function downloads(release) {
  const out = { windows: [], macos: [], linux: [] };
  for (const kind of KINDS) {
    for (const a of release.assets || []) {
      if (kind.test.test(a.name)) {
        out[kind.os].push({ label: kind.label, name: a.name, url: a.browser_download_url, size: a.size });
      }
    }
  }
  return out;
}

/** The latest release, cached for this browser session (GitHub limits anonymous requests). */
async function latest(repo) {
  const path = repo.replace(/\/$/, "").replace("https://github.com/", "");
  const key = `release:${path}`;
  const cached = sessionStorage.getItem(key);
  if (cached) return JSON.parse(cached);
  const res = await fetch(`https://api.github.com/repos/${path}/releases/latest`, {
    headers: { Accept: "application/vnd.github+json" },
  });
  if (!res.ok) throw new Error(res.status === 404 ? "No release yet" : "GitHub is busy, try again shortly");
  const release = await res.json();
  try { sessionStorage.setItem(key, JSON.stringify(release)); } catch { /* storage full or off */ }
  return release;
}

function dateText(iso) {
  return iso ? new Date(iso).toLocaleDateString(undefined, { year: "numeric", month: "short", day: "numeric" }) : "";
}

const DOWNLOAD_ICON = '<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M12 3v12m0 0-5-5m5 5 5-5M5 21h14"/></svg>';

function button(text, href, primary) {
  const a = el("a", { class: `btn small${primary ? " primary" : ""}`, href });
  a.innerHTML = DOWNLOAD_ICON;
  a.append(el("span", {}, text));
  return a;
}

// ---------------------------------------------------------------- ReLauncher

async function launcher() {
  const main = document.getElementById("launcher-download");
  const version = document.getElementById("launcher-version");
  const table = document.getElementById("launcher-platforms");
  try {
    const r = await latest(LAUNCHER_REPO);
    const files = downloads(r);
    version.textContent = `Version ${r.tag_name.replace(/^v/, "")} · ${dateText(r.published_at)}`;
    const mine = OS && files[OS][0];
    if (mine) {
      main.href = mine.url;
      main.querySelector("span").textContent = `Download for ${PLATFORMS[OS]}`;
    }
    table.replaceChildren(
      ...Object.entries(PLATFORMS).map(([os, name]) => {
        const head = el("h4", {}, name, os === OS ? el("span", { class: "here" }, "Your system") : null);
        const list = el("ul");
        for (const f of files[os]) {
          list.append(el("li", {}, el("a", { href: f.url }, el("span", {}, f.label), el("span", { class: "size" }, size(f.size)))));
        }
        if (!files[os].length) list.append(el("li", { class: "muted" }, "Not available yet"));
        return el("div", { class: "platform" }, head, list);
      }),
    );
  } catch (e) {
    version.textContent = e.message;
    table.replaceChildren(el("p", { class: "muted" }, "Downloads are on the ", el("a", { href: `${LAUNCHER_REPO}/releases/latest` }, "release page"), "."));
  }
}

// ---------------------------------------------------------------- programs

async function programs() {
  const grid = document.getElementById("program-grid");
  let list;
  try {
    list = (await (await fetch("programs.json", { cache: "no-cache" })).json()).programs;
  } catch {
    grid.replaceChildren(el("p", { class: "muted" }, "The program list couldn't be loaded."));
    return;
  }
  grid.replaceChildren(...list.map(programCard));
}

function programCard(p) {
  const version = el("div", { class: "version" }, "Checking the latest version…");
  const actions = el("div", { class: "actions" });
  const releaseLink = el("a", { href: `${p.repo}/releases/latest` }, "Release notes");
  const card = el(
    "article",
    { class: "card program" },
    el("div", { class: "program-head" }, el("img", { src: p.icon || "icon.png", alt: "", loading: "lazy" }), el("div", {}, el("h3", {}, p.name), version)),
    el("p", { class: "desc" }, p.description || ""),
    actions,
    el(
      "div",
      { class: "links" },
      p.docs ? el("a", { href: `docs.html?p=${encodeURIComponent(p.id)}` }, "Documentation") : null,
      releaseLink,
      el("a", { href: p.repo }, "Source code"),
      el("a", { href: `${p.repo}/issues/new/choose` }, "Report a bug"),
    ),
  );
  latest(p.repo)
    .then((r) => {
      version.textContent = `Version ${r.tag_name.replace(/^v/, "")} · ${dateText(r.published_at)}`;
      version.classList.add("ok");
      const files = downloads(r);
      const order = OS ? [OS, ...Object.keys(PLATFORMS).filter((o) => o !== OS)] : Object.keys(PLATFORMS);
      for (const os of order) {
        const f = files[os][0];
        if (f) actions.append(button(os === OS ? `Download for ${PLATFORMS[os]}` : PLATFORMS[os], f.url, os === OS));
      }
      if (!actions.childElementCount) actions.append(el("span", { class: "muted" }, "No downloads in this release yet."));
    })
    .catch((e) => {
      version.textContent = e.message;
      actions.append(button("Releases", `${p.repo}/releases`, false));
    });
  return card;
}

launcher();
programs();
