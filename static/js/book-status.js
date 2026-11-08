document.querySelectorAll("[data-book-status]").forEach(function (button) {
  button.addEventListener("click", async function () {
    if (button.dataset.confirm && !confirm(button.dataset.confirm)) return;
    button.disabled = true;
    const status = button.parentElement.querySelector(".status-message");
    try {
      const response = await fetch("/api/book/" + button.dataset.bookId + "/status", {
        method: "POST", headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ finished: button.dataset.bookStatus === "true" }),
      });
      if (!response.ok) throw new Error("Unable to update reading status");
      window.location.reload();
    } catch (_) {
      status.textContent = status.dataset.error;
      button.disabled = false;
    }
  });
});
