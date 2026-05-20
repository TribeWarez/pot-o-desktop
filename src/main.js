import { invoke } from "@tauri-apps/api/core";
import "./styles.css";

let config = {};
let dashboardData = {};
let dashboardTimer = null;
let miningTimer = null;

// ── Navigation ───────────────────────────────────────────

document.addEventListener("click", (e) => {
  const btn = e.target.closest("[data-tab]");
  if (!btn) return;
  document.querySelectorAll("[data-tab]").forEach(b => b.classList.remove("active"));
  btn.classList.add("active");
  document.querySelectorAll(".tab").forEach(t => t.classList.remove("active"));
  document.getElementById("tab_" + btn.dataset.tab).classList.add("active");
  if (btn.dataset.tab === "dashboard") startDashboard();
  else stopDashboard();
});

// ── Settings ─────────────────────────────────────────────

async function loadConfig() {
  config = await invoke("get_config");
  renderSettings();
}

async function saveConfig() {
  gatherFormValues();
  await invoke("save_config", { config });
  showStatus("Settings saved", "ok");
}

function gatherFormValues() {
  config.rpc_url = document.getElementById("rpc_url").value;
  config.status_url = document.getElementById("status_url").value;
  config.solana_rpc_url = document.getElementById("solana_rpc_url").value;
  config.miner_pubkey = document.getElementById("miner_pubkey").value;
  config.max_iterations = parseInt(document.getElementById("max_iterations").value) || 10000;
  config.max_tensor_dim = parseInt(document.getElementById("max_tensor_dim").value) || 256;
  config.loop_delay = parseInt(document.getElementById("loop_delay").value) || 2;
  config.operation = document.getElementById("operation").value;
  config.path_layers = document.getElementById("path_layers").value;
  config.mml_threshold = document.getElementById("mml_threshold").value;
  config.device_type = document.getElementById("device_type").value;
  config.device_id = document.getElementById("device_id").value;
  config.hexchain_mode = document.getElementById("hexchain_mode").checked;
  config.explain = document.getElementById("explain").checked;
  config.verbose = document.getElementById("verbose").checked;
}

function showStatus(msg, type) {
  const el = document.getElementById("status_msg");
  el.textContent = msg;
  el.className = "status " + type;
  setTimeout(() => { el.textContent = ""; el.className = "status"; }, 3000);
}

function testConnection() {
  gatherFormValues();
  invoke("rpc_get", { path: "/health" })
    .then((resp) => {
      const svc = resp.service || "unknown";
      const ver = resp.version || "?";
      showStatus(`RPC OK: ${svc} v${ver}`, "ok");
    })
    .catch((e) => {
      showStatus(`RPC error: ${e}`, "err");
    });
}

