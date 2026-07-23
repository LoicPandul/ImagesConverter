// ImagesConverter UI — vanilla JS over the Tauri global API (no build step).

const { invoke, Channel } = window.__TAURI__.core;
const { getCurrentWindow } = window.__TAURI__.window;
const { getCurrentWebview } = window.__TAURI__.webview;
const { open } = window.__TAURI__.dialog;
const { revealItemInDir } = window.__TAURI__.opener;

const appWindow = getCurrentWindow();

const $ = (id) => document.getElementById(id);
const els = {
  dropzone: $("dropzone"),
  browse: $("browse"),
  queue: $("queue"),
  list: $("queue-list"),
  count: $("queue-count"),
  clearAll: $("clear-all"),
  clearDone: $("clear-done"),
  convert: $("convert"),
  instant: $("instant"),
  dragLabel: $("drag-label"),
  status: $("status-line"),
  compress: $("compress"),
  maxKb: $("max-kb"),
  deleteOriginal: $("delete-original"),
  removeBg: $("remove-bg"),
  bgHint: $("bg-hint"),
  bgSetup: $("bg-setup"),
  bgSetupSize: $("bg-setup-size"),
  bgDownload: $("bg-download"),
  bgCancelSetup: $("bg-cancel-setup"),
  bgProgress: $("bg-progress"),
  bgProgressFill: $("bg-progress-fill"),
  bgProgressText: $("bg-progress-text"),
  segThumb: document.querySelector(".segment-thumb"),
};

const EXTENSIONS = ["jpg", "jpeg", "png", "webp", "gif", "bmp", "tif", "tiff"];

/** path -> item {path,name,size,ext,supported,status,el,...} */
const items = new Map();
let converting = false;
/** Background-removal assets (runtime + model) present on disk. */
let bgReady = false;
let bgInstalling = false;

/* ---------- helpers ---------- */

