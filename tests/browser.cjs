const assert = require("node:assert/strict");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const net = require("node:net");
const { spawn, spawnSync } = require("node:child_process");
const { chromium } = require("playwright");

async function dismissNote(page, action) {
  await page.locator("#endnote-dialog").evaluate(function (dialog) {
    delete dialog.dataset.closeComplete;
    dialog.addEventListener("close", function () { dialog.dataset.closeComplete = "true"; }, { once: true });
  });
  await action();
  await page.locator('#endnote-dialog[data-close-complete="true"]').waitFor({ state: "attached" });
}

async function waitForProgressSave(page, action) {
  const saved = page.waitForResponse(function (response) {
    return new URL(response.url()).pathname === "/api/progress" && response.request().method() === "POST" && response.ok();
  });
  await action();
  await (await saved).finished();
}

async function checkReaderStatus(page, failed = false) {
  assert.equal(await page.locator("#reader-status").count(), 1);
  if (!failed) {
    await page.waitForFunction(function () {
      const status = document.getElementById("reader-status");
      return status.hidden && status.textContent === "" && !status.classList.contains("save-error");
    });
    assert.ok(await page.locator("#reader-status").evaluate(function (status) { return !!status.closest(".reader-toolbar") && status.getClientRects().length === 0; }), "Successful saves must not display a status or reserve toolbar space");
    return;
  }
  await page.waitForFunction(function () {
    const status = document.getElementById("reader-status");
    return !status.hidden && status.classList.contains("save-error") && status.textContent.trim().length > 0;
  });
  assert.ok(await page.locator("#reader-status").evaluate(function (status) {
    const toolbar = status.closest(".reader-toolbar");
    if (!toolbar) return false;
    const parent = toolbar.getBoundingClientRect();
    const bounds = status.getBoundingClientRect();
    return bounds.width > 0 && bounds.height > 0 && bounds.left >= parent.left && bounds.right <= parent.right && bounds.top >= parent.top && bounds.bottom <= parent.bottom && parent.left >= 0 && parent.right <= window.innerWidth;
  }), "Save status must fit inside the top reader toolbar");
}

async function checkNotePosition(page) {
  const failures = await page.evaluate(function () {
    const failures = [];
    const sample = document.createElement("div");
    sample.style.cssText = "position:fixed;top:0;left:0;font:20px/2 sans-serif;white-space:normal;";
    const anchor = document.querySelector("#chapter-text .fn-ref").closest(".note-anchor").cloneNode(true);
    const marker = anchor.querySelector(".fn-ref");
    marker.textContent = "123";
    marker.removeAttribute("data-note");
    const text = document.createTextNode("");
    sample.append(text, anchor);
    document.body.append(sample);
    for (const words of ["甲乙丙丁", "one two word"]) {
      text.textContent = words.slice(0, -1);
      anchor.firstChild.textContent = words.slice(-1);
      for (let width = 65; width <= 200; width++) {
        sample.style.width = width + "px";
        const preceding = document.createRange();
        preceding.selectNodeContents(anchor.firstChild);
        const reference = document.createRange();
        reference.selectNodeContents(marker);
        const word = preceding.getBoundingClientRect();
        const number = reference.getBoundingClientRect();
        if (number.left < word.right - 1 || number.left > word.right + 1 || number.top >= word.top || number.bottom >= word.bottom) {
          failures.push({ words, width, word: word.toJSON(), number: number.toJSON() });
          break;
        }
      }
      if (sample.textContent !== words + "123") failures.push("Note styling changed text coordinates");
    }
    sample.remove();
    return failures;
  });
  assert.deepEqual(failures, [], "Note markers must stay at the upper-right of their preceding text, including at line breaks");
}

async function checkSelect(locator) {
  const styles = await locator.evaluate(function (select) {
    const style = getComputedStyle(select);
    const optionStyle = getComputedStyle(select.options[0]);
    const bounds = select.getBoundingClientRect();
    return {
      appearance: style.appearance,
      radius: style.borderRadius,
      background: style.backgroundColor,
      optionBackground: optionStyle.backgroundColor,
      color: style.color,
      textColor: getComputedStyle(document.body).color,
      arrow: style.backgroundImage,
      padding: parseFloat(style.paddingRight),
      height: bounds.height,
      fits: bounds.left >= 0 && bounds.right <= window.innerWidth,
    };
  });
  assert.equal(styles.appearance, "none");
  assert.equal(styles.radius, "8px");
  assert.equal(styles.background, styles.optionBackground);
  assert.equal(styles.color, styles.textColor);
  assert.notEqual(styles.arrow, "none");
  assert.ok(styles.padding >= 32 && styles.height >= 44 && styles.fits);
}