function renderSettings() {
  const app = document.getElementById("app");
  app.innerHTML = `
    <div class="container">
      <header>
        <h1>Pot-O Desktop</h1>
        <nav>
          <button data-tab="settings">Settings</button>
          <button class="active" data-tab="dashboard">Dashboard</button>
          <button data-tab="keypair">Keys</button>
        </nav>
      </header>
      <div id="status_msg" class="status"></div>

      <div id="tab_keypair" class="tab">
        <section>
          <h2>Keypair Manager</h2>
          <div class="row">
            <label>Keypair File <input id="kp_path" value="${esc(config.miner_json_path || '')}" placeholder="~/pot-o-miner-cli/mineri.json" /></label>
            <button style="margin-top:18px" onclick="loadKeypair()">Load</button>
          </div>
          <div id="kp_info"></div>
          <div class="actions">
            <button class="primary" onclick="generateKeypair()">Generate New Keypair</button>
            <button onclick="document.getElementById('kp_path').value=''; loadKeypair();">Clear</button>
          </div>
        </section>
        <section>
          <h2>Pubkey Identity</h2>
          <p style="font-size:0.82rem;color:#999;margin-bottom:8px">Set this keypair's pubkey as your miner identity in Settings.</p>
          <div class="row">
            <label>Miner Pubkey <input id="kp_pubkey_display" readonly placeholder="Load a keypair to see pubkey" /></label>
            <button style="margin-top:18px" onclick="setPubkeyFromKeypair()">Use as Identity</button>
          </div>
        </section>
      </div>

      <div id="tab_settings" class="tab">
        <section>
          <h2>Connection</h2>
          <label>RPC URL <input id="rpc_url" value="${esc(config.rpc_url)}" /></label>
          <label>Status URL <input id="status_url" value="${esc(config.status_url)}" /></label>
          <label>Solana RPC URL <input id="solana_rpc_url" value="${esc(config.solana_rpc_url)}" placeholder="Optional" /></label>
          <button onclick="testConnection()">Test Connection</button>
        </section>
        <section>
          <h2>Identity</h2>
          <label>Miner Pubkey <input id="miner_pubkey" value="${esc(config.miner_pubkey)}" placeholder="Solana pubkey or identity string" /></label>
        </section>
        <section>
          <h2>Mining Parameters</h2>
          <div class="row">
            <label>Max Iterations <input id="max_iterations" type="number" value="${config.max_iterations}" /></label>
            <label>Max Tensor Dim <input id="max_tensor_dim" type="number" value="${config.max_tensor_dim}" /></label>
            <label>Loop Delay (s) <input id="loop_delay" type="number" value="${config.loop_delay}" /></label>
          </div>
          <div class="row">
            <label>Operation
              <select id="operation">
                <option value="">(Default)</option>
                <option value="matrix_multiply" ${sel("matrix_multiply")}>Matrix Multiply</option>
                <option value="convolution" ${sel("convolution")}>Convolution</option>
                <option value="relu" ${sel("relu")}>ReLU</option>
                <option value="sigmoid" ${sel("sigmoid")}>Sigmoid</option>
                <option value="tanh" ${sel("tanh")}>Tanh</option>
                <option value="dot_product" ${sel("dot_product")}>Dot Product</option>
                <option value="normalize" ${sel("normalize")}>Normalize</option>
              </select>
            </label>
            <label>Path Layers <input id="path_layers" value="${esc(config.path_layers)}" placeholder="32,16,8" /></label>
            <label>MML Threshold <input id="mml_threshold" value="${esc(config.mml_threshold)}" placeholder="(Challenge default)" /></label>
          </div>
        </section>
        <section>
          <h2>Device</h2>
          <div class="row">
            <label>Device Type
              <select id="device_type">
                <option value="cpu" ${seld("cpu")}>CPU</option>
                <option value="gpu" ${seld("gpu")}>GPU</option>
                <option value="esp32" ${seld("esp32")}>ESP32</option>
                <option value="native" ${seld("native")}>Native</option>
              </select>
            </label>
            <label>Device ID <input id="device_id" value="${esc(config.device_id)}" placeholder="(Optional)" /></label>
          </div>
        </section>
        <section>
          <h2>Mode</h2>
          <label class="checkbox"><input id="hexchain_mode" type="checkbox" ${config.hexchain_mode ? "checked" : ""} /> Hexchain Lattice PoW Mode</label>
        </section>
        <section>
          <h2>Debug</h2>
          <label class="checkbox"><input id="explain" type="checkbox" ${config.explain ? "checked" : ""} /> Explain</label>
          <label class="checkbox"><input id="verbose" type="checkbox" ${config.verbose ? "checked" : ""} /> Verbose</label>
        </section>
        <div class="actions">
          <button class="primary" onclick="saveConfig()">Save Settings</button>
        </div>
      </div>

      <div id="tab_dashboard" class="tab active"></div>
    </div>
  `;
  // Re-bind tab clicks (they were lost on innerHTML replace)
  document.querySelectorAll("[data-tab]").forEach(b => {
    b.addEventListener("click", (e) => {
      document.querySelectorAll("[data-tab]").forEach(x => x.classList.remove("active"));
      b.classList.add("active");
      document.querySelectorAll(".tab").forEach(t => t.classList.remove("active"));
      document.getElementById("tab_" + b.dataset.tab).classList.add("active");
      if (b.dataset.tab === "dashboard") startDashboard();
      else stopDashboard();
    });
  });
  // Start dashboard by default
  startDashboard();
}

// ── Dashboard ────────────────────────────────────────────

function startDashboard() {
  renderDashboard();
  if (!dashboardTimer) {
    dashboardTimer = setInterval(refreshDashboard, 5000);
  }
  refreshDashboard();
}

function stopDashboard() {
  if (dashboardTimer) { clearInterval(dashboardTimer); dashboardTimer = null; }
  if (miningTimer) { clearInterval(miningTimer); miningTimer = null; }
}

