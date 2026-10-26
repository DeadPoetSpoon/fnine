(function () {
  "use strict";
  const app = document.getElementById("reader-app");
  const content = document.getElementById("chapter-text");
  if (!app || !content) return;
  const bookId = app.dataset.bookId;
  const chapter = Number(app.dataset.chapter);
  const initialPosition = Number(app.dataset.position) || 0;
  const status = document.getElementById("reader-status");
  const sidebar = document.getElementById("toc-sidebar");
  const tocButton = document.getElementById("toc-open-btn");
  const noteDialog = document.getElementById("endnote-dialog");
  const annotationDialog = document.getElementById("annotation-dialog");
  const annotateButton = document.getElementById("annotate-selection");
  const canonicalText = content.textContent;
  let restoring = true;
  let progressTimer;
  let selectionRange;
  let noteTrigger;
  let pendingProgress = null;
  let saving = false;
  let annotations = [];

  content.querySelectorAll(".fn-ref").forEach(function (reference) {
    const previous = reference.previousSibling;
    if (previous?.nodeType === Node.ELEMENT_NODE && previous.classList.contains("note-anchor")) {
      previous.append(reference);
      return;
    }
    if (previous?.nodeType !== Node.TEXT_NODE) return;
    const tail = previous.textContent.match(/\S[ \t]*$/u);
    if (!tail || tail.index + tail[0].length !== previous.textContent.length) return;
    const attached = previous.splitText(tail.index);
    const anchor = document.createElement("span");
    anchor.className = "note-anchor";
    reference.before(anchor);
    anchor.append(attached, reference);
  });

  function message(failed) {
    status.textContent = failed ? status.dataset.error : "";
    status.classList.toggle("save-error", failed);
    status.hidden = !failed;
  }

  let sidebarOpen = !window.matchMedia("(max-width: 767px)").matches;
  try {
    const stored = localStorage.getItem("fnine-sidebar");
    if (stored) sidebarOpen = stored === "visible";
  } catch (_) {}

  function setSidebar(open, focus) {
    sidebarOpen = open;
    sidebar.hidden = !open;
    sidebar.classList.toggle("open", open);
    tocButton.hidden = open;
    tocButton.style.display = open ? "none" : "inline-block";
    tocButton.setAttribute("aria-expanded", String(open));
    try { localStorage.setItem("fnine-sidebar", open ? "visible" : "hidden"); } catch (_) {}
    if (focus) {
      if (open) sidebar.querySelector("button").focus();
      else tocButton.focus();
    }
  }
  setSidebar(sidebarOpen, false);

  async function toggleFullscreen() {
    try {
      if (document.fullscreenElement) await document.exitFullscreen();
      else await document.documentElement.requestFullscreen();
    } catch (_) {}
  }

  document.addEventListener("click", function (event) {
    const action = event.target.closest("[data-action]")?.dataset.action;
    if (action === "hide-sidebar") setSidebar(false, true);
    if (action === "open-toc") setSidebar(true, true);
    if (action === "toggle-fullscreen") toggleFullscreen();
    if (action === "cancel-annotation") annotationDialog.close();
    const reference = event.target.closest("[data-note]");
    if (reference) {
      const template = document.getElementById(reference.dataset.note);
      if (!template) return;
      noteTrigger = reference;
      document.getElementById("endnote-body").replaceChildren(template.content.cloneNode(true));
      annotateButton.hidden = true;
      noteDialog.showModal();
    }
  });

  for (const dialog of [noteDialog, annotationDialog]) {
    dialog.addEventListener("click", function (event) {
      const bounds = dialog.getBoundingClientRect();
      if (event.target === dialog && (event.clientX < bounds.left || event.clientX > bounds.right || event.clientY < bounds.top || event.clientY > bounds.bottom)) dialog.close();
    });
  }
  noteDialog.addEventListener("close", function () { noteTrigger?.focus({ preventScroll: true }); });

  document.addEventListener("keydown", function (event) {
    if (noteDialog.open || annotationDialog.open) return;
    if (event.key === "Escape" && sidebarOpen) { setSidebar(false, true); return; }
    if (event.target.closest("input, textarea, select, button, a, [contenteditable]") || event.altKey || event.ctrlKey || event.metaKey || window.getSelection()?.toString()) return;
    const selector = event.key === "ArrowRight" ? ".reader-nav a.btn-primary" : event.key === "ArrowLeft" ? ".reader-nav a.btn-secondary" : null;
    if (selector) {
      const link = document.querySelector(selector);
      if (link) { event.preventDefault(); link.click(); }
    }
    if (event.key === "f") toggleFullscreen();
  });

  function textNodes() {
    const walker = document.createTreeWalker(content, NodeFilter.SHOW_TEXT);
    const nodes = [];
    while (walker.nextNode()) nodes.push(walker.currentNode);
    return nodes;
  }

  function offsetOf(node, offset) {
    const range = document.createRange();
    range.selectNodeContents(content);
    range.setEnd(node, offset);
    return range.toString().length;
  }

  function rangeAt(offset) {
    let remaining = Math.max(0, offset);
    for (const node of textNodes()) {
      if (remaining <= node.length) {
        const range = document.createRange();
        range.setStart(node, remaining);
        range.collapse(true);
        return range;
      }
      remaining -= node.length;
    }
    return null;
  }

  function positionAtViewport() {
    const bounds = content.getBoundingClientRect();
    const referenceY = window.innerHeight * 0.3;
    if (referenceY < bounds.top) return 0;
    if (referenceY > bounds.bottom || window.innerHeight + window.scrollY >= document.documentElement.scrollHeight - 2) return 1;
    const referenceX = bounds.left + Math.max(8, bounds.width * 0.1);
    let node;
    let offset;
    if (document.caretPositionFromPoint) {
      const caret = document.caretPositionFromPoint(referenceX, referenceY);
      node = caret?.offsetNode;
      offset = caret?.offset;
    } else if (document.caretRangeFromPoint) {
      const caret = document.caretRangeFromPoint(referenceX, referenceY);
      node = caret?.startContainer;
      offset = caret?.startOffset;
    }
    if (node && content.contains(node)) return offsetOf(node, offset) / Math.max(1, canonicalText.length);
    let lower = 0;
    let upper = canonicalText.length;
    while (lower < upper) {
      const middle = Math.floor((lower + upper) / 2);
      const bounds = rangeAt(middle)?.getClientRects()[0];
      if (!bounds || bounds.top < referenceY) lower = middle + 1;
      else upper = middle;
    }
    return lower / Math.max(1, canonicalText.length);
  }

  function progressPayload() {
    return { book_id: bookId, chapter: chapter, position: Math.max(0, Math.min(1, positionAtViewport())) };
  }

  async function drainProgress() {
    if (saving) return;
    saving = true;
    while (pendingProgress) {
      const payload = pendingProgress;
      pendingProgress = null;
      try {
        const response = await fetch("/api/progress", { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify(payload), keepalive: true });
        if (!response.ok) throw new Error("save failed");
        message(false);
      } catch (_) { message(true); }
    }
    saving = false;
  }

  function saveProgress() {
    if (restoring || noteDialog.open || annotationDialog.open) return;
    pendingProgress = progressPayload();
    drainProgress();
  }

  window.addEventListener("scroll", function () {
    if (restoring) return;
    clearTimeout(progressTimer);
    progressTimer = setTimeout(saveProgress, 500);
  }, { passive: true });
  document.addEventListener("visibilitychange", function () {
    if (document.visibilityState === "hidden") { clearTimeout(progressTimer); saveProgress(); }
  });
  window.addEventListener("pagehide", saveProgress);
  document.querySelectorAll("a").forEach(function (link) { link.addEventListener("click", saveProgress); });

  function restorePosition() {
    if (initialPosition > 0) {
      const range = rangeAt(Math.round(initialPosition * canonicalText.length));
      const bounds = range?.getClientRects()[0];
      if (bounds) window.scrollTo(0, window.scrollY + bounds.top - window.innerHeight * 0.3);
    }
    requestAnimationFrame(function () { restoring = false; saveProgress(); });
  }

  function renderHighlights() {
    content.querySelectorAll("mark").forEach(function (mark) { mark.replaceWith(...mark.childNodes); });
    content.normalize();
    let position = 0;
    const relevant = annotations.filter(function (annotation) { return annotation.chapter === chapter && !annotation.needs_review; });
    for (const node of textNodes()) {
      const length = node.length;
      const ranges = relevant.map(function (annotation) {
        return { start: Math.max(0, Math.round(annotation.position_start * canonicalText.length) - position), end: Math.min(length, Math.round(annotation.position_end * canonicalText.length) - position), color: annotation.color };
      }).filter(function (range) { return range.start < range.end && range.end > 0 && range.start < length; });
      const boundaries = Array.from(new Set([0, length, ...ranges.flatMap(function (range) { return [range.start, range.end]; })])).sort(function (left, right) { return left - right; });
      const fragment = document.createDocumentFragment();
      for (let index = 0; index < boundaries.length - 1; index++) {
        const start = boundaries[index];
        const end = boundaries[index + 1];
        const active = ranges.filter(function (range) { return range.start <= start && range.end >= end; });
        const text = document.createTextNode(node.data.slice(start, end));
        if (active.length) {
          const mark = document.createElement("mark");
          mark.style.backgroundColor = active[active.length - 1].color;
          mark.appendChild(text);
          fragment.appendChild(mark);
        } else fragment.appendChild(text);
      }
      node.replaceWith(fragment);
      position += length;
    }
  }

  function selectedRange() {
    const selection = window.getSelection();
    if (!selection?.rangeCount || selection.isCollapsed) return null;
    const range = selection.getRangeAt(0);
    if (!content.contains(range.commonAncestorContainer) || !selection.toString().trim()) return null;
    return range;
  }

  let selectionTimer;
  document.addEventListener("selectionchange", function () {
    if (annotationDialog.open || noteDialog.open) return;
    clearTimeout(selectionTimer);
    selectionTimer = setTimeout(function () {
      const range = selectedRange();
      if (range) selectionRange = range.cloneRange();
      annotateButton.hidden = !range;
    }, 250);
  });
  annotateButton.addEventListener("pointerdown", function (event) { event.preventDefault(); });
  annotateButton.addEventListener("click", function () {
    if (!selectionRange) return;
    document.getElementById("annotation-quote").textContent = selectionRange.toString();
    document.getElementById("annotation-note").value = "";
    annotateButton.hidden = true;
    annotationDialog.showModal();
  });
  document.getElementById("annotation-form").addEventListener("submit", async function (event) {
    event.preventDefault();
    if (!selectionRange) return;
    const submit = event.submitter;
    if (submit) submit.disabled = true;
    try {
      const payload = {
        chapter: chapter, selected_text: selectionRange.toString(), note: document.getElementById("annotation-note").value.trim() || null,
        position_start: offsetOf(selectionRange.startContainer, selectionRange.startOffset) / Math.max(1, canonicalText.length),
        position_end: offsetOf(selectionRange.endContainer, selectionRange.endOffset) / Math.max(1, canonicalText.length),
        color: document.getElementById("annotation-color").value,
      };
      const response = await fetch("/api/book/" + bookId + "/annotations", { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify(payload) });
      if (!response.ok) throw new Error("annotation save failed");
      annotations.push(await response.json());
      window.getSelection()?.removeAllRanges();
      annotationDialog.close();
      renderHighlights();
      selectionRange = null;
      message(false);
    } catch (_) { message(true); }
    finally { if (submit) submit.disabled = false; }
  });

  async function initialize() {
    try {
      const response = await fetch("/api/book/" + bookId + "/annotations");
      if (!response.ok) throw new Error("annotation load failed");
      annotations = await response.json();
      renderHighlights();
    } catch (_) { message(true); }
    if (document.fonts) {
      try { await document.fonts.ready; } catch (_) {}
    }
    requestAnimationFrame(function () { requestAnimationFrame(restorePosition); });
  }
  initialize();
})();
