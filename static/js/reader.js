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

  // ── Progress auto-save ─────────────────────────────
  var progressTimer = null;
  var pageMeta = document.getElementById("reader-app");
  var bookId = pageMeta ? pageMeta.dataset.bookId : null;
  var chapter = pageMeta ? parseInt(pageMeta.dataset.chapter) : 0;
  var initPos = pageMeta ? parseFloat(pageMeta.dataset.position) || 0 : 0;

  // Restore scroll position
  if (initPos > 0) {
    var docHeight = document.documentElement.scrollHeight - window.innerHeight;
    window.scrollTo(0, Math.round(initPos * docHeight));
  }

  function saveProgress() {
    if (!bookId) return;
    var scrollPos = window.scrollY;
    var docHeight = document.documentElement.scrollHeight - window.innerHeight;
    var position =
      docHeight > 0 ? Math.min(1, Math.max(0, scrollPos / docHeight)) : 0;

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
    annotPopup.style.top = rect.top + window.scrollY - 40 + "px";
    annotPopup.style.left =
      rect.left + window.scrollX + rect.width / 2 - 40 + "px";
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
