// Reader UI controller
(function () {
  // ── Sidebar ────────────────────────────────────────
  var sidebarHidden = localStorage.getItem("fnine-sidebar") === "hidden";

  function applySidebar() {
    var sidebar = document.getElementById("toc-sidebar");
    var openBtn = document.getElementById("toc-open-btn");
    if (sidebarHidden) {
      if (sidebar) sidebar.style.display = "none";
      if (openBtn) openBtn.style.display = "inline-block";
    } else {
      if (sidebar) sidebar.style.display = "";
      if (openBtn) openBtn.style.display = "none";
    }
  }

  function hideSidebar() {
    sidebarHidden = true;
    localStorage.setItem("fnine-sidebar", "hidden");
    applySidebar();
  }

  function openToc() {
    sidebarHidden = false;
    localStorage.setItem("fnine-sidebar", "visible");
    applySidebar();
  }

  // ── Keyboard navigation ────────────────────────────
  document.addEventListener("keydown", function (e) {
    if (e.target.tagName === "INPUT" || e.target.tagName === "TEXTAREA") return;
    if (e.key === "ArrowRight") {
      var next = document.querySelector(".reader-nav a.btn-primary");
      if (next) next.click();
    } else if (e.key === "ArrowLeft") {
      var prev = document.querySelector(".reader-nav a.btn-secondary");
      if (prev) prev.click();
    }
  });

  // ── Delegate toolbar clicks ────────────────────────
  document.addEventListener("click", function (e) {
    var el = e.target;
    if (el.nodeType === 3) el = el.parentElement;
    if (!el) return;
    var btn = el.closest("button");
    if (!btn) return;

    if (btn.getAttribute("data-action") === "hide-sidebar") {
      hideSidebar();
    }

    if (btn.getAttribute("data-action") === "open-toc") {
      openToc();
    }
  });

  // ── Init ───────────────────────────────────────────
  applySidebar();

  // ── Progress auto-save (character-offset based for cross-device consistency) ──
  var progressTimer = null;
  var pageMeta = document.getElementById("reader-app");
  var bookId = pageMeta ? pageMeta.dataset.bookId : null;
  var chapter = pageMeta ? parseInt(pageMeta.dataset.chapter) : 0;
  var initPos = pageMeta ? parseFloat(pageMeta.dataset.position) || 0 : 0;
  var REFERENCE_Y = 0.3; // reference point 30% from top of viewport

  // Get character offset within contentEl at the reference viewport point
  function getCharOffsetAtRef(contentEl) {
    var rect = contentEl.getBoundingClientRect();
    var x = rect.left + Math.max(10, rect.width * 0.1);
    var y = window.innerHeight * REFERENCE_Y;
    // Reference point above or below content → clamp to bounds
    if (y < rect.top) return 0;
    if (y > rect.bottom) return contentEl.textContent.length;

    if (!document.caretRangeFromPoint) return null;
    var range;
    try {
      range = document.caretRangeFromPoint(x, y);
    } catch (e) {
      return null;
    }
    if (!range || !contentEl.contains(range.startContainer)) return null;

    var walker = document.createTreeWalker(contentEl, NodeFilter.SHOW_TEXT);
    var offset = 0;
    while (walker.nextNode()) {
      if (walker.currentNode === range.startContainer) {
        return offset + range.startOffset;
      }
      offset += walker.currentNode.textContent.length;
    }
    return offset;
  }

  // Scroll so that charOffset within contentEl appears at the reference point
  function scrollToCharOffset(contentEl, charOffset) {
    var walker = document.createTreeWalker(contentEl, NodeFilter.SHOW_TEXT);
    var pos = 0;
    while (walker.nextNode()) {
      var len = walker.currentNode.textContent.length;
      if (pos + len >= charOffset) {
        var range = document.createRange();
        range.setStart(walker.currentNode, Math.min(charOffset - pos, len));
        range.collapse(true);
        var rect = range.getClientRects()[0];
        if (rect) {
          window.scrollTo(
            0,
            window.scrollY + rect.top - window.innerHeight * REFERENCE_Y,
          );
        }
        return;
      }
      pos += len;
    }
  }

  // Restore scroll position (character-offset based)
  if (initPos > 0) {
    var contentEl = document.getElementById("chapter-content");
    if (contentEl) {
      var totalChars = contentEl.textContent.length;
      if (totalChars > 0) {
        scrollToCharOffset(contentEl, Math.round(initPos * totalChars));
      }
    }
  }

  function saveProgress() {
    if (!bookId) return;
    var contentEl = document.getElementById("chapter-content");
    if (!contentEl) return;

    var totalChars = contentEl.textContent.length;
    if (totalChars === 0) return;

    // Primary: character-offset based (consistent across devices)
    var charOffset = getCharOffsetAtRef(contentEl);
    var position;
    if (charOffset !== null) {
      position = Math.min(1, Math.max(0, charOffset / totalChars));
    } else {
      // Fallback: scroll percentage (when caretRangeFromPoint unavailable)
      var scrollPos = window.scrollY;
      var docHeight =
        document.documentElement.scrollHeight - window.innerHeight;
      position =
        docHeight > 0 ? Math.min(1, Math.max(0, scrollPos / docHeight)) : 0;
    }

    fetch("/api/progress", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        book_id: bookId,
        chapter: chapter,
        position: Math.round(position * 1000) / 1000,
      }),
    }).catch(function () {});
  }

  window.addEventListener("scroll", function () {
    clearTimeout(progressTimer);
    progressTimer = setTimeout(saveProgress, 500);
  });

  // ── Text selection annotation ──────────────────────
  var annotPopup = document.createElement("div");
  annotPopup.className = "annot-popup";
  annotPopup.id = "annot-popup";
  annotPopup.innerHTML =
    '<div id="annot-btns">' +
    '<button class="annot-btn" data-action="start-note">标注</button>' +
    '<button class="annot-btn" data-action="cancel">取消</button>' +
    "</div>" +
    '<div id="annot-form" style="display:none">' +
    '<textarea class="annot-textarea" id="annot-note" placeholder="写点想法..."></textarea>' +
    "<div>" +
    '<button class="annot-btn" data-action="confirm">确定</button>' +
    '<button class="annot-btn" data-action="cancel">取消</button>' +
    "</div>" +
    "</div>";
  document.body.appendChild(annotPopup);

  var selection = null;
  var selectRange = null;

  function showAnnotPopup(rect) {
    document.getElementById("annot-btns").style.display = "";
    document.getElementById("annot-form").style.display = "none";
    annotPopup.classList.add("visible");
  }

  function showAnnotForm() {
    document.getElementById("annot-btns").style.display = "none";
    document.getElementById("annot-form").style.display = "";
    annotPopup.classList.add("visible");
    setTimeout(function () {
      var ta = document.getElementById("annot-note");
      if (ta) ta.focus();
    }, 50);
  }

  document.addEventListener("mouseup", function (e) {
    // Don't interfere with clicks on the annotation popup itself
    if (e.target.closest("#annot-popup")) return;

    setTimeout(function () {
      var sel = window.getSelection();
      var text = sel ? sel.toString().trim() : "";
      if (text.length < 2) {
        annotPopup.classList.remove("visible");
        return;
      }
      selection = sel;
      selectRange = sel.getRangeAt(0).cloneRange();
      var rect = selectRange.getBoundingClientRect();
      showAnnotPopup(rect);
    }, 10);
  });

  annotPopup.addEventListener("click", function (e) {
    var btn = e.target.closest("button");
    if (!btn) return;
    var action = btn.getAttribute("data-action");

    if (action === "start-note") {
      showAnnotForm();
      return;
    }

    if (action === "confirm" && selectRange && bookId) {
      var text = selectRange.toString().trim();
      if (!text) return;

      var noteEl = document.getElementById("annot-note");
      var note = noteEl ? noteEl.value.trim() || null : null;

      var content = document.getElementById("chapter-content");
      var offset = getTextOffset(
        content,
        selectRange.startContainer,
        selectRange.startOffset,
      );
      var endOffset = getTextOffset(
        content,
        selectRange.endContainer,
        selectRange.endOffset,
      );
      var fullText = content ? content.textContent : "";
      var totalLen = fullText.length || 1;

      fetch("/api/book/" + bookId + "/annotations", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          chapter: chapter,
          selected_text: text,
          note: note,
          position_start: offset / totalLen,
          position_end: endOffset / totalLen,
        }),
      })
        .then(function (r) {
          return r.json();
        })
        .then(function (data) {
          saveProgress();
          window.location.reload();
        })
        .catch(function () {});
    }

    if (action === "cancel") {
      annotPopup.classList.remove("visible");
      selection = null;
      selectRange = null;
      return;
    }

    annotPopup.classList.remove("visible");
    selection = null;
    selectRange = null;
  });

  function highlightRange(range, color) {
    try {
      var span = document.createElement("mark");
      span.style.backgroundColor = color;
      span.style.borderRadius = "3px";
      span.style.padding = "0 1px";
      range.surroundContents(span);
    } catch (e) {}
  }

  // ── Load existing annotations ──────────────────────
  if (bookId) {
    fetch("/api/book/" + bookId + "/annotations")
      .then(function (r) {
        return r.json();
      })
      .then(function (list) {
        var content = document.getElementById("chapter-content");
        if (!content || !list.length) return;
        var fullText = content.textContent;
        var totalLen = fullText.length || 1;

        list.forEach(function (a) {
          if (a.chapter !== chapter) return;
          var start = Math.floor(a.position_start * totalLen);
          var end = Math.ceil(a.position_end * totalLen);
          applyHighlight(start, end, a.color);
        });
      })
      .catch(function () {});
  }

  function applyHighlight(start, end, color) {
    var content = document.getElementById("chapter-content");
    if (!content) return;
    var walker = document.createTreeWalker(content, NodeFilter.SHOW_TEXT);
    var currentPos = 0;
    var startNode = null,
      startOffset = 0;
    var endNode = null,
      endOffset = 0;

    while (walker.nextNode()) {
      var node = walker.currentNode;
      var len = node.textContent.length;
      if (!startNode && currentPos + len > start) {
        startNode = node;
        startOffset = start - currentPos;
      }
      if (!endNode && currentPos + len >= end) {
        endNode = node;
        endOffset = end - currentPos;
        break;
      }
      currentPos += len;
    }

    if (startNode && endNode) {
      try {
        var range = document.createRange();
        range.setStart(startNode, startOffset);
        range.setEnd(endNode, endOffset);
        var mark = document.createElement("mark");
        mark.style.backgroundColor = color || "#ffee77";
        mark.style.borderRadius = "3px";
        mark.style.padding = "0 1px";
        range.surroundContents(mark);
      } catch (e) {}
    }
  }

  function getTextOffset(root, targetNode, targetOffset) {
    var walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
    var pos = 0;
    while (walker.nextNode()) {
      if (walker.currentNode === targetNode) return pos + targetOffset;
      pos += walker.currentNode.textContent.length;
    }
    return pos;
  }
})();