function fmtBytes(n) {
  if (n == null) return "";
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(n < 10240 ? 1 : 0)} KB`;
  return `${(n / (1024 * 1024)).toFixed(2)} MB`;
}

function baseName(path) {
  const i = Math.max(path.lastIndexOf("\\"), path.lastIndexOf("/"));
  return i >= 0 ? path.slice(i + 1) : path;
}

function svg(pathData, viewBox = "0 0 16 16", cls = "") {
  return `<svg class="${cls}" viewBox="${viewBox}" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${pathData}</svg>`;
}

const ICONS = {
  image: svg('<rect x="2" y="3" width="12" height="10" rx="2"/><circle cx="5.6" cy="6.4" r="1" fill="currentColor" stroke="none"/><path d="M4 11l3-3.4 2.4 2.7 1.6-1.6 2 2.3"/>'),
  check: svg('<path class="ok" d="M3 8.5l3.2 3.2L13 5"/>'),
  alert: svg('<path class="err" d="M8 5v4.2M8 11.8v.2"/><circle class="err" cx="8" cy="8" r="6.4"/>'),
  reveal: svg('<path d="M2 5a1.5 1.5 0 0 1 1.5-1.5h2.6l1.5 1.7h5A1.5 1.5 0 0 1 14 6.7V11a1.5 1.5 0 0 1-1.5 1.5h-9A1.5 1.5 0 0 1 2 11z"/>'),
  remove: svg('<path d="M4 4l8 8M12 4l-8 8"/>'),
};

/* ---------- options ---------- */

function currentFormat() {
  return document.querySelector('input[name="format"]:checked').value;
}

function currentOptions() {
  const compressOn = els.compress.checked;
  const kb = parseInt(els.maxKb.value, 10);
  return {
    format: currentFormat(),
    maxSizeKb: compressOn ? (Number.isFinite(kb) && kb > 0 ? kb : 500) : null,
    deleteOriginal: els.deleteOriginal.checked,
    removeBackground: els.removeBg.checked && !els.removeBg.disabled,
  };
}

function saveOptions() {
  try {
    localStorage.setItem("options", JSON.stringify({
      format: currentFormat(),
      compress: els.compress.checked,
      maxKb: els.maxKb.value,
      deleteOriginal: els.deleteOriginal.checked,
      removeBg: els.removeBg.checked,
      instant: els.instant.checked,
    }));
  } catch { /* best effort */ }
}

function restoreOptions() {
  try {
    const saved = JSON.parse(localStorage.getItem("options") || "null");
    if (!saved) return;
    if (["jpeg", "webp", "png"].includes(saved.format)) {
      document.querySelector(`input[name="format"][value="${saved.format}"]`).checked = true;
    }
    els.compress.checked = !!saved.compress;
    if (/^\d*$/.test(saved.maxKb ?? "")) els.maxKb.value = saved.maxKb ?? "";
    els.deleteOriginal.checked = saved.deleteOriginal !== false;
    els.instant.checked = !!saved.instant;
  } catch { /* corrupted storage must never brick the app */ }
  syncCompressField();
}

function syncCompressField() {
  els.maxKb.disabled = !els.compress.checked;
}

/* ---------- instant convert ---------- */

function instantOn() {
  return els.instant.checked;
}

function syncInstantUi() {
  els.dragLabel.textContent = instantOn() ? "Release to convert" : "Release to add";
}

/** Idle status line: what will happen next, given the current mode. */
function idleLine() {
  const fmt = currentFormat().toUpperCase();
  return instantOn()
    ? `Instant convert — images convert to ${fmt} as soon as you add them`
    : `Mode: convert to ${fmt}`;
}

function readyLine(ready) {
  return `${ready} image${ready > 1 ? "s" : ""} ready — ${currentFormat().toUpperCase()}`;
}

/* ---------- background removal ---------- */

function syncBgControl() {
  const jpeg = currentFormat() === "jpeg";
  els.removeBg.disabled = jpeg;
  els.bgHint.hidden = !jpeg;
  els.bgHint.textContent = jpeg ? "WEBP / PNG only" : "";
  if (jpeg) showBgSetup(false);
}

function showBgSetup(show) {
  els.bgSetup.hidden = !show;
  if (show) els.bgDownload.focus();
}

async function initBg() {
  els.bgSetupSize.textContent = "≈250 MB";
  try {
    const status = await invoke("bg_status");
    bgReady = status.ready;
    if (!bgReady) els.bgSetupSize.textContent = `≈${fmtBytes(status.missingBytes)}`;
    if (bgReady && !els.removeBg.disabled) {
      const saved = JSON.parse(localStorage.getItem("options") || "null");
      if (saved?.removeBg) els.removeBg.checked = true;
    }
  } catch { /* feature stays off */ }
  syncBgControl();
}

async function installBg() {
  if (bgInstalling) return;
  bgInstalling = true;
  els.bgDownload.disabled = true;
  els.bgDownload.textContent = "Downloading…";
  els.bgCancelSetup.hidden = true;
  els.bgProgress.hidden = false;

  const onEvent = new Channel();
  onEvent.onmessage = (ev) => {
    if (ev.type === "progress") {
      const pct = ev.total ? Math.min(100, (ev.received / ev.total) * 100) : 0;
      els.bgProgressFill.style.width = `${pct.toFixed(1)}%`;
      els.bgProgressText.textContent = `${fmtBytes(ev.received)} / ${fmtBytes(ev.total)}`;
    }
  };

  try {
    await invoke("bg_install", { onEvent });
    bgReady = true;
    showBgSetup(false);
    if (!els.removeBg.disabled) {
      els.removeBg.checked = true;
      saveOptions();
      els.removeBg.focus();
    } else {
      els.browse.focus();
    }
    if (!converting) setStatus("Background removal ready", "good");
  } catch (e) {
    // The error lives in the panel: the status line may be owned by a
    // running conversion.
    els.bgProgressText.textContent = `failed: ${e}`;
    els.bgCancelSetup.hidden = false;
    if (!converting) setStatus("Background removal setup failed", "bad");
  } finally {
    bgInstalling = false;
    els.bgDownload.disabled = false;
    els.bgDownload.textContent = "Download";
    els.bgProgress.hidden = true;
    els.bgProgressFill.style.width = "0%";
  }
}

function moveSegmentThumb() {
  const checked = document.querySelector('input[name="format"]:checked + label');
  if (!checked) return;
  els.segThumb.style.left = `${checked.offsetLeft}px`;
  els.segThumb.style.width = `${checked.offsetWidth}px`;
}

/* ---------- queue rendering ---------- */

function setView() {
  const hasItems = items.size > 0;
  document.body.dataset.view = hasItems ? "queue" : "empty";
  els.queue.hidden = !hasItems;
  refreshHeader();
  refreshAction();
}

function refreshHeader() {
  const total = items.size;
  els.count.textContent = total ? `· ${total}` : "";
  const finished = [...items.values()].filter((i) => i.status === "done").length;
  els.clearDone.hidden = finished === 0;
}

function readyItems() {
  return [...items.values()].filter((i) => i.status === "ready");
}

function refreshAction() {
  if (converting) {
    els.convert.disabled = false;
    els.convert.classList.add("cancel");
    els.convert.textContent = "Cancel";
    return;
  }
  els.convert.classList.remove("cancel");
  const ready = readyItems().length;
  els.convert.disabled = ready === 0;
  els.convert.textContent = ready > 1 ? `Convert ${ready} images` : "Convert";
}

function setStatus(text, tone = "") {
  els.status.textContent = text;
  els.status.className = `status-line ${tone}`;
}

// File names and backend strings go through textContent only — never markup.
function span(text, className) {
  const s = document.createElement("span");
  if (className) s.className = className;
  s.textContent = text;
  return s;
}

function cardSub(item) {
  const el = item.el.querySelector(".file-sub");
  el.textContent = "";
  switch (item.status) {
    case "ready":
      el.append(span(fmtBytes(item.size), "mono"));
      break;
    case "converting":
      el.append(span(fmtBytes(item.size), "mono"), span("→", "arrow"), "converting…");
      break;
    case "done": {
      const out = item.result;
      el.append(
        span(fmtBytes(out.inBytes), "mono"),
        span("→", "arrow"),
        span(fmtBytes(out.outBytes), "mono out"),
        " ",
        span(baseName(out.outPath), "out"),
      );
      if (out.resizedTo) {
        el.append(" ", span(`resized ${out.resizedTo[0]}×${out.resizedTo[1]}`, "mono"));
      }
      if (out.warning) el.append(` — ${out.warning}`);
      break;
    }
    case "error":
      el.textContent = item.message || "failed";
      break;
  }
}

function cardTrail(item) {
  const trail = item.el.querySelector(".trail");
  trail.innerHTML = "";

  if (item.status === "done" && item.result) {
    if (item.result.backgroundRemoved) {
      const chip = document.createElement("span");
      chip.className = "badge chip";
      chip.textContent = "no bg";
      trail.appendChild(chip);
    }
    const { inBytes, outBytes } = item.result;
    if (inBytes > 0 && outBytes != null) {
      const delta = 1 - outBytes / inBytes;
      const badge = document.createElement("span");
      badge.className = "badge" + (delta < 0 ? " up" : "");
      badge.textContent = `${delta >= 0 ? "−" : "+"}${Math.abs(delta * 100).toFixed(0)}%`;
      trail.appendChild(badge);
    }
  }

  const state = document.createElement("span");
  state.className = "state-dot";
  if (item.status === "converting") state.innerHTML = '<span class="spinner"></span>';
  else if (item.status === "done") state.innerHTML = ICONS.check;
  else if (item.status === "error") state.innerHTML = ICONS.alert;
  trail.appendChild(state);

  if (item.status === "done" && item.result) {
    const reveal = document.createElement("button");
    reveal.className = "icon-btn";
    reveal.type = "button";
    reveal.title = "Show in folder";
    reveal.setAttribute("aria-label", `Show ${baseName(item.result.outPath)} in folder`);
    reveal.innerHTML = ICONS.reveal;
    reveal.addEventListener("click", () => revealItemInDir(item.result.outPath).catch(() => {}));
    trail.appendChild(reveal);
  }

  const remove = document.createElement("button");
  remove.className = "icon-btn remove";
  remove.type = "button";
  remove.title = "Remove from queue";
  remove.setAttribute("aria-label", `Remove ${item.name} from queue`);
  remove.innerHTML = ICONS.remove;
  remove.addEventListener("click", () => {
    if (converting) return;
    const nextFocus =
      item.el.nextElementSibling?.querySelector(".icon-btn.remove") ||
      item.el.previousElementSibling?.querySelector(".icon-btn.remove");
    items.delete(item.path);
    item.el.remove();
    setView();
    (nextFocus || (items.size ? els.clearAll : els.browse)).focus();
  });
  trail.appendChild(remove);
}

function renderCard(item) {
  item.el.dataset.state = item.status;
  cardSub(item);
  cardTrail(item);
}

function createCard(item, index) {
  const li = document.createElement("li");
  li.className = "file-card";
  li.style.animationDelay = `${Math.min(index * 30, 240)}ms`;
  li.innerHTML = `
    <div class="thumb">${ICONS.image}</div>
    <div class="meta">
      <span class="file-name" title=""></span>
      <span class="file-sub"></span>
    </div>
    <div class="trail"></div>`;
  li.querySelector(".file-name").textContent = item.name;
  li.querySelector(".file-name").title = item.path;
  item.el = li;
  els.list.appendChild(li);
  renderCard(item);
}

/// Show the actual cutout (with a checkerboard behind it) once done.
async function refreshOutputThumb(item) {
  const requested = item.result?.outPath;
  if (!requested) return;
  try {
    const uri = await invoke("file_thumbnail", { path: requested });
    // A newer conversion may have landed while the thumbnail was loading.
    if (item.result?.outPath !== requested) return;
    const thumb = item.el?.querySelector(".thumb");
    if (!thumb) return;
    thumb.classList.add("alpha");
    thumb.innerHTML = "";
    const img = document.createElement("img");
    img.alt = "";
    img.src = uri;
    thumb.appendChild(img);
  } catch { /* keep the source preview */ }
}

async function loadThumbnail(item) {
  if (!item.supported) return;
  try {
    const uri = await invoke("file_thumbnail", { path: item.path });
    const img = document.createElement("img");
    img.alt = "";
    img.src = uri;
    const thumb = item.el?.querySelector(".thumb");
    if (thumb) { thumb.innerHTML = ""; thumb.appendChild(img); }
  } catch { /* keep the placeholder glyph */ }
}

/* ---------- adding files ---------- */

async function addFiles(paths) {
  if (!paths?.length || converting) return;
  const fresh = [...new Set(paths)];
  let inspected;
  try {
    inspected = await invoke("inspect_files", { paths: fresh });
  } catch (e) {
    setStatus(`Could not read files: ${e}`, "bad");
    return;
  }

  let index = 0;
  let armed = 0; // convertible items this add produced or re-armed
  for (const info of inspected) {
    const existing = items.get(info.path);
    if (existing) {
      // Re-adding a finished/failed file re-arms it.
      if (existing.status !== "converting") {
        existing.status = existing.supported ? "ready" : "error";
        existing.size = info.size;
        existing.result = null;
        existing.el?.querySelector(".thumb")?.classList.remove("alpha");
        renderCard(existing);
        loadThumbnail(existing);
        if (existing.status === "ready") armed += 1;
      }
      continue;
    }
    const item = {
      path: info.path,
      name: info.name,
      size: info.size,
      ext: info.extension,
      supported: info.supported,
      status: info.supported ? "ready" : "error",
      message: info.supported ? "" : `unsupported format${info.extension ? ` .${info.extension}` : ""}`,
      result: null,
      el: null,
    };
    items.set(item.path, item);
    createCard(item, index++);
    loadThumbnail(item);
    if (item.status === "ready") armed += 1;
  }
  setView();
  const ready = readyItems().length;
  // A conversion may have started while inspect_files was in flight; calling
  // convert() now would hit its cancel branch and kill that batch.
  if (!ready || converting) return;
  // Instant convert: adding images presses Convert for you — but only when
  // this add actually brought something convertible, so arming the toggle over
  // a sitting queue keeps its promise of not firing on its own.
  if (instantOn() && armed) convert();
  else setStatus(readyLine(ready));
}

async function browse() {
  if (converting) return;
  const picked = await open({
    multiple: true,
    title: "Choose images",
    filters: [{ name: "Images", extensions: EXTENSIONS }],
  }).catch(() => null);
  if (picked) addFiles(Array.isArray(picked) ? picked : [picked]);
}

/* ---------- conversion ---------- */

async function convert() {
  if (converting) {
    invoke("cancel_conversion").catch(() => {});
    setStatus("Cancelling…");
    return;
  }
  const batch = readyItems();
  if (!batch.length) return;

  converting = true;
  document.body.classList.add("converting");
  refreshAction();
  const options = currentOptions();
  setStatus(`Converting 0/${batch.length}…`);

  let processed = 0;
  const onEvent = new Channel();
  onEvent.onmessage = (ev) => {
    if (ev.type === "start") {
      const item = items.get(ev.path);
      if (item) { item.status = "converting"; renderCard(item); }
    } else if (ev.type === "file") {
      const item = items.get(ev.path);
      processed += 1;
      setStatus(`Converting ${processed}/${batch.length}…`);
      if (!item) return;
      if (ev.ok) {
        item.status = "done";
        item.result = ev;
      } else {
        item.status = "error";
        item.message = ev.message || "failed";
      }
      renderCard(item);
      if (ev.ok && ev.backgroundRemoved) {
        refreshOutputThumb(item);
      } else {
        item.el?.querySelector(".thumb")?.classList.remove("alpha");
      }
    } else if (ev.type === "done") {
      finishBatch(ev, batch);
    }
  };

  try {
    await invoke("convert_files", {
      paths: batch.map((i) => i.path),
      options,
      onEvent,
    });
  } catch (e) {
    converting = false;
    document.body.classList.remove("converting");
    setStatus(`${e}`, "bad");
    [...items.values()].forEach((i) => {
      if (i.status === "converting") { i.status = "ready"; renderCard(i); }
    });
    refreshAction();
    // A failed integrity check removes assets server-side: re-sync so the
    // toggle offers the download again.
    initBg();
  }
}

function finishBatch(ev, batch) {
  converting = false;
  document.body.classList.remove("converting");

  // Anything not reached before a cancel goes back to ready.
  [...items.values()].forEach((i) => {
    if (i.status === "converting") { i.status = "ready"; renderCard(i); }
  });

  const saved = batch
    .filter((i) => i.status === "done" && i.result)
    .reduce((acc, i) => acc + Math.max(0, (i.result.inBytes ?? 0) - (i.result.outBytes ?? 0)), 0);

  if (ev.cancelled) {
    setStatus(`Cancelled — ${ev.succeeded} done, ${ev.failed} failed`);
  } else if (ev.failed > 0 && ev.succeeded === 0) {
    setStatus(`Failed — ${ev.failed} error${ev.failed > 1 ? "s" : ""}`, "bad");
  } else if (ev.failed > 0) {
    setStatus(`Done with errors — ${ev.succeeded} converted, ${ev.failed} failed`, "bad");
  } else {
    const savings = saved > 0 ? ` · ${fmtBytes(saved)} saved` : "";
    setStatus(`Done — ${ev.succeeded} image${ev.succeeded > 1 ? "s" : ""} processed${savings}`, "good");
  }
  refreshHeader();
  refreshAction();
}

/* ---------- wiring ---------- */

function wire() {
  // Window controls.
  $("win-min").addEventListener("click", () => appWindow.minimize());
  $("win-max").addEventListener("click", () => appWindow.toggleMaximize());
  $("win-close").addEventListener("click", () => appWindow.close());

  // Drop zone: the inner browse button is the single accessible control;
  // clicking anywhere in the zone works too for mouse users.
  els.dropzone.addEventListener("click", browse);
  els.browse.addEventListener("click", (e) => {
    e.stopPropagation();
    browse();
  });

  // Native drag & drop from the OS.
  getCurrentWebview().onDragDropEvent((event) => {
    const kind = event.payload.type;
    if (kind === "enter" || kind === "over") {
      if (!converting) document.body.classList.add("dragging");
    } else if (kind === "leave") {
      document.body.classList.remove("dragging");
    } else if (kind === "drop") {
      document.body.classList.remove("dragging");
      addFiles(event.payload.paths);
    }
  });

  // Block the webview's own file handling (would navigate away).
  window.addEventListener("dragover", (e) => e.preventDefault());
  window.addEventListener("drop", (e) => e.preventDefault());

  // Options.
  document.querySelectorAll('input[name="format"]').forEach((radio) =>
    radio.addEventListener("change", () => {
      moveSegmentThumb();
      syncBgControl();
      saveOptions();
      if (!converting) {
        const ready = readyItems().length;
        setStatus(ready ? readyLine(ready) : idleLine());
      }
    })
  );
  els.compress.addEventListener("change", () => {
    // Make the default explicit: the placeholder becomes a real value.
    if (els.compress.checked && !els.maxKb.value) els.maxKb.value = "500";
    syncCompressField();
    saveOptions();
  });
  els.maxKb.addEventListener("input", () => {
    els.maxKb.value = els.maxKb.value.replace(/\D/g, "");
    saveOptions();
  });
  els.deleteOriginal.addEventListener("change", saveOptions);

  // Instant convert. Arming it never fires a conversion by itself: only the
  // next add does — silently converting an already-sitting queue would surprise.
  els.instant.addEventListener("change", () => {
    saveOptions();
    syncInstantUi();
    if (converting) return;
    const ready = readyItems().length;
    if (instantOn()) {
      setStatus(ready
        ? `Instant convert on — press Convert to run the ${ready} queued image${ready > 1 ? "s" : ""}`
        : "Instant convert on — images convert as soon as you add them");
    } else {
      setStatus(ready ? readyLine(ready) : idleLine());
    }
  });

  // Background removal.
  els.removeBg.addEventListener("change", () => {
    if (els.removeBg.checked && !bgReady) {
      els.removeBg.checked = false;
      if (converting) {
        setStatus("Finish the current batch before setting up background removal");
        return;
      }
      showBgSetup(true);
      return;
    }
    showBgSetup(false);
    saveOptions();
    if (!converting) {
      setStatus(els.removeBg.checked
        ? "Background removal on — output gets a transparent background"
        : idleLine());
    }
  });
  els.bgDownload.addEventListener("click", installBg);
  els.bgCancelSetup.addEventListener("click", () => {
    showBgSetup(false);
    els.removeBg.focus();
  });

  // Queue actions.
  els.clearAll.addEventListener("click", () => {
    if (converting) return;
    items.clear();
    els.list.innerHTML = "";
    setView();
    setStatus(idleLine());
    els.browse.focus();
  });
  els.clearDone.addEventListener("click", () => {
    if (converting) return;
    [...items.values()].forEach((i) => {
      if (i.status === "done") { items.delete(i.path); i.el.remove(); }
    });
    setView();
    (items.size ? els.clearAll : els.browse).focus();
  });

  els.convert.addEventListener("click", convert);

  window.addEventListener("resize", moveSegmentThumb);
}

restoreOptions();
wire();
moveSegmentThumb();
syncBgControl();
syncInstantUi();
setView();
initBg();
setStatus(idleLine());
// Fonts load async; the thumb depends on final label widths.
if (document.fonts?.ready) document.fonts.ready.then(moveSegmentThumb);
