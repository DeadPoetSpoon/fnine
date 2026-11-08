(function () {
  "use strict";
  const panel = document.getElementById("bookmarks-panel");
  if (!panel) return;
  const bookId = panel.dataset.bookId;
  const list = document.getElementById("bookmarks-list");
  const status = document.getElementById("bookmarks-status");
  const previous = document.getElementById("bookmarks-prev");
  const next = document.getElementById("bookmarks-next");
  const dialog = document.getElementById("bookmark-dialog");
  const form = document.getElementById("bookmark-form");
  const input = document.getElementById("bookmark-name");
  const error = document.getElementById("bookmark-error");
  const reader = document.getElementById("reader-app");
  const preferences = new URLSearchParams({ lang: document.documentElement.lang, theme: document.documentElement.dataset.theme });
  const endpoint = "/api/book/" + bookId + "/bookmarks";
  let page = reader ? 1 : Number(new URL(location.href).searchParams.get("bookmark_page")) || 1;
  let pages = 1;
  let loadVersion = 0;
  let pending;
  let trigger;

  function openEditor(bookmark) {
    trigger = document.activeElement;
    pending = { ...bookmark };
    input.value = bookmark.name || "";
    error.textContent = "";
    document.getElementById("bookmark-title").textContent = bookmark.id ? panel.dataset.rename : panel.dataset.add;
    dialog.showModal();
  }

  function action(label, pathData, handler, destructive = false) {
    const button = document.createElement("button");
    button.type = "button";
    button.className = "btn-icon action-icon" + (destructive ? " action-delete" : "");
    button.setAttribute("aria-label", label);
    button.title = label;
    const icon = document.createElementNS("http://www.w3.org/2000/svg", "svg");
    icon.setAttribute("width", "16");
    icon.setAttribute("height", "16");
    icon.setAttribute("viewBox", "0 0 24 24");
    icon.setAttribute("fill", "none");
    icon.setAttribute("stroke", "currentColor");
    icon.setAttribute("stroke-width", "1.5");
    icon.setAttribute("stroke-linecap", "round");
    icon.setAttribute("stroke-linejoin", "round");
    icon.setAttribute("aria-hidden", "true");
    const path = document.createElementNS("http://www.w3.org/2000/svg", "path");
    path.setAttribute("d", pathData);
    icon.append(path);
    button.append(icon);
    button.addEventListener("click", handler);
    return button;
  }

  async function load(requested) {
    const version = ++loadVersion;
    previous.disabled = next.disabled = true;
    status.textContent = "";
    panel.setAttribute("aria-busy", "true");
    try {
      const response = await fetch(endpoint + "?" + preferences + "&page=" + requested);
      if (!response.ok) throw new Error("Unable to load bookmarks");
      const data = await response.json();
      if (version !== loadVersion) return;
      page = data.page;
      pages = data.pages;
      list.replaceChildren();
      for (const bookmark of data.items) {
        const item = document.createElement("li");
        item.className = "bookmark-item";
        const link = document.createElement("a");
        link.className = "bookmark-link";
        link.href = "/book/" + bookId + "/read/" + bookmark.chapter + "?" + preferences + "&pos=" + bookmark.position;
        link.textContent = bookmark.name;
        item.append(link);
        if (reader) {
          list.append(item);
          continue;
        }
        const footer = document.createElement("div");
        footer.className = "entry-footer bookmark-footer";
        const meta = document.createElement("span");
        meta.className = "annotation-meta";
        meta.textContent = Math.round(bookmark.position * 100) + "% · " + new Date(bookmark.created_at).toLocaleString(document.documentElement.lang);
        meta.title = meta.textContent;
        footer.append(meta, action(panel.dataset.rename, "m16 3 5 5-12 12H4v-5L16 3ZM14 5l5 5", function () { openEditor(bookmark); }), action(panel.dataset.delete, "M6 6l12 12M18 6 6 18", async function (event) {
          if (!confirm(panel.dataset.deleteConfirm)) return;
          const button = event.currentTarget;
          button.disabled = true;
          try {
            const response = await fetch(endpoint + "/" + bookmark.id, { method: "DELETE" });
            if (!response.ok) throw new Error("Unable to delete bookmark");
            await load(page);
          } catch (_) { status.textContent = panel.dataset.error; button.disabled = false; }
        }, true));
        item.append(footer);
        list.append(item);
      }
      status.textContent = data.total ? "" : panel.dataset.empty;
      document.getElementById("bookmarks-page").textContent = page + " / " + pages + " · " + panel.dataset.total + " " + data.total;
      if (!reader) {
        const url = new URL(location.href);
        url.searchParams.set("bookmark_page", page);
        history.replaceState(null, "", url);
        document.querySelectorAll(".annotations-section .pagination a, .annotation-delete-form").forEach(function (element) {
          const attribute = element.tagName === "FORM" ? "action" : "href";
          const target = new URL(element.getAttribute(attribute), location.href);
          target.searchParams.set("bookmark_page", page);
          element.setAttribute(attribute, target.pathname + target.search);
        });
      }
    } catch (_) { if (version === loadVersion) status.textContent = panel.dataset.error; }
    finally {
      if (version === loadVersion) {
        panel.removeAttribute("aria-busy");
        previous.disabled = page <= 1;
        next.disabled = page >= pages;
      }
    }
  }

  previous.addEventListener("click", function () { load(page - 1); });
  next.addEventListener("click", function () { load(page + 1); });
  document.getElementById("bookmark-cancel").addEventListener("click", function () { dialog.close(); });
  dialog.addEventListener("close", function () { trigger?.focus({ preventScroll: true }); });
  dialog.addEventListener("click", function (event) {
    const bounds = dialog.getBoundingClientRect();
    if (event.target === dialog && (event.clientX < bounds.left || event.clientX > bounds.right || event.clientY < bounds.top || event.clientY > bounds.bottom)) dialog.close();
  });
  form.addEventListener("submit", async function (event) {
    event.preventDefault();
    const button = form.querySelector("button[type=submit]");
    if (button.disabled || !pending) return;
    button.disabled = true;
    error.textContent = "";
    try {
      const response = await fetch(endpoint + (pending.id ? "/" + pending.id : "") + "?" + preferences, {
        method: pending.id ? "PATCH" : "POST", headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ chapter: pending.chapter, position: pending.position, name: input.value.trim() }),
      });
      if (!response.ok) throw new Error("Unable to save bookmark");
      const requested = pending.id ? page : 1;
      dialog.close();
      await load(requested);
    } catch (_) { error.textContent = panel.dataset.error; }
    finally { button.disabled = false; }
  });
  document.addEventListener("fnine-add-bookmark", function (event) { openEditor(event.detail); });
  if (reader) {
    const tocTab = document.getElementById("toc-tab");
    const bookmarksTab = document.getElementById("bookmarks-tab");
    function showBookmarks(show) {
      document.getElementById("toc-list").hidden = show;
      document.getElementById("reader-bookmarks").hidden = !show;
      tocTab.setAttribute("aria-pressed", String(!show));
      bookmarksTab.setAttribute("aria-pressed", String(show));
      if (show) load(page);
    }
    tocTab.addEventListener("click", function () { showBookmarks(false); });
    bookmarksTab.addEventListener("click", function () { showBookmarks(true); });
  } else load(page);
})();