async function run() {
  const root = path.resolve(__dirname, "..");
  const temporary = fs.mkdtempSync(path.join(os.tmpdir(), "fnine-browser-"));
  const fixture = path.join(temporary, "fixture.epub");
  for (const args of [["build", "--locked"], ["test", "--locked", "browser_fixture", "--", "--ignored"]]) {
    const result = spawnSync("cargo", args, { cwd: root, env: { ...process.env, FNINE_TEST_EPUB: fixture }, stdio: "inherit" });
    assert.equal(result.status, 0, "Rust browser fixture/build failed");
  }
  const port = await new Promise(function (resolve) {
    const listener = net.createServer();
    listener.listen(0, "127.0.0.1", function () {
      const port = listener.address().port;
      listener.close(function () { resolve(port); });
    });
  });
  const base = `http://127.0.0.1:${port}`;
  const server = spawn(path.join(root, "target/debug/fnine"), [], {
    cwd: root,
    env: { ...process.env, FNINE_HOST: "127.0.0.1", FNINE_PORT: String(port), FNINE_DATA_DIR: path.join(temporary, "data") },
    stdio: ["ignore", "pipe", "pipe"],
  });
  let serverOutput = "";
  server.stdout.on("data", function (data) { serverOutput += data; });
  server.stderr.on("data", function (data) { serverOutput += data; });
  let browser;
  try {
    let ready = false;
    for (let attempt = 0; attempt < 100; attempt++) {
      try { ready = (await fetch(base)).ok; } catch (_) {}
      if (ready) break;
      await new Promise(function (resolve) { setTimeout(resolve, 100); });
    }
    assert.ok(ready, serverOutput);
    const executablePath = process.env.FNINE_BROWSER || (fs.existsSync("/usr/bin/chromium") ? "/usr/bin/chromium" : undefined);
    browser = await chromium.launch({ executablePath, headless: true, args: ["--no-sandbox"] });
    const context = await browser.newContext({ viewport: { width: 1280, height: 900 } });
    const page = await context.newPage();
    const errors = [];
    page.on("pageerror", function (error) { errors.push(error.message); });
    await page.goto(base + "/settings?lang=en");
    await page.locator('label').filter({ has: page.locator('[name="language"][value="en"]') }).click();
    await page.locator('label').filter({ has: page.locator('[name="theme"][value="dark"]') }).click();
    await page.getByRole("button", { name: "Save", exact: true }).click();
    await page.waitForURL("**/settings?lang=en&theme=dark");
    const fontPath = [process.env.FNINE_TEST_FONT, "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf", "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf", "/usr/share/fonts/truetype/liberation2/LiberationSans-Regular.ttf", "/usr/share/fonts/ttf-ia-writer/iAWriterQuattroS-Regular.ttf"].find(function (candidate) { return candidate && fs.existsSync(candidate); });
    assert.ok(fontPath, "Set FNINE_TEST_FONT to a valid TTF file for font-loading coverage");
    await page.locator("#font-upload-input").setInputFiles(fontPath);
    await page.waitForURL("**/settings");
    const customFont = page.locator('[name="font_family"]:not([value=""])').last();
    const fontId = await customFont.getAttribute("value");
    assert.match(fontId, /^[0-9a-f-]{36}\.ttf$/);
    await customFont.locator("..").click();
    await page.getByRole("button", { name: "Save", exact: true }).click();
    await page.waitForURL("**/settings?lang=en&theme=dark");
    await page.goto(base + "/upload");
    assert.equal(await page.locator("html").getAttribute("lang"), "en");
    assert.equal(await page.locator("html").getAttribute("data-theme"), "dark");
    await page.locator("#file-input").setInputFiles(fixture);
    await page.locator(".upload-form").getByRole("button", { name: "Upload" }).click();
    await page.waitForURL("**/book/*?lang=en&theme=dark");
    const bookPath = new URL(page.url()).pathname;
    await waitForProgressSave(page, function () { return page.getByRole("link", { name: "Start Reading" }).click(); });
    await page.waitForURL("**/read/0?lang=en&theme=dark");
    assert.match(await page.locator(".chapter-title").textContent(), /Actual chapter title/);
    await checkReaderStatus(page);
    const originalPage = await context.request.get(page.url());
    assert.ok(originalPage.ok());
    assert.ok(await page.evaluate(function (html) {
      const original = new DOMParser().parseFromString(html, "text/html");
      return original.getElementById("chapter-text").textContent === document.getElementById("chapter-text").textContent;
    }, await originalPage.text()), "Attaching note markers must preserve the exact chapter text");
    const repeatedReferences = page.locator('[data-note="note-0"]');
    assert.ok(await repeatedReferences.last().evaluate(function (reference) { return !reference.closest(".note-anchor"); }), "A paragraph-start reference must not attach to the preceding paragraph");
    assert.ok(await repeatedReferences.nth((await repeatedReferences.count()) - 2).evaluate(function (reference) {
      const anchor = reference.closest(".note-anchor");
      return anchor.firstChild.textContent === "😀" && anchor.querySelectorAll(".fn-ref").length === 2;
    }), "Adjacent references must remain together without splitting Unicode characters");
    await checkNotePosition(page);
    assert.ok(await page.evaluate(async function () {
      await document.fonts.load('18px "FnineCustom"');
      return Array.from(document.fonts).some(function (font) { return font.family === "FnineCustom" && font.status === "loaded"; });
    }));
    const reference = page.locator('[data-note="note-0"]').first();
    await reference.focus();
    await page.keyboard.press("Enter");
    assert.ok(await page.locator("#endnote-dialog").evaluate(function (dialog) { return dialog.open; }));
    assert.match(await page.locator("#endnote-body").textContent(), /The actual endnote text 中文/);
    await dismissNote(page, function () { return page.keyboard.press("Escape"); });
    assert.ok(await reference.evaluate(function (element) { return document.activeElement === element; }));
    await page.locator('[data-note="note-2"]').click();
    assert.match(await page.locator("#endnote-body").textContent(), /could not be found/);
    await dismissNote(page, function () { return page.getByRole("button", { name: "Close", exact: true }).click(); });
    const publisherNotes = [
      ["note-4", /The actual publisher-styled note body/],
      ["note-5", /The first unlinked note body/],
      ["note-6", /A different unlinked note body/],
    ];
    for (const [key, body] of publisherNotes) {
      const trigger = page.locator(`[data-note="${key}"]`).first();
      await trigger.focus();
      await page.keyboard.press("Enter");
      assert.match(await page.locator("#endnote-body").textContent(), body);
      assert.doesNotMatch(await page.locator("#endnote-body").textContent(), /\[17\]/);
      await dismissNote(page, function () { return page.keyboard.press("Escape"); });
      assert.ok(await trigger.evaluate(function (element) { return document.activeElement === element; }));
    }

    const bookId = bookPath.split("/").pop();
    await reference.scrollIntoViewIfNeeded();
    await page.waitForTimeout(650);
    const current = await (await context.request.get(base + "/api/recent")).json();
    await reference.click();
    await page.waitForTimeout(650);
    const afterNote = await (await context.request.get(base + "/api/recent")).json();
    assert.equal(afterNote.progress.position, current.progress.position);
    await dismissNote(page, function () { return page.keyboard.press("Escape"); });

    await page.evaluate(function () {
      const node = document.getElementById("chapter-text").firstChild;
      const range = document.createRange(); range.setStart(node, 0); range.setEnd(node, 7);
      const selection = window.getSelection(); selection.removeAllRanges(); selection.addRange(range);
    });
    await page.locator("#annotate-selection").waitFor({ state: "visible" });
    await page.locator("#annotate-selection").click();
    await page.locator("#annotation-note").fill("Browser-created annotation");
    await checkSelect(page.locator("#annotation-color"));
    await page.locator("#annotation-color").selectOption("#a6d8ff");
    await page.getByRole("button", { name: "Confirm", exact: true }).click();
    await page.waitForFunction(function () { return !document.getElementById("annotation-dialog").open; });
    assert.ok(await page.locator("mark").count());
    const text = await page.locator("#chapter-text").textContent();
    const overlap = await context.request.post(base + `/api/book/${bookId}/annotations`, { data: { chapter: 0, selected_text: text.slice(3, 12), note: "Overlap", position_start: 3 / text.length, position_end: 12 / text.length, color: "#ffb3c6" } });
    assert.ok(overlap.ok());
    await waitForProgressSave(page, function () { return page.reload(); });
    await checkReaderStatus(page);
    assert.ok(await page.locator("mark").count() >= 3);
    assert.equal(await page.locator("#chapter-text").textContent(), text);

    await page.evaluate(function () { window.scrollTo(0, document.documentElement.scrollHeight * 0.45); });
    await page.waitForTimeout(900);
    const saved = await (await context.request.get(base + "/api/recent")).json();
    assert.ok(saved.progress.position > 0.2 && saved.progress.position < 0.8);
    await waitForProgressSave(page, function () { return page.reload(); });
    await checkReaderStatus(page);
    const restored = await (await context.request.get(base + "/api/recent")).json();
    assert.ok(Math.abs(restored.progress.position - saved.progress.position) < 0.02);

    await page.setViewportSize({ width: 390, height: 844 });
    await checkNotePosition(page);
    await page.evaluate(function () { localStorage.removeItem("fnine-sidebar"); });
    await page.reload();
    await page.locator("#toc-open-btn").click();
    assert.ok(await page.locator("#toc-sidebar").evaluate(function (sidebar) { return sidebar.classList.contains("open") && sidebar.getBoundingClientRect().left >= 0; }));
    await page.keyboard.press("Escape");
    assert.ok(await page.locator("#toc-sidebar").evaluate(function (sidebar) { return sidebar.hidden; }));
    await checkReaderStatus(page);
    await page.locator('[data-note="note-0"]').first().click();
    assert.ok(await page.locator("#endnote-dialog").evaluate(function (dialog) { return dialog.getBoundingClientRect().width < window.innerWidth; }));
    assert.ok(await page.locator("#endnote-dialog").evaluate(function (dialog) { return dialog.scrollHeight > dialog.clientHeight; }));
    await page.locator("#endnote-dialog").evaluate(function (dialog) { dialog.scrollTop = dialog.scrollHeight; });
    assert.ok(await page.locator("#endnote-dialog").evaluate(function (dialog) { return dialog.scrollTop > 0; }));
    await dismissNote(page, function () { return page.keyboard.press("Escape"); });

    await page.goto(base + bookPath);
    await page.locator("details").first().locator("summary").click();
    await checkSelect(page.locator('.annotation-edit select').first());
    await page.locator(".annotation-edit").first().locator("textarea").fill("Edited in browser");
    await page.locator(".annotation-edit").first().getByRole("button", { name: "Save", exact: true }).click();
    await page.waitForFunction(function () { return document.body.textContent.includes("Edited in browser"); });
    const exported = await context.request.get(base + bookPath + "/annotations/export");
    assert.match(await exported.text(), /Edited in browser/);
    await page.goto(base + "/?status=reading&sort=last_read");
    assert.ok(await page.locator(".continue-reading").count());
    assert.equal(await page.locator(".book-card").count(), 1);
    for (const theme of ["light", "dark", "sepia"]) {
      await page.goto(base + `/?lang=zh&theme=${theme}`);
      for (const width of [1280, 390, 320]) {
        await page.setViewportSize({ width, height: 900 });
        await checkSelect(page.locator("#library-sort"));
        await checkSelect(page.locator("#library-status"));
        assert.ok(await page.locator(".library-controls").evaluate(function (controls) {
          const bottoms = Array.from(controls.querySelectorAll("select, button")).map(function (element) { return element.getBoundingClientRect().bottom; });
          return document.documentElement.scrollWidth <= window.innerWidth && (window.innerWidth <= 480 || Math.max(...bottoms) - Math.min(...bottoms) < 1);
        }));
      }
    }
    await page.locator("#library-sort").focus();
    await page.keyboard.press("ArrowDown");
    await page.keyboard.press("Tab");
    assert.equal(await page.locator("#library-sort").inputValue(), "last_read");
    assert.ok(await page.locator("#library-status").evaluate(function (select) {
      return document.activeElement === select && getComputedStyle(select).outlineStyle !== "none";
    }));
    await page.emulateMedia({ forcedColors: "active" });
    assert.equal(await page.locator("#library-sort").evaluate(function (select) { return getComputedStyle(select).appearance; }), "auto");
    await page.emulateMedia({ forcedColors: "none" });

    await waitForProgressSave(page, function () { return page.goto(base + bookPath + "/read/0"); });
    await checkReaderStatus(page);
    await page.route("**/api/progress", function (route) { return route.fulfill({ status: 500, body: "simulated failure" }); });
    await page.evaluate(function () { window.scrollBy(0, 500); });
    await page.waitForFunction(function () { return document.getElementById("reader-status").classList.contains("save-error"); });
    await checkReaderStatus(page, true);
    await page.unroute("**/api/progress");
    await waitForProgressSave(page, function () { return page.evaluate(function () { window.scrollBy(0, 100); }); });
    await checkReaderStatus(page);
    for (const [chapter, language, expected] of [
      [1, "en", "Actual chapter title (continued)"],
      [2, "en", "Actual chapter title (continued 2)"],
      [2, "zh", "Actual chapter title (续 2)"],
      [3, "en", "Next chapter"],
      [4, "en", "Next chapter (continued)"],
      [1, "en", "Actual chapter title (continued)"],
    ]) {
      await page.goto(base + bookPath + `/read/${chapter}?lang=${language}`);
      assert.equal(await page.locator(".chapter-title").textContent(), expected);
      assert.equal(await page.locator(".toc-item.active").textContent(), expected);
      assert.equal(await page.locator("#reader-app").getAttribute("data-chapter"), String(chapter));
    }
    if (process.env.FNINE_TEST_BOOKS) {
      const directory = path.resolve(process.env.FNINE_TEST_BOOKS);
      for (const filename of fs.readdirSync(directory).filter(function (filename) { return filename.endsWith(".epub"); })) {
        const upload = await context.request.post(base + "/upload", { multipart: { file: { name: filename, mimeType: "application/epub+zip", buffer: fs.readFileSync(path.join(directory, filename)) } } });
        assert.ok(upload.ok(), `Real EPUB upload failed: ${filename}`);
        const realBook = new URL(upload.url()).pathname;
        await page.goto(base + realBook + "/read/0");
        const chapters = await page.locator(".toc-list a").evaluateAll(function (links) { return links.map(function (link) { return link.href; }); });
        let checked = false;
        for (const chapter of chapters) {
          const response = await context.request.get(chapter);
          assert.ok(response.ok());
          if (!(await response.text()).includes('data-note="')) continue;
          await page.goto(chapter);
          const trigger = page.locator("[data-note]").first();
          const key = await trigger.getAttribute("data-note");
          const expected = await page.locator(`#${key}`).evaluate(function (template) { return template.content.querySelector(".note-body").textContent; });
          assert.ok(expected.trim().length > 1);
          assert.doesNotMatch(expected, /could not be found/);
          await trigger.click();
          assert.equal(await page.locator("#endnote-body .note-body").textContent(), expected);
          assert.ok(await page.locator("#endnote-dialog").evaluate(function (dialog) { return dialog.open; }));
          await dismissNote(page, function () { return page.keyboard.press("Escape"); });
          checked = true;
          console.log(`Real EPUB popup passed: ${filename}`);
          break;
        }
        assert.ok(checked, `No real EPUB note popup checked: ${filename}`);
      }
    }
    assert.deepEqual(errors, []);
    console.log("Chromium checks passed: attached superscript markers, localized continuation titles, actual/long endnotes, keyboard/focus, font upload/loading, preferences, annotation creation/edit/overlap/export, mobile TOC, progress restore, filters, save failures.");
  } catch (error) {
    console.error(serverOutput);
    throw error;
  } finally {
    if (browser) await browser.close();
    server.kill("SIGTERM");
    await new Promise(function (resolve) { if (server.exitCode !== null) resolve(); else server.once("exit", resolve); });
    fs.rmSync(temporary, { recursive: true, force: true });
  }
}
run().catch(function (error) { console.error(error); process.exitCode = 1; });