async function refreshDashboard() {
  if (!document.getElementById("tab_dashboard")) return;
  try {
    const [gateway, apiLive, pool, peers, devices, miner, stats] = await Promise.all([
      safeFetch("/status", false),
      safeFetch("/api/live", false),
      safeFetch("/pool", true),
      safeFetch("/network/peers", true),
      safeFetch("/devices", true),
      config.miner_pubkey ? safeFetch("/miners/" + encodeURIComponent(config.miner_pubkey), true) : null,
      invoke("get_mining_stats"),
    ]);
    dashboardData.gateway = gateway;
    dashboardData.apiLive = apiLive;
    dashboardData.pool = pool;
    dashboardData.peers = peers;
    dashboardData.devices = devices;
    dashboardData.miner = miner;
    dashboardData.stats = stats;

    if (config.hexchain_mode) {
      const [hstatus, hlattice] = await Promise.all([
        safeFetch("/hexchain/status", true),
        safeFetch("/hexchain/lattice", true),
      ]);
      dashboardData.hexStatus = hstatus;
      dashboardData.hexLattice = hlattice;
    }

    renderDashboard();
  } catch (e) {
    console.error("Dashboard refresh error:", e);
  }
}

async function safeFetch(path, isPot) {
  try {
    if (isPot) return await invoke("rpc_get", { path });
    else return await invoke("status_api_get", { path });
  } catch {
    return { _error: "fetch failed" };
  }
}

