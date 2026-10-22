document.querySelectorAll(".annotation-edit").forEach(function (form) {
  form.addEventListener("submit", async function (event) {
    event.preventDefault();
    const button = form.querySelector("button");
    button.disabled = true;
    try {
      const response = await fetch(form.dataset.url, {
        method: "PATCH", headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ note: form.elements.note.value.trim() || null, color: form.elements.color.value }),
      });
      if (!response.ok) throw new Error("Unable to save annotation");
      window.location.reload();
    } catch (_) {
      const status = form.querySelector("[role=status]");
      status.textContent = status.dataset.error;
      button.disabled = false;
    }
  });
});
