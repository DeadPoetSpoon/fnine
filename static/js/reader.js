// Reader UI controller
(function () {
  // ── Theme ──────────────────────────────────────────
  function getTheme() {
    return localStorage.getItem("fnine-theme") || "light";
  }

  function applyTheme(theme) {
    document.documentElement.dataset.theme = theme;
    var btn = document.getElementById("theme-btn");
    if (btn) {
      var icons = { light: "☀️", dark: "🌙", sepia: "📜" };
      btn.textContent = icons[theme] || "☀️";
    }
  }

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

    if (btn.id === "theme-btn") {
      var themes = ["light", "dark", "sepia"];
      var cur = getTheme();
      var next = themes[(themes.indexOf(cur) + 1) % themes.length];
      localStorage.setItem("fnine-theme", next);
      applyTheme(next);
    }

    if (btn.getAttribute("data-action") === "hide-sidebar") {
      hideSidebar();
    }

    if (btn.getAttribute("data-action") === "open-toc") {
      openToc();
    }
  });

  // ── Init ───────────────────────────────────────────
  applyTheme(getTheme());
  applySidebar();
})();