function renderDashboard() {
  const el = document.getElementById("tab_dashboard");
  if (!el) return;
  const d = dashboardData;
  const stats = d.stats || {};
  const running = stats.running || false;

  let html = `<div class="container">`;
  // Header + mining controls
  html += `
    <header>
      <h1>Dashboard</h1>
      <div class="mining-controls">
        <span class="mining-status ${running ? 'running' : ''}">${running ? '● Mining' : '○ Idle'}</span>
        <button class="${running ? 'btn-stop' : 'btn-start'}" onclick="${running ? 'doStopMining()' : 'doStartMining()'}">
          ${running ? 'Stop' : 'Start'} Mining
        </button>
      </div>
    </header>`;

  // Stats bar
  html += `<div class="stats-bar">
    <div class="stat"><span class="num">${stats.challenges || 0}</span> Challenges</div>
    <div class="stat"><span class="num">${stats.proofs_found || 0}</span> Found</div>
    <div class="stat"><span class="num">${stats.proofs_submitted || 0}</span> Submitted</div>
    <div class="stat"><span class="num">${stats.proofs_accepted || 0}</span> Accepted</div>
    <div class="stat"><span class="num">${stats.start_time ? fmtDuration(Math.floor(Date.now()/1000) - stats.start_time) : '—'}</span> Uptime</div>
  </div>`;

  // Gateway services
  html += `<section><h2>Gateway Services</h2>`;
  const gs = d.gateway || {};
  if (gs._error) {
    html += `<p class="err">${gs._error}</p>`;
  } else {
    const summary = gs.summary || {};
    html += `<div class="summary-line">ok=${summary.up||0} degraded=${summary.degraded||0} down=${summary.down||0} total=${summary.total||0}</div>`;
    const services = gs.services || [];
    if (services.length) {
      html += `<table><tr><th>Service</th><th>Status</th><th>Latency</th><th>URL</th></tr>`;
      for (const s of services.slice(0, 8)) {
        const st = s.status || '?';
        const cls = st === 'up' ? 'st-up' : st === 'degraded' ? 'st-deg' : 'st-down';
        html += `<tr><td>${esc(s.id||'')}</td><td class="${cls}">${st}</td><td>${s.latency_ms != null ? s.latency_ms+'ms' : '—'}</td><td class="url">${esc((s.url||'').slice(0,40))}</td></tr>`;
      }
      html += `</table>`;
    }
  }
  html += `</section>`;

  // PoT-O Live
  html += `<section><h2>PoT-O Validator</h2>`;
  const live = d.apiLive;
  if (!live || live._error) {
    html += `<p class="err">${live ? live._error : 'No data'}</p>`;
  } else {
    const pot = live.pot_o || live;
    html += `<div class="grid-2">
      <div><strong>Node ID:</strong> ${esc(String(pot.node_id||'—').slice(0,28))}</div>
      <div><strong>Difficulty:</strong> ${pot.difficulty ?? '—'}</div>
      <div><strong>Max Tensor Dim:</strong> ${pot.max_tensor_dim ?? '—'}</div>
      <div><strong>Network:</strong> ${pot.peer_network_mode ?? '—'}</div>
      <div><strong>Pool:</strong> ${pot.pool_strategy ?? '—'}</div>`;
    const st = pot.stats || {};
    html += `<div><strong>Challenges:</strong> ${st.total_challenges_issued ?? '—'}</div>
      <div><strong>Proofs Valid:</strong> ${st.total_proofs_valid ?? '—'}</div>`;
    const eng = pot.engine || {};
    html += `<div><strong>Engine tasks:</strong> ${eng.tasks_processed ?? '—'}</div>
      <div><strong>OK/Fail:</strong> ${eng.successful ?? '—'}/${eng.failed ?? '—'}</div>`;
    const net = pot.network || {};
    html += `<div><strong>Network Nodes:</strong> ${net.total_nodes ?? '—'}</div>
      <div><strong>Synced:</strong> ${net.synced ?? '—'}</div>`;
    html += `</div>`;
    // current challenge
    const ch = pot.current_challenge || {};
    if (ch.id) {
      html += `<div class="challenge-line"><strong>Current Challenge:</strong> id=${esc(String(ch.id).slice(0,24))} slot=${ch.slot ?? '?'}</div>`;
    }
    // miners by device
    const mbd = pot.miners_by_device || (d.apiLive && d.apiLive.miners_by_device);
    if (mbd && typeof mbd === 'object') {
      html += `<div class="miners-line"><strong>Miners by device:</strong> `;
      for (const [k, v] of Object.entries(mbd)) {
        const cnt = typeof v === 'object' ? (v.count ?? 0) : v;
        html += `${k}:${cnt} `;
      }
      html += `</div>`;
    }
  }
  html += `</section>`;

  // Hexchain (if enabled)
  if (config.hexchain_mode) {
    html += `<section><h2>Hexchain Lattice</h2>`;
    const hs = d.hexStatus || {};
    if (hs._error) {
      html += `<p class="err">${hs._error}</p>`;
    } else {
      html += `<div class="grid-2">
        <div><strong>Occupied:</strong> ${hs.occupied_coords ?? '—'}</div>
        <div><strong>Depth:</strong> ${hs.latest_depth ?? '—'}</div>`;
      const hch = hs.current_challenge || {};
      if (hch.id) {
        html += `<div><strong>Challenge:</strong> id=${esc(String(hch.id).slice(0,24))} coord=${JSON.stringify(hch.coord||{})}</div>`;
      }
      html += `</div>`;
      const hblocks = (d.hexLattice && d.hexLattice.blocks) || [];
      if (hblocks.length) {
        html += `<table><tr><th>Coord</th><th>Depth</th><th>Hash</th></tr>`;
        for (const b of hblocks.slice(0, 5)) {
          const hash = b.block_hash || '?';
          html += `<tr><td>${JSON.stringify(b.coord||{})}</td><td>${b.depth??'?'}</td><td class="mono">${esc(String(hash).slice(0,16))}</td></tr>`;
        }
        html += `</table>`;
      }
    }
    html += `</section>`;
  }

  // Pool
  html += `<section><h2>Pool</h2>`;
  const pool = d.pool || {};
  if (pool._error) {
    html += `<p class="err">${pool._error}</p>`;
  } else {
    html += `<div class="grid-2">
      <div><strong>Type:</strong> ${pool.pool_type || pool.type || '—'}</div>
      <div><strong>Miners:</strong> ${pool.total_miners || pool.miners || '—'}</div>
      <div><strong>Stake:</strong> ${pool.total_stake || pool.stake || '—'}</div>
      <div><strong>Min Stake:</strong> ${pool.minimum_stake ?? '—'}</div>
    </div>`;
  }
  html += `</section>`;

  // Miner info
  if (config.miner_pubkey) {
    html += `<section><h2>Miner (${esc(config.miner_pubkey.slice(0,20))}...)</h2>`;
    const min = d.miner;
    if (!min || min._error) {
      const err = min ? min._error : 'No data';
      const is404 = String(err).includes('404');
      html += `<p class="${is404 ? 'dim' : 'err'}">${is404 ? 'Not on-chain yet' : esc(err)}</p>`;
    } else {
      html += `<pre class="json">${esc(JSON.stringify(min, null, 2))}</pre>`;
    }
    html += `</section>`;
  }

  // Peers
  html += `<section><h2>Network Peers</h2>`;
  const peers = d.peers;
  if (!peers || peers._error) {
    html += `<p class="dim">${peers ? peers._error : 'No data'}</p>`;
  } else {
    const list = Array.isArray(peers) ? peers : (peers.peers || []);
    if (list.length) {
      html += `<ul class="peer-list">`;
      for (const p of list.slice(0, 10)) {
        const s = typeof p === 'string' ? p : JSON.stringify(p);
        html += `<li>${esc(s.slice(0,60))}</li>`;
      }
      html += `</ul>`;
    } else {
      html += `<p class="dim">(none or local_only)</p>`;
    }
  }
  html += `</section>`;

  html += `</div>`;
  el.innerHTML = html;
}

// ── Mining controls ──────────────────────────────────────

async function doStartMining() {
  await invoke("start_mining");
  renderDashboard();
  runMiningLoop();
}

