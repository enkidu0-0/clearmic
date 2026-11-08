/* clearmic desktop UI. Talks to the Rust engine through Tauri commands (src-tauri/src/main.rs):
 *   get_status() -> { state: waiting|connected|error, error, delay_ms, nr_db, loss_pct, addr }
 *   get_levels() -> { in: [peak], out: [peak] }
 *   get_pairing() -> { uri, addr, key, qr_svg }       get_params() -> current settings and device list
 *   set_param(name, value)   set_bypass(on)   reconnect()   pair()
 * Opened in a plain browser there is no engine, so a clearly labelled preview signal drives the screen.
 */
(() => {
  const $ = (id) => document.getElementById(id);
  const tauri = window.__TAURI__ && window.__TAURI__.core;
  const call = (cmd, args) => (tauri ? tauri.invoke(cmd, args).catch((e) => { console.error(cmd, e); return null; }) : Promise.resolve(null));
  const reduceMotion = matchMedia("(prefers-reduced-motion: reduce)").matches;

  const states = ["connected", "waiting", "connecting", "error"];
  const params = new URLSearchParams(location.search);
  let state = "waiting";
  const setState = (s, detail) => {
    state = s;
    document.body.dataset.state = s;
    const text = {
      connected: ["Phone connected", detail || ""],
      waiting: ["Waiting for a phone", detail || "No phone sending yet"],
      connecting: ["Connecting", "Looking for your phone"],
      error: ["Connection lost", detail || ""],
    }[s];
    const pill = $("status-pill");
    if (pill.textContent !== text[0]) {
      pill.textContent = text[0];
      pill.classList.remove("pop"); void pill.offsetWidth; pill.classList.add("pop");
    }
    $("status-detail").textContent = text[1];
  };

  /* numbers glide to their new value instead of jumping */
  const tweens = new WeakMap();
  function tween(el, to) {
    const toStr = String(to);
    if (el.textContent === toStr) return;
    const target = parseFloat(toStr), from = parseFloat(el.textContent);
    cancelAnimationFrame(tweens.get(el));
    if (reduceMotion || Number.isNaN(target) || Number.isNaN(from)) { el.textContent = toStr; return; }
    const dec = (toStr.split(".")[1] || "").length, t0 = performance.now(), dur = 600;
    const step = (now) => {
      const p = Math.min(1, (now - t0) / dur), e = 1 - (1 - p) ** 3;
      el.textContent = (from + (target - from) * e).toFixed(dec);
      if (p < 1) tweens.set(el, requestAnimationFrame(step));
    };
    tweens.set(el, requestAnimationFrame(step));
  }

  /* navigation: four tabs, a sliding underline, views that ease out and in */
  const views = ["live", "sound", "phone", "output"];
  const tabs = [...document.querySelectorAll(".tab")];
  const ink = document.querySelector(".tabs__ink");
  let current = null, navId = 0, userNavigated = false;
  function moveInk() {
    const t = document.querySelector('.tab[aria-selected="true"]');
    if (!t) return;
    ink.style.setProperty("--x", t.offsetLeft + 20 + "px");
    ink.style.setProperty("--sx", (t.offsetWidth - 40) / 100);
  }
  function go(name, fromUser) {
    if (!views.includes(name) || name === current) return;
    if (fromUser) userNavigated = true;
    const prev = current && $("view-" + current), next = $("view-" + name), id = ++navId;
    current = name;
    tabs.forEach((t) => { const on = t.dataset.view === name; t.setAttribute("aria-selected", String(on)); t.tabIndex = on ? 0 : -1; });
    moveInk();
    try { history.replaceState(null, "", "#" + name); } catch (_) { /* some webviews block it */ }
    const show = () => {
      if (id !== navId) return;
      views.forEach((v) => { const el = $("view-" + v); el.hidden = v !== name; el.classList.remove("is-out"); });
      next.classList.remove("is-in"); void next.offsetWidth; next.classList.add("is-in");
      if (name === "live") tick();
    };
    if (prev && !reduceMotion) { prev.classList.add("is-out"); setTimeout(show, 180); } else show();
  }
  tabs.forEach((t, i) => {
    t.addEventListener("click", () => go(t.dataset.view, true));
    t.addEventListener("keydown", (e) => {
      const d = e.key === "ArrowRight" ? 1 : e.key === "ArrowLeft" ? -1 : 0;
      if (!d) return;
      e.preventDefault();
      const n = tabs[(i + d + tabs.length) % tabs.length];
      n.focus(); go(n.dataset.view, true);
    });
  });
  addEventListener("resize", moveInk);
  if (document.fonts && document.fonts.ready) document.fonts.ready.then(moveInk);
  $("btn-show-pair").addEventListener("click", () => go("phone", true));

  if (!tauri) {
    $("preview-note").hidden = false;
    setState(states.includes(params.get("state")) ? params.get("state") : "connected", "Preview");
  }
  if (params.has("dev")) $("dev").hidden = false;

  /* controls */
  const strength = $("strength");
  const paintStrength = () => {
    strength.style.setProperty("--fill", strength.value + "%");
    $("strength-out").textContent = strength.value + "%";
  };
  strength.addEventListener("input", () => { paintStrength(); call("set_param", { name: "strength", value: +strength.value / 100 }); });
  paintStrength();

  async function loadParams() {
    const p = await call("get_params");
    if (!p) return;
    strength.value = Math.round(p.strength * 100); paintStrength();
    const pre = document.querySelector(`input[name="preset"][value="${p.preset}"]`); if (pre) pre.checked = true;
    const eng = document.querySelector(`input[name="performance"][value="${p.performance}"]`); if (eng) eng.checked = true;
    paintPerf(p.performance);
    $("boot-row").hidden = !p.autostart_supported;
    $("c-boot").checked = p.autostart;
    $("c-monitor").checked = !!p.monitor;
    for (const k of ["hpf", "gate", "eq", "comp", "deess"]) { const el = document.querySelector(`input[name="${k}"]`); if (el) el.checked = p[k]; }
    const sel = $("out-dev");
    const cur = (p.output || "").toLowerCase();
    const opts = p.outputs.map((o) => new Option(o, o, false, cur !== "" && o.toLowerCase().includes(cur)));
    if (cur.startsWith("wav:")) opts.push(new Option("File " + p.output.slice(4), p.output, true, true));
    sel.replaceChildren(new Option("Choose where the sound goes", "", false, cur === ""), ...opts);
  }

  document.querySelectorAll('input[name="preset"]').forEach((el) =>
    el.addEventListener("change", async () => { await call("set_param", { name: "preset", value: el.value }); loadParams(); }));
  const perfText = {
    efficient: "Lightest on the CPU. Best while you game.",
    balanced: "A little more CPU for a little more detail.",
    quality: "Best sound. Uses the most CPU.",
    off: "No noise removal, only the voice chain.",
  };
  const paintPerf = (m) => { $("perf-help").textContent = perfText[m] || ""; };
  document.querySelectorAll('input[name="performance"]').forEach((el) =>
    el.addEventListener("change", async () => { paintPerf(el.value); await call("set_param", { name: "performance", value: el.value }); loadParams(); }));
  $("c-boot").addEventListener("change", async (e) => {
    const r = await call("set_param", { name: "autostart", value: e.target.checked });
    if (r === null && tauri) loadParams(); // refused: show the real setting again
  });
  $("btn-hide").addEventListener("click", () => call("hide_to_tray"));
  document.querySelectorAll(".switch").forEach((el) =>
    el.addEventListener("change", () => call("set_param", { name: el.name, value: el.checked })));
  $("out-dev").addEventListener("change", async (e) => { if (e.target.value) { await call("set_param", { name: "output", value: e.target.value }); refresh(); } });

  /* hold to hear the original: pointer or keyboard */
  const compare = $("btn-compare");
  const bypass = (on) => { compare.setAttribute("aria-pressed", String(on)); call("set_bypass", { on }); };
  compare.addEventListener("pointerdown", () => bypass(true));
  ["pointerup", "pointerleave", "blur"].forEach((ev) => compare.addEventListener(ev, () => bypass(false)));
  compare.addEventListener("keydown", (e) => { if (e.key === " " || e.key === "Enter") bypass(true); });
  compare.addEventListener("keyup", (e) => { if (e.key === " " || e.key === "Enter") bypass(false); });

  $("btn-retry").addEventListener("click", async () => { setState("connecting"); await call("reconnect"); refresh(); });
  $("btn-pair").addEventListener("click", async () => { await call("pair"); loadPairing(); refresh(); });
  $("btn-usb").addEventListener("click", () => alert("USB mode is not built yet. Use Wi-Fi for now."));

  /* pairing code */
  async function loadPairing() {
    const p = await call("get_pairing");
    if (!p) return;
    $("pair-addr").textContent = p.addr;
    $("pair-key").textContent = p.key;
    const box = document.querySelector(".pair__qr");
    box.innerHTML = p.qr_svg; // generated by our own backend from the pairing address and key
    box.setAttribute("aria-label", "Pairing code, scan it with the ClearMic phone app");
  }

  /* lanes */
  const cv = { in: $("wave-in"), out: $("wave-out") };
  const css = getComputedStyle(document.documentElement);
  const col = (n) => css.getPropertyValue(n).trim();
  const BAR = 3, GAP = 2;
  const history = { in: [], out: [] };

  function draw(kind, bypassOn) {
    const c = cv[kind];
    const dpr = devicePixelRatio || 1;
    const w = c.clientWidth, h = c.clientHeight;
    if (!w) return;
    if (c.width !== Math.round(w * dpr)) { c.width = Math.round(w * dpr); c.height = Math.round(h * dpr); }
    const g = c.getContext("2d");
    g.setTransform(dpr, 0, 0, dpr, 0, 0);
    g.clearRect(0, 0, w, h);
    const n = Math.floor(w / (BAR + GAP));
    const arr = history[kind].slice(-n);
    g.fillStyle = kind === "in" ? col("--color-muted") : (!bypassOn ? col("--color-accent") : col("--color-muted"));
    arr.forEach((v, i) => {
      const bh = Math.max(2, Math.min(1, v) * h);
      g.globalAlpha = 0.25 + 0.75 * ((i + 1) / arr.length); // older bars fade into the bed
      g.beginPath();
      g.roundRect(w - (arr.length - i) * (BAR + GAP), (h - bh) / 2, BAR, bh, 1.5);
      g.fill();
    });
    g.globalAlpha = 1;
  }

  let t = 0;
  const preview = () => {
    t += 1;
    const talking = Math.sin(t / 22) + Math.sin(t / 7.3) * 0.5 > 0.1;
    const voice = talking ? 0.25 + 0.55 * Math.abs(Math.sin(t / 3.1)) * Math.random() ** 0.5 : 0;
    const noise = 0.12 + 0.1 * Math.random();
    const s = +strength.value / 100;
    return { in: Math.min(1, voice + noise), out: Math.min(1, voice + noise * (1 - s) * 0.8) };
  };

  async function tick() {
    const live = tauri ? await call("get_levels") : null;
    const v = live ? { in: live.in.at(-1) ?? 0, out: live.out.at(-1) ?? 0 } : preview();
    const lastIn = history.in.at(-1) ?? v.in, lastOut = history.out.at(-1) ?? v.out;
    history.in.push(lastIn * 0.5 + v.in * 0.5); history.out.push(lastOut * 0.5 + v.out * 0.5); // eased so the bars flow
    if (history.in.length > 600) { history.in.shift(); history.out.shift(); }
    if (state === "connected" && current === "live") {
      const b = compare.getAttribute("aria-pressed") === "true";
      draw("in", b); draw("out", b);
    }
  }

  async function refresh() {
    const s = await call("get_status");
    if (s) {
      setState(s.state, s.state === "error" ? s.error : s.addr);
      tween($("fig-delay"), s.delay_ms);
      tween($("fig-nr"), s.nr_db);
      tween($("fig-loss"), s.loss_pct);
      tween($("fig-cpu"), s.load_pct);
      $("perf-note").hidden = !s.stepped_down;
      const mon = $("c-monitor").checked;
      $("mon-note").hidden = !mon;
      if (mon) $("mon-note").textContent = s.monitor_info || "Plays your voice here while the phone is sending.";
      $("out-note").hidden = !s.notice;
      if (s.notice) $("out-note").textContent = s.notice;
      if (s.state === "waiting" && !userNavigated && current === "live") go("phone");
      if (s.state === "error" && s.error) {
        $("fail-text").textContent = s.error;
        $("fail-title").textContent = s.error.startsWith("the phone stopped") ? "The phone stopped sending" : s.error.startsWith("the phone is sending") ? "Pairing code does not match" : "ClearMic could not start";
      }
      $("lvl-in").textContent = "Raw";
      $("lvl-out").textContent = "Cleaned";
    } else {
      ["fig-delay", "fig-nr", "fig-loss", "fig-cpu"].forEach((id) => ($(id).textContent = "--"));
      $("lvl-in").textContent = "Preview";
      $("lvl-out").textContent = "Preview";
    }
  }

  if (reduceMotion) {
    for (let i = 0; i < 400; i++) tick();
  } else {
    let last = 0;
    const loop = (now) => { if (now - last > 33) { last = now; tick(); } requestAnimationFrame(loop); };
    requestAnimationFrame(loop);
  }
  /* a small secret: seven quick taps on the logo */
  (() => {
    let taps = [];
    document.querySelector(".brand").addEventListener("click", () => {
      const now = Date.now();
      taps = taps.filter((t) => now - t < 3000).concat(now);
      if (taps.length < 7) return;
      taps = [];
      const note = document.createElement("div");
      note.className = "egg";
      note.setAttribute("role", "status");
      note.textContent = "Made by Afif. Nice find.";
      document.body.append(note);
      setTimeout(() => note.remove(), 3200);
    });
  })();
  console.log("%cclearmic", "font:600 14px system-ui;color:#146B67", "psst: atob('QWZpZiB3YXMgaGVyZQ==')");

  if (tauri) { loadPairing(); loadParams(); }
  go(views.includes(location.hash.slice(1)) ? location.hash.slice(1) : "live");
  refresh(); setInterval(refresh, 500);
  addEventListener("resize", () => { if (state === "connected" && current === "live") tick(); });
})();
