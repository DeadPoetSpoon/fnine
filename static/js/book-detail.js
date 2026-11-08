(function () {
  "use strict";
  const entries = document.getElementById("book-entries");
  if (!entries) return;
  const tabs = Array.from(entries.querySelectorAll('[role="tab"]'));

  function selectTab(selected, updateUrl = true) {
    tabs.forEach(function (tab) {
      const active = tab === selected;
      tab.setAttribute("aria-selected", String(active));
      tab.tabIndex = active ? 0 : -1;
      document.getElementById(tab.getAttribute("aria-controls")).hidden = !active;
    });
    if (updateUrl) {
      const url = new URL(location.href);
      url.searchParams.set("tab", selected.dataset.tab);
      history.replaceState(null, "", url);
    }
  }

  tabs.forEach(function (tab, index) {
    tab.addEventListener("click", function () { selectTab(tab); });
    tab.addEventListener("keydown", function (event) {
      let next;
      if (event.key === "ArrowRight") next = (index + 1) % tabs.length;
      else if (event.key === "ArrowLeft") next = (index + tabs.length - 1) % tabs.length;
      else if (event.key === "Home") next = 0;
      else if (event.key === "End") next = tabs.length - 1;
      else return;
      event.preventDefault();
      selectTab(tabs[next]);
      tabs[next].focus();
    });
  });
  window.addEventListener("popstate", function () {
    const selected = new URL(location.href).searchParams.get("tab");
    selectTab(tabs.find(function (tab) { return tab.dataset.tab === selected; }) || tabs[0], false);
  });
})();