async function doStopMining() {
  await invoke("stop_mining");
  if (miningTimer) { clearInterval(miningTimer); miningTimer = null; }
  renderDashboard();
}

async function runMiningLoop() {
  const stats = await invoke("get_mining_stats");
  if (!stats.running) return;

  // Fetch challenge
  try {
    const challenge = await invoke("rpc_post", {
      path: "/challenge",
      body: { device_type: config.device_type || "cpu" },
    });

    if (challenge && challenge.id) {
      stats.challenges++;
      stats.last_challenge_id = challenge.id || "";
      await invoke("set_mining_stats", { stats });

      // Run mining engine
      const result = config.hexchain_mode
        ? await invoke("mine_hexchain", { challenge })
        : await invoke("mine_pot_o", { challenge });

      if (result.status === "proof_found") {
        stats.proofs_found++;
        await invoke("set_mining_stats", { stats });

        // Submit proof
        try {
          const submitResp = await invoke("rpc_post", {
            path: "/submit",
            body: { proof: result.proof },
          });
          stats.proofs_submitted++;
          if (submitResp && submitResp.accepted) {
            stats.proofs_accepted++;
          }
          await invoke("set_mining_stats", { stats });
        } catch (e) {
          console.error("Submit failed:", e);
        }
      }
    }
  } catch (e) {
    console.error("Mining cycle error:", e);
  }

  // Schedule next cycle
  const delay = (config.loop_delay || 2) * 1000;
  miningTimer = setTimeout(runMiningLoop, delay);
  renderDashboard();
}

// ── Helpers ──────────────────────────────────────────────

function esc(s) {
  return String(s).replace(/&/g, "&amp;").replace(/"/g, "&quot;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}
function sel(v) { return config.operation === v ? "selected" : ""; }
function seld(v) { return config.device_type === v ? "selected" : ""; }
function fmtDuration(s) {
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const sec = s % 60;
  return `${String(h).padStart(2,'0')}:${String(m).padStart(2,'0')}:${String(sec).padStart(2,'0')}`;
}

// ── Keypair management ───────────────────────────────────

async function generateKeypair() {
  const defaultPath = config.miner_json_path || osDefaultKeypath();
  const path = prompt("Save keypair to:", defaultPath);
  if (!path) return;
  try {
    const info = await invoke("generate_keypair", { path });
    document.getElementById("kp_path").value = path;
    renderKeypairInfo(info);
    showStatus(`Keypair generated: ${info.pubkey.slice(0,16)}...`, "ok");
  } catch (e) {
    showStatus(`Generation failed: ${e}`, "err");
  }
}

async function loadKeypair() {
  const path = document.getElementById("kp_path").value;
  if (!path) { document.getElementById("kp_info").innerHTML = ""; return; }
  try {
    const info = await invoke("read_keypair", { path });
    renderKeypairInfo(info);
  } catch (e) {
    document.getElementById("kp_info").innerHTML = `<p class="err">${esc(e)}</p>`;
  }
}

function renderKeypairInfo(info) {
  const el = document.getElementById("kp_info");
  const isKp = info.is_keypair;
  const icon = isKp ? "🔑" : "👤";
  el.innerHTML = `
    <div class="kp-detail">
      <div class="kp-icon">${icon}</div>
      <div>
        <strong>${isKp ? 'Full Keypair (64 bytes)' : 'Public Key (32 bytes)'}</strong><br/>
        <span class="mono">${esc(info.pubkey)}</span><br/>
        <span class="dim">${esc(info.path)}</span>
      </div>
    </div>
  `;
  document.getElementById("kp_pubkey_display").value = info.pubkey;
  // Check if 64-byte keypair (never send as signature)
  if (isKp) {
    invoke("is_keypair_file", { path: info.path }).then(isKpFile => {
      if (isKpFile) {
        el.innerHTML += `<p class="warn">⚠️ 64-byte Solana keypair — never use as proof signature</p>`;
      }
    });
  }
}

async function setPubkeyFromKeypair() {
  const pubkey = document.getElementById("kp_pubkey_display").value;
  if (!pubkey) { showStatus("No pubkey loaded", "err"); return; }
  document.getElementById("miner_pubkey").value = pubkey;
  document.querySelector('[data-tab="settings"]').click();
  showStatus("Pubkey set — save settings to persist", "ok");
}

function osDefaultKeypath() {
  const home = typeof process !== 'undefined' && process.env?.HOME ? process.env.HOME : '~';
  return home + '/pot-o-miner-cli/miner.json';
}

// ── Init ─────────────────────────────────────────────────

document.addEventListener("DOMContentLoaded", loadConfig);
