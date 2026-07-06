import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import "./styles.css";

let config = {};
let dashboardData = {};
let dashboardTimer = null;
let miningTimer = null;
let lastMiningErrorToast = 0;
let wsEventUnlisten = [];
let wsListenersSetup = false;
let wsChallengeHandler = null;
let wsConnected = false;
let walletData = {};
let walletTimer = null;
let traceTimer = null;
let walletLoggedIn = false;
let walletAccounts = [];

const TOAST_MS = 4000;
const MINING_ERROR_DEBOUNCE_MS = 10000;

// ── Toasts ───────────────────────────────────────────────

function initToasts() {
  if (document.getElementById("toast-container")) return;
  const container = document.createElement("div");
  container.id = "toast-container";
  container.className = "toast-container";
  document.body.appendChild(container);
}

function showToast(message, type = "info") {
  initToasts();
  const container = document.getElementById("toast-container");
  const toast = document.createElement("div");
  const cls = type === "success" || type === "ok" ? "toast-success"
    : type === "error" || type === "err" ? "toast-error"
    : "toast-info";
  toast.className = `toast ${cls}`;
  toast.textContent = message;
  container.appendChild(toast);
  setTimeout(() => toast.remove(), TOAST_MS);
}

function showMiningError(message) {
  const now = Date.now();
  if (now - lastMiningErrorToast < MINING_ERROR_DEBOUNCE_MS) return;
  lastMiningErrorToast = now;
  showToast(message, "error");
}

function formatError(e) {
  return String(e?.message ?? e);
}

// ── Navigation & actions (event delegation) ────────────

function switchTab(tab) {
  document.querySelectorAll("[data-tab]").forEach(b => {
    b.classList.toggle("active", b.dataset.tab === tab);
  });
  document.querySelectorAll(".tab").forEach(t => t.classList.remove("active"));
  const panel = document.getElementById("tab_" + tab);
  if (panel) panel.classList.add("active");
  if (tab === "dashboard") { startDashboard(); stopWallet(); stopTracePolling(); }
  else if (tab === "wallet") { startWallet(); stopDashboard(); stopTracePolling(); }
  else if (tab === "logs") { renderLogsTab(); stopDashboard(); stopWallet(); stopTracePolling(); }
  else if (tab === "trace") { startTracePolling(); stopDashboard(); stopWallet(); }
  else { stopDashboard(); stopWallet(); stopTracePolling(); }
}

document.addEventListener("click", async (e) => {
  const tabBtn = e.target.closest("[data-tab]");
  if (tabBtn) {
    switchTab(tabBtn.dataset.tab);
    return;
  }

  const toggleBtn = e.target.closest("[data-toggle]");
  if (toggleBtn) {
    const targetId = toggleBtn.dataset.toggle;
    const target = document.getElementById(targetId);
    if (target) {
      const expanded = target.style.display !== "none";
      target.style.display = expanded ? "none" : "";
      toggleBtn.classList.toggle("collapsed", expanded);
    }
    return;
  }

  const actionBtn = e.target.closest("[data-action]");
  if (!actionBtn) return;

  const action = actionBtn.dataset.action;
  try {
    switch (action) {
      case "save-config":
        await saveConfig();
        break;
      case "test-connection":
        await testConnection();
        break;
      case "load-keypair":
        await loadKeypair();
        break;
      case "generate-keypair":
        await generateKeypair();
        break;
      case "clear-keypair": {
        const pathEl = document.getElementById("kp_path");
        if (pathEl) pathEl.value = "";
        const infoEl = document.getElementById("kp_info");
        if (infoEl) infoEl.innerHTML = "";
        const pubEl = document.getElementById("kp_pubkey_display");
        if (pubEl) pubEl.value = "";
        break;
      }
      case "set-pubkey":
        await setPubkeyFromKeypair();
        break;
      case "start-mining":
        await doStartMining();
        break;
      case "stop-mining":
        await doStopMining();
        break;
      case "ws-connect":
        await doWsConnect();
        break;
      case "ws-disconnect":
        await doWsDisconnect();
        break;
      case "register-device":
        await doRegisterDevice();
        break;
      case "wallet-refresh":
        await refreshWallet();
        break;
      case "wallet-transfer":
        await doWalletTransfer();
        break;
      case "wallet-set-address":
        await setWalletAddress();
        break;
      case "wallet-gateway-login":
        await doWalletGatewayLogin();
        break;
      case "wallet-gateway-logout":
        await doWalletGatewayLogout();
        break;
      case "wallet-gateway-list":
        await doWalletGatewayList();
        break;
      case "refresh-logs":
        await refreshLogs();
        break;
      case "clear-logs":
        await clearLogs();
        break;
      default:
        break;
    }
  } catch (err) {
    showToast(formatError(err), "error");
  }
});

// ── Settings ─────────────────────────────────────────────

async function loadConfig() {
  try {
    config = await invoke("get_config");
    wsConnected = await invoke("ws_is_connected");
    renderSettings();
  } catch (e) {
    showToast(`Failed to load config: ${formatError(e)}`, "error");
  }
}

async function saveConfig() {
  gatherFormValues();
  await invoke("save_config", { config });
  showToast("Settings saved", "success");
}

function gatherFormValues() {
  config.rpc_url = document.getElementById("rpc_url").value;
  config.status_url = document.getElementById("status_url").value;
  config.ws_url = document.getElementById("ws_url").value;
  config.wallet_base_url = document.getElementById("wallet_base_url").value;
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
  config.peer_network_mode = document.getElementById("peer_network_mode").value;
  config.pool_strategy = document.getElementById("pool_strategy").value;
  config.bootstrap_urls = document.getElementById("bootstrap_urls").value.split("\n").map(s => s.trim()).filter(Boolean);
  config.enable_mdns = document.getElementById("enable_mdns").checked;
  config.mdns_service_name = document.getElementById("mdns_service_name").value;
  config.peer_timeout_secs = parseInt(document.getElementById("peer_timeout_secs").value) || 30;
  config.challenge_relay_enabled = document.getElementById("challenge_relay_enabled").checked;
  config.explain = document.getElementById("explain").checked;
  config.verbose = document.getElementById("verbose").checked;
}

async function testConnection() {
  gatherFormValues();
  try {
    const resp = await invoke("rpc_get", { path: "/health" });
    const svc = resp.service || "unknown";
    const ver = resp.version || "?";
    showToast(`RPC OK: ${svc} v${ver}`, "success");
  } catch (e) {
    showToast(`RPC error: ${formatError(e)}`, "error");
  }
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
          <button data-tab="trace">Trace</button>
          <button data-tab="wallet">Wallet</button>
          <button data-tab="keypair">Keys</button>
          <button data-tab="logs">Logs</button>
        </nav>
      </header>

      <div id="tab_keypair" class="tab">
        <section>
          <h2>Keypair Manager</h2>
          <div class="row">
            <label>Keypair File <input id="kp_path" value="${esc(config.miner_json_path || '')}" placeholder="~/pot-o-miner-cli/miner.json" /></label>
            <button type="button" style="margin-top:18px" data-action="load-keypair">Load</button>
          </div>
          <div id="kp_info"></div>
          <div class="actions">
            <button type="button" class="primary" data-action="generate-keypair">Generate New Keypair</button>
            <button type="button" data-action="clear-keypair">Clear</button>
          </div>
        </section>
        <section>
          <h2>Pubkey Identity</h2>
          <p style="font-size:0.82rem;color:#999;margin-bottom:8px">Set this keypair's pubkey as your miner identity in Settings.</p>
          <div class="row">
            <label>Miner Pubkey <input id="kp_pubkey_display" readonly placeholder="Load a keypair to see pubkey" /></label>
            <button type="button" style="margin-top:18px" data-action="set-pubkey">Use as Identity</button>
          </div>
        </section>
      </div>

      <div id="tab_settings" class="tab">
        <section>
          <h2>Connection</h2>
          <label>RPC URL <input id="rpc_url" value="${esc(config.rpc_url)}" /></label>
          <label>Status URL <input id="status_url" value="${esc(config.status_url)}" /></label>
          <label>WebSocket URL <input id="ws_url" value="${esc(config.ws_url || '')}" placeholder="Auto from RPC URL if empty" /></label>
          <label>Wallet Gateway URL <input id="wallet_base_url" value="${esc(config.wallet_base_url)}" placeholder="https://wallet.rpc.gateway.tribewarez.com" /></label>
          <button type="button" data-action="test-connection">Test Connection</button>
        </section>
        <section>
          <h2>Identity</h2>
          <label>Miner Pubkey <input id="miner_pubkey" value="${esc(config.miner_pubkey)}" placeholder="Miner identity pubkey" /></label>
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
            <label>Device ID <input id="device_id" value="${esc(config.device_id)}" placeholder="(Auto-generated if empty)" /></label>
          </div>
        </section>
        <section>
          <h2>P2P / Network</h2>
          <div class="row">
            <label>Network Mode
              <select id="peer_network_mode">
                <option value="local_only" ${selmode("local_only")}>Local Only</option>
                <option value="vpn_mesh" ${selmode("vpn_mesh")}>VPN Mesh</option>
              </select>
            </label>
            <label>Pool Strategy
              <select id="pool_strategy">
                <option value="solo" ${selpool("solo")}>Solo</option>
                <option value="proportional" ${selpool("proportional")}>Proportional</option>
                <option value="pplns" ${selpool("pplns")}>PPLNS</option>
              </select>
            </label>
          </div>
          <label>Peer Timeout (s) <input id="peer_timeout_secs" type="number" value="${config.peer_timeout_secs}" /></label>
          <label>Bootstrap URLs (one per line)
            <textarea id="bootstrap_urls" rows="3" placeholder="http://bootstrap1.local:8765">${esc((config.bootstrap_urls || []).join("\n"))}</textarea>
          </label>
          <label>mDNS Service Name <input id="mdns_service_name" value="${esc(config.mdns_service_name)}" placeholder="pot-o-desktop" /></label>
          <label class="checkbox"><input id="enable_mdns" type="checkbox" ${config.enable_mdns ? "checked" : ""} /> Enable mDNS Discovery</label>
          <label class="checkbox"><input id="challenge_relay_enabled" type="checkbox" ${config.challenge_relay_enabled ? "checked" : ""} /> Challenge Relay Enabled</label>
        </section>
        <section>
          <h2>Mode</h2>
          <label class="checkbox"><input id="hexchain_mode" type="checkbox" ${config.hexchain_mode ? "checked" : ""} /> Hexchain Lattice PoW Mode</label>
        </section>
        <section>
          <h2>WebSocket</h2>
          <p style="font-size:0.82rem;color:#999;margin-bottom:8px">Connect to validator WebSocket for push challenges and real-time updates.</p>
          <div class="actions">
            <button type="button" data-action="ws-connect" id="ws-connect-btn" ${wsConnected ? 'disabled' : ''}>Connect WS</button>
            <button type="button" data-action="ws-disconnect" id="ws-disconnect-btn" ${!wsConnected ? 'disabled' : ''}>Disconnect WS</button>
            <span id="ws-status" style="font-size:0.82rem;color:${wsConnected ? '#00d4aa' : '#666'};margin-left:8px">${wsConnected ? '● Connected' : '○ Disconnected'}</span>
          </div>
        </section>
        <section>
          <h2>Debug</h2>
          <label class="checkbox"><input id="explain" type="checkbox" ${config.explain ? "checked" : ""} /> Explain</label>
          <label class="checkbox"><input id="verbose" type="checkbox" ${config.verbose ? "checked" : ""} /> Verbose</label>
        </section>
        <div class="actions">
          <button type="button" class="primary" data-action="save-config">Save Settings</button>
          <button type="button" data-action="register-device">Register Device</button>
        </div>
      </div>

      <div id="tab_dashboard" class="tab active"></div>

      <div id="tab_trace" class="tab"></div>

      <div id="tab_wallet" class="tab"></div>

      <div id="tab_logs" class="tab"></div>
    </div>
  `;
  startDashboard();
}

// ── WebSocket ────────────────────────────────────────────

async function doWsConnect() {
  try {
    const deviceId = await invoke("ws_connect");
    wsConnected = true;
    updateWsUi();
    showToast(`WS connected (device: ${deviceId.slice(0, 8)}...)`, "success");
    setupWsListeners();
  } catch (e) {
    showToast(`WS connect failed: ${formatError(e)}`, "error");
  }
}

async function doWsDisconnect() {
  try {
    await invoke("ws_disconnect");
    wsConnected = false;
    wsChallengeHandler = null;
    updateWsUi();
    showToast("WS disconnected", "info");
  } catch (e) {
    showToast(`WS disconnect failed: ${formatError(e)}`, "error");
  }
}

// ── Device Registration ─────────────────────────────────

async function doRegisterDevice() {
  gatherFormValues();
  try {
    const resp = await invoke("register_device", {
      deviceType: (config.device_type || "native").toLowerCase(),
      deviceId: config.device_id || null,
      minerPubkey: config.miner_pubkey || null,
    });
    showToast(`Device registered: ${resp.device_id || "ok"}`, "success");
  } catch (e) {
    showToast(`Register failed: ${formatError(e)}`, "error");
  }
}

// ── P2P Bootstrapping ──────────────────────────────────
let bootstrapped = false;

async function autoBootstrap() {
  if (bootstrapped || !config.miner_pubkey) return;
  bootstrapped = true;

  // Step 1: Register device on validator
  try {
    await invoke("register_device", {
      deviceType: (config.device_type || "native").toLowerCase(),
      minerPubkey: config.miner_pubkey,
    });
  } catch (_) {
    // non-fatal
  }

  // Step 2: Connect WebSocket for push challenges + P2P presence
  if (!wsConnected) {
    try {
      await doWsConnect();
    } catch (_) {
      // non-fatal
    }
  }
}

// ── Wallet ──────────────────────────────────────────────

const TOKEN_TYPES = ["tribechain", "pttc", "nmtc", "stomp", "aum", "ai3"];

function startWallet() {
  renderWallet();
  if (!walletTimer) walletTimer = setInterval(refreshWallet, 15000);
  refreshWallet();
}

function stopWallet() {
  if (walletTimer) { clearInterval(walletTimer); walletTimer = null; }
}

async function refreshWallet() {
  const addr = (config.wallet_address || config.miner_pubkey || "").trim();
  if (!addr) {
    walletData = { error: "No wallet address configured" };
    renderWallet();
    return;
  }
  const results = {};
  results.address = addr;
  const balances = {};
  await Promise.all(TOKEN_TYPES.map(async (tok) => {
    try {
      const r = await invoke("status_api_get", { path: `/api/balance/${addr}/${tok}` });
      balances[tok] = r.balance;
    } catch { balances[tok] = null; }
  }));
  results.balances = balances;
  try {
    const txData = await invoke("status_api_get", { path: `/api/transactions/${addr}` });
    results.transactions = txData.transactions || [];
  } catch { results.transactions = []; }
  try {
    const supply = await invoke("status_api_get", { path: "/api/supply" });
    results.tribeAddress = "TRIBE";
    results.tribeSupply = supply.TRIBE;
    results.tribeMinted = supply.TRIBE;
  } catch { results.tribeSupply = null; }
  walletData = results;
  renderWallet();
}

async function doWalletTransfer() {
  const from = document.getElementById("wf_from").value || config.miner_pubkey;
  const to = document.getElementById("wf_to").value;
  const token = document.getElementById("wf_token").value;
  const amount = parseInt(document.getElementById("wf_amount").value);
  const fee = parseInt(document.getElementById("wf_fee").value) || 0;
  if (!to || !amount) { showToast("Recipient and amount required", "error"); return; }
  try {
    const r = await invoke("rpc_post", {
      path: "/token/transfer",
      body: { from, to, token_type: token, amount, fee },
    });
    showToast(`Transfer sent: ${r.tx_hash || "ok"}`, "success");
    refreshWallet();
  } catch (e) {
    showToast(`Transfer failed: ${formatError(e)}`, "error");
  }
}

async function setWalletAddress() {
  const addr = document.getElementById("wallet_address").value.trim();
  if (!addr) { showToast("Enter a wallet address", "error"); return; }
  config.wallet_address = addr;
  document.getElementById("miner_pubkey").value = addr;
  showToast("Wallet address set", "success");
  refreshWallet();
}

async function doWalletGatewayLogin() {
  const addr = document.getElementById("wg_address").value.trim();
  const pass = document.getElementById("wg_password").value;
  if (!addr || !pass) { showToast("Address and password required", "error"); return; }
  try {
    await invoke("wallet_login", { address: addr, password: pass });
    walletLoggedIn = true;
    showToast("Wallet gateway logged in", "success");
    renderWallet();
  } catch (e) {
    showToast(`Login failed: ${formatError(e)}`, "error");
  }
}

async function doWalletGatewayLogout() {
  await invoke("wallet_logout");
  walletLoggedIn = false;
  showToast("Logged out", "info");
  renderWallet();
}

async function doWalletGatewayList() {
  try {
    const accounts = await invoke("wallet_list_accounts");
    walletAccounts = accounts;
    showToast(`Found ${accounts.length} accounts`, "success");
    renderWallet();
  } catch (e) {
    showToast(`List failed: ${formatError(e)}`, "error");
  }
}

function renderWallet() {
  const el = document.getElementById("tab_wallet");
  if (!el) return;
  const d = walletData || {};
  const addr = config.wallet_address || config.miner_pubkey || "";
  let html = `<div class="container">
    <header><h1>Wallet</h1></header>`;

  // Wallet address
  html += `<section><h2>Account</h2>
    <div class="row">
      <label>Wallet Address <input id="wallet_address" value="${esc(addr)}" placeholder="Enter wallet address" /></label>
      <button type="button" style="margin-top:18px" data-action="wallet-set-address">Set</button>
    </div>`;
  if (!addr) {
    html += `<p class="dim">No wallet address set. Enter your pubkey above or load a keypair from the Keys tab.</p>`;
  }
  html += `</section>`;

  // Balances
  html += `<section><h2>Balances</h2>`;
  if (d.error) {
    html += `<p class="err">${esc(d.error)}</p>`;
  } else if (d.balances) {
    html += `<table><tr><th>Token</th><th>Balance</th></tr>`;
    for (const tok of TOKEN_TYPES) {
      const bal = d.balances[tok];
      html += `<tr><td>${tok.toUpperCase()}</td><td>${bal != null ? bal : '—'}</td></tr>`;
    }
    html += `</table>`;
  } else {
    html += `<p class="dim">Loading...</p>`;
  }
  html += `</section>`;

  // TRIBE info
  if (d.tribeAddress) {
    html += `<section><h2>TRIBE Token</h2>
      <div class="grid-2">
        <div><strong>Mint:</strong> ${esc(d.tribeAddress)}</div>
        <div><strong>Supply:</strong> ${d.tribeSupply ?? '—'}</div>
        <div><strong>Minted:</strong> ${d.tribeMinted ?? '—'}</div>
      </div></section>`;
  }

  // Transfer
  html += `<section><h2>Transfer</h2>
    <div class="row">
      <label>From <input id="wf_from" value="${esc(addr)}" /></label>
      <label>To <input id="wf_to" placeholder="Recipient address" /></label>
    </div>
    <div class="row">
      <label>Token
        <select id="wf_token">
          ${TOKEN_TYPES.map(t => `<option value="${t}">${t.toUpperCase()}</option>`).join('')}
        </select>
      </label>
      <label>Amount <input id="wf_amount" type="number" placeholder="0" /></label>
      <label>Fee <input id="wf_fee" type="number" value="0" /></label>
    </div>
    <button type="button" class="primary" data-action="wallet-transfer">Send</button></section>`;

  // Transactions
  html += `<section><h2>Recent Transactions</h2>`;
  const txs = d.transactions || [];
  if (txs.length === 0) {
    html += `<p class="dim">No transactions yet</p>`;
  } else {
    html += `<table><tr><th>Type</th><th>Token</th><th>Amount</th><th>From/To</th><th>Time</th></tr>`;
    for (const tx of txs.slice(0, 10)) {
      const tok = tx.token_type || tx.token || '?';
      const amt = tx.amount ?? '?';
      const from = esc(String(tx.from || '').slice(0, 16));
      const to = esc(String(tx.to || '').slice(0, 16));
      const ts = tx.timestamp ? String(tx.timestamp).slice(0, 19) : '?';
      const isOut = tx.from === addr;
      html += `<tr><td>${isOut ? 'OUT' : 'IN'}</td><td>${esc(tok)}</td><td>${amt}</td><td>${isOut ? '→ ' + to : from + ' →'}</td><td>${ts}</td></tr>`;
    }
    html += `</table>`;
  }
  html += `</section>`;

  // Wallet Gateway (optional account management)
  html += `<section><h2>Wallet Gateway</h2>
    <p style="font-size:0.82rem;color:#999;margin-bottom:8px">Login to manage your wallet account on the gateway.</p>`;
  if (walletLoggedIn) {
    html += `<p style="color:#00d4aa;font-size:0.85rem">● Logged in</p>
      <div class="actions">
        <button type="button" data-action="wallet-gateway-list">List Accounts</button>
        <button type="button" data-action="wallet-gateway-logout">Logout</button>
      </div>`;
    if (walletAccounts.length > 0) {
      html += `<ul class="peer-list">`;
      for (const acct of walletAccounts) {
        html += `<li>${esc(acct)}</li>`;
      }
      html += `</ul>`;
    }
  } else {
    html += `<div class="row">
      <label>Address <input id="wg_address" placeholder="Account address" /></label>
      <label>Password <input id="wg_password" type="password" placeholder="Password" /></label>
    </div>
    <button type="button" data-action="wallet-gateway-login">Sign In</button>`;
  }
  html += `</section>`;

  html += `</div>`;
  el.innerHTML = html;
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
  if (miningTimer) { clearTimeout(miningTimer); miningTimer = null; }
}

async function refreshDashboard() {
  if (!document.getElementById("tab_dashboard")) return;

  // If WS is connected and streaming dashboard updates, skip HTTP polling
  if (wsConnected) {
    const stats = await invoke("get_mining_stats");
    dashboardData.stats = stats;
    renderDashboard();
    return;
  }

  try {
    const [gateway, apiLive, hexStatus, hexLattice, stats] = await Promise.all([
      safeFetch("/status", false),
      safeFetch("/api/live", false),
      config.hexchain_mode ? safeFetch("/api/hexchain/status", false) : null,
      config.hexchain_mode ? safeFetch("/api/hexchain/lattice", false) : null,
      invoke("get_mining_stats"),
    ]);
    dashboardData.gateway = gateway;
    dashboardData.apiLive = apiLive;
    dashboardData.pool = apiLive?.telemetry?.pool?.data;
    dashboardData.peers = apiLive?.pot_o?.connected_peers;
    dashboardData.miner = findMinerInLive(apiLive, config.miner_pubkey);
    dashboardData.hexStatus = hexStatus;
    dashboardData.hexLattice = hexLattice;
    dashboardData.stats = stats;

    renderDashboard();
  } catch (e) {
    console.error("Dashboard refresh error:", e);
  }
}

function findMinerInLive(apiLive, pubkey) {
  if (!pubkey || !apiLive?.pot_o?.devices_detail) return null;
  for (const dev of Object.values(apiLive.pot_o.devices_detail)) {
    if (dev.miner_pubkeys && dev.miner_pubkeys.includes(pubkey)) {
      return { pubkey, ...dev };
    }
  }
  return { _error: "404 Not found" };
}

async function safeFetch(path, isPot) {
  try {
    if (isPot) return await invoke("rpc_get", { path });
    else return await invoke("status_api_get", { path });
  } catch (e) {
    const msg = typeof e === "string" ? e : (e.message || "fetch failed");
    return { _error: msg };
  }
}

function renderDashboard() {
  const el = document.getElementById("tab_dashboard");
  if (!el) return;
  const d = dashboardData;
  const stats = d.stats || {};
  const running = stats.running || false;

  let html = `<div class="container">`;
  html += `
    <header>
      <h1>Dashboard</h1>
      <div class="mining-controls">
        <span class="mining-status ${running ? 'running' : ''}">${running ? '● Mining' : '○ Idle'}</span>
        <button type="button" class="${running ? 'btn-stop' : 'btn-start'}" data-action="${running ? 'stop-mining' : 'start-mining'}">
          ${running ? 'Stop' : 'Start'} Mining
        </button>
      </div>
    </header>`;

  html += `<div class="stats-bar">
    <div class="stat"><span class="num">${stats.challenges || 0}</span> Challenges</div>
    <div class="stat"><span class="num">${stats.proofs_found || 0}</span> Found</div>
    <div class="stat"><span class="num">${stats.proofs_submitted || 0}</span> Submitted</div>
    <div class="stat"><span class="num">${stats.proofs_accepted || 0}</span> Accepted</div>
    <div class="stat"><span class="num">${stats.start_time ? fmtDuration(Math.floor(Date.now()/1000) - stats.start_time) : '—'}</span> Uptime</div>
  </div>`;

  // ── Compact trace summary on dashboard ──
  const mode = stats.mining_mode || "";
  const activeTrace = mode !== "" && mode !== null && running;
  if (activeTrace) {
    const pct = stats.total_nonces > 0 ? Math.min(100, Math.round((stats.current_nonce / stats.total_nonces) * 100)) : 0;
    const bestDist = stats.best_distance === 4294967295 ? "—" : stats.best_distance;
    const cid = (stats.last_challenge_id || "").slice(0, 16) + "…";
    html += `<section class="trace-compact">
      <h2>Live Trace — ${mode.toUpperCase()}</h2>
      <div class="trace-grid">
        <div><strong>Challenge:</strong> <span class="mono">${esc(cid)}</span></div>
        <div><strong>Nonce:</strong> ${stats.current_nonce || 0} / ${stats.total_nonces || 0} (${pct}%)</div>
        <div><strong>Best Dist:</strong> ${bestDist}</div>`;
    if (mode === "pot_o") {
      html += `<div><strong>MML:</strong> ${stats.current_mml_score != null ? stats.current_mml_score.toFixed(4) : "—"}</div>`;
    }
    if (mode === "hexchain") {
      html += `<div><strong>Coord:</strong> ${esc(stats.hexchain_coord || "—")}</div>`;
    }
    html += `</div>
      <div class="progress-bar"><div class="progress-fill" style="width:${pct}%"></div></div>
    </section>`;
  }

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
    const ch = pot.current_challenge || {};
    if (ch.id) {
      html += `<div class="challenge-line"><strong>Current Challenge:</strong> id=${esc(String(ch.id).slice(0,24))} slot=${ch.slot ?? '?'} diff=${ch.difficulty ?? '?'}</div>`;
    }
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

  // WebSocket status in dashboard
  html += `<section><h2>WebSocket</h2>`;
  html += `<p id="ws-dashboard-status" style="font-size:0.82rem;color:${wsConnected ? '#00d4aa' : '#666'}">${wsConnected ? '● Connected — receiving push challenges' : '○ Not connected'}</p>`;
  html += `</section>`;

  html += `<section><h2>Pool</h2>`;
  const pool = d.pool || {};
  if (pool._error) {
    html += `<p class="err">${pool._error}</p>`;
  } else {
    html += `<div class="grid-2">
      <div><strong>Type:</strong> ${pool.pool_type ?? pool.type ?? '—'}</div>
      <div><strong>Miners:</strong> ${pool.total_miners ?? pool.miners ?? '—'}</div>
      <div><strong>Stake:</strong> ${pool.total_stake ?? pool.stake ?? '—'}</div>
      <div><strong>Min Stake:</strong> ${pool.minimum_stake ?? '—'}</div>
    </div>`;
  }
  html += `</section>`;

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

  // ── P2P + Hex Lattice ───────────────────────────────
  const peers = d.peers;
  const netPeers = Array.isArray(peers) ? peers : (peers?.peers || []);
  const liveData = dashboardData.apiLive;
  const pot = liveData?.pot_o || liveData || {};
  const p2pMode = pot.peer_network_mode || '—';
  const net = pot.network || {};
  const synced = net.synced;
  const totalNodes = net.total_nodes ?? '—';
  const hexLatticeBlocks = (dashboardData.hexLattice && dashboardData.hexLattice.blocks) || [];
  const validatorId = pot.node_id || '—';
  const peerCount = netPeers.length + (wsConnected ? 1 : 0);
  
  html += `<section>
    <h2 class="collapsible" data-toggle="p2p-section">Connected Peers <span class="peer-cnt">(${peerCount})</span></h2>
    <div id="p2p-section">
      <div class="p2p-mode-line">
        <strong>P2P mode:</strong> ${esc(p2pMode)}
        <span class="p2p-stats">· <strong>Network:</strong> ${totalNodes} node${totalNodes !== 1 ? 's' : ''} · ${synced ? 'Synced' : 'Not synced'}</span>
      </div>
      <div class="peer-list-section"><strong class="label">Peers:</strong><ul class="peer-list">`;

  // Always show the validator as the primary peer when WS is connected
  if (wsConnected) {
    html += `<li class="peer-validator">🟢 ${esc(validatorId)} <span class="dim">(validator · WS connected)</span></li>`;
  }
  for (const p of netPeers.slice(0, 20)) {
    const s = typeof p === 'string' ? p : JSON.stringify(p);
    html += `<li>${esc(s.slice(0, 80))}</li>`;
  }
  if (peerCount === 0) {
    html += `<li class="dim">No peers on network</li>`;
  }
  html += `</ul></div>`;

  // Hex chain lattice — show if hexchain mode or data exists
  if (config.hexchain_mode || hexLatticeBlocks.length > 0) {
    const hs = dashboardData.hexStatus || {};
    html += `<div class="hex-lattice-section">
      <h3>Hex Chain Lattice <span class="dim">(3D honeycomb)</span></h3>
      <div class="grid-2">
        <div><strong>Occupied:</strong> ${hs.occupied_coords ?? (hexLatticeBlocks.length || '—')}</div>
        <div><strong>Depth:</strong> ${hs.latest_depth ?? '—'}</div>`;
    const hch = hs.current_challenge || {};
    if (hch.id) {
      html += `<div><strong>Challenge:</strong> id=${esc(String(hch.id).slice(0, 24))} coord=${JSON.stringify(hch.coord || {})}</div>`;
    }
    html += `</div>`;

    if (hexLatticeBlocks.length > 0) {
      html += `<div class="honeycomb">`;
      for (const b of hexLatticeBlocks.slice(0, 30)) {
        const coord = b.coord || {};
        const q = coord.q ?? coord[0] ?? '';
        const r = coord.r ?? coord[1] ?? '';
        const s = coord.s ?? coord[2] ?? '';
        const depth = b.depth ?? '?';
        const hash = (b.block_hash || '').slice(0, 8);
        html += `<div class="hex-cell" title="q=${q} r=${r} s=${s} depth=${depth} hash=${hash}">
          <span class="hex-coord">${q},${r},${s}</span>
          <span class="hex-depth">d:${depth}</span>
          <span class="hex-hash">${hash}</span>
        </div>`;
      }
      html += `</div>`;
    } else {
      html += `<p class="dim" style="margin-top:6px">No lattice blocks yet</p>`;
    }
    html += `</div>`;
  }

  html += `</div></section>`;

  html += `</div>`;
  el.innerHTML = html;
}

let miningActive = false;

// ── Mining controls ──────────────────────────────────────

async function doStartMining() {
  await invoke("start_mining");
  showToast("Mining started", "success");
  renderDashboard();
  if (!miningActive) runMiningLoop();
}

async function doStopMining() {
  await invoke("stop_mining");
  miningActive = false;
  if (miningTimer) { clearTimeout(miningTimer); miningTimer = null; }
  showToast("Mining stopped", "success");
  renderDashboard();
}

function waitForWsResult(timeoutMs = 5000) {
  let done = false;
  let unsub1, unsub2, unsub3;
  const cleanup = () => {
    if (unsub1) try { unsub1(); } catch (_) {}
    if (unsub2) try { unsub2(); } catch (_) {}
    if (unsub3) try { unsub3(); } catch (_) {}
  };
  return new Promise((resolve, reject) => {
    listen("ws-proof-accepted", (e) => {
      if (done) return;
      done = true;
      cleanup();
      resolve({ accepted: true, tx_signature: e.payload.tx_signature });
    }).then((u) => { unsub1 = u; if (done) cleanup(); });
    listen("ws-proof-rejected", (e) => {
      if (done) return;
      done = true;
      cleanup();
      resolve({ accepted: false, reason: e.payload.reason });
    }).then((u) => { unsub2 = u; if (done) cleanup(); });
    listen("ws-error", (e) => {
      if (done) return;
      done = true;
      cleanup();
      reject(new Error(`WS error: ${e.payload.message}`));
    }).then((u) => { unsub3 = u; if (done) cleanup(); });
    setTimeout(() => {
      if (done) return;
      done = true;
      cleanup();
      reject(new Error("WS submit timeout"));
    }, timeoutMs);
  });
}

async function runMiningLoop() {
  if (miningActive) return;
  miningActive = true;
  try {
    const stats = await invoke("get_mining_stats");
    if (!stats.running) { miningActive = false; return; }

    try {
      let challenge;

      // If WS connected, try to use push challenge (set by wsChallengeHandler)
      if (wsChallengeHandler) {
        challenge = await wsChallengeHandler();
        wsChallengeHandler = null; // consume once — next WS push will set it again
      }

      // Fallback to HTTP pull if WS challenge is missing or lacks tensor data
      if (!challenge || !challenge.id || !challenge.input_tensor?.data?.F32) {
        if (challenge && challenge.id && !challenge.input_tensor?.data?.F32) {
          console.warn("WS challenge lacks tensor data — falling back to HTTP /challenge");
        }
        challenge = await invoke("rpc_post", {
          path: "/challenge",
          body: { device_type: (config.device_type || "native").toLowerCase() },
        });
      }

      if (challenge && challenge.id) {
        stats.challenges++;
        stats.last_challenge_id = challenge.id || "";
        await invoke("set_mining_stats", { stats });

        // Auto-detect mining mode from challenge fields.
        // Hexchain challenges have coord + target; PoT-O challenges have operation_type + tensor.
        const isHexChallenge = challenge.coord && challenge.target;
        const useHexchain = isHexChallenge || (config.hexchain_mode && !challenge.operation_type);

        const result = useHexchain
          ? await invoke("mine_hexchain", { challenge })
          : await invoke("mine_pot_o", { challenge });

        if (result.reason) {
          console.warn("Mining result:", result.status, result.reason, "mml_score:", result.mml_score);
        }

        if (result.status === "proof_found") {
          stats.proofs_found++;
          await invoke("set_mining_stats", { stats });

          try {
            // Try WS submission first, fall back to HTTP
            let accepted;
            if (wsConnected) {
              try {
                await invoke("ws_send", {
                  payload: {
                    type: "submit_proof",
                    proof: result.proof,
                    device_id: config.device_id || null,
                    device_type: (config.device_type || "native").toLowerCase(),
                  },
                });
                const wsResult = await waitForWsResult(2000);
                accepted = wsResult.accepted;
                if (accepted) {
                  showToast("Proof accepted via WS", "success");
                } else {
                  showToast(`Proof rejected: ${wsResult.reason || "unknown"}`, "error");
                }
              } catch (e) {
                // WS failed, fall through to HTTP
                console.warn("WS submit failed, falling back to HTTP:", e);
              }
            }

            if (!wsConnected || accepted === undefined) {
              const submitResp = await invoke("rpc_post", {
                path: "/submit",
                body: {
                  proof: result.proof,
                  device_id: config.device_id || null,
                  device_type: config.device_type || "native",
                },
              });
              if (submitResp && submitResp.accepted) {
                accepted = true;
                showToast("Proof accepted", "success");
              } else {
                showToast("Proof submitted (not accepted)", "info");
              }
            }

            stats.proofs_submitted++;
            if (accepted) {
              stats.proofs_accepted++;
            }
            await invoke("set_mining_stats", { stats });
          } catch (e) {
            showMiningError(`Submit failed: ${formatError(e)}`);
          }
        }
      }
    } catch (e) {
      showMiningError(`Mining cycle: ${formatError(e)}`);
    }

    const delay = (config.loop_delay || 2) * 1000;
    miningTimer = setTimeout(() => {
      miningActive = false;
      runMiningLoop();
    }, delay);
  } catch (e) {
    miningActive = false;
    showMiningError(`Mining error: ${formatError(e)}`);
    const delay = (config.loop_delay || 2) * 1000;
    miningTimer = setTimeout(runMiningLoop, delay);
  }
  renderDashboard();
}

// ── Helpers ──────────────────────────────────────────────

function esc(s) {
  return String(s).replace(/&/g, "&amp;").replace(/"/g, "&quot;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}
function sel(v) { return config.operation === v ? "selected" : ""; }
function seld(v) { return config.device_type === v ? "selected" : ""; }
function selmode(v) { return config.peer_network_mode === v ? "selected" : ""; }
function selpool(v) { return config.pool_strategy === v ? "selected" : ""; }
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
  const info = await invoke("generate_keypair", { path });
  document.getElementById("kp_path").value = path;
  renderKeypairInfo(info);
  showToast(`Keypair generated: ${info.pubkey.slice(0, 16)}...`, "success");
}

async function loadKeypair() {
  const path = document.getElementById("kp_path").value;
  if (!path) {
    document.getElementById("kp_info").innerHTML = "";
    return;
  }
  try {
    const info = await invoke("read_keypair", { path });
    renderKeypairInfo(info);
    showToast("Keypair loaded", "success");
  } catch (e) {
    document.getElementById("kp_info").innerHTML = `<p class="err">${esc(formatError(e))}</p>`;
    showToast(`Load failed: ${formatError(e)}`, "error");
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
}

async function setPubkeyFromKeypair() {
  const pubkey = document.getElementById("kp_pubkey_display").value;
  if (!pubkey) {
    showToast("No pubkey loaded", "error");
    return;
  }
  const minerEl = document.getElementById("miner_pubkey");
  if (minerEl) minerEl.value = pubkey;
  switchTab("settings");
  showToast("Pubkey set — save settings to persist", "success");
}

function osDefaultKeypath() {
  return '~/pot-o-miner-cli/miner.json';
}

// ── Log Viewer ──────────────────────────────────────────

async function refreshLogs() {
  try {
    const content = await invoke("read_log", { maxLines: 200 });
    const el = document.getElementById("log_content");
    if (el) el.textContent = content || "(empty log)";
  } catch (e) {
    const el = document.getElementById("log_content");
    if (el) el.textContent = `Failed to read log: ${formatError(e)}`;
  }
}

async function clearLogs() {
  try {
    await invoke("clear_log");
    const el = document.getElementById("log_content");
    if (el) el.textContent = "(log cleared)";
    showToast("Log cleared", "success");
  } catch (e) {
      showToast(`Clear failed: ${formatError(e)}`, "error");
  }
}

function renderLogsTab() {
  const el = document.getElementById("tab_logs");
  if (!el) return;
  el.innerHTML = `<div class="container">
    <header>
      <h1>Error Log</h1>
      <div class="actions">
        <button type="button" data-action="refresh-logs">Refresh</button>
        <button type="button" data-action="clear-logs">Clear</button>
      </div>
    </header>
    <section>
      <p style="font-size:0.82rem;color:#999;margin-bottom:8px">Log file: ~/.config/pot-o-desktop/pot-o-desktop.log</p>
      <pre id="log_content" class="log-viewer">(loading...)</pre>
    </section>
  </div>`;
  refreshLogs();
}

// ── Trace Tab ────────────────────────────────────────────

function startTracePolling() {
  renderTraceTab();
  refreshTrace();
  if (!traceTimer) traceTimer = setInterval(refreshTrace, 500);
}

function stopTracePolling() {
  if (traceTimer) { clearInterval(traceTimer); traceTimer = null; }
}

async function refreshTrace() {
  if (!document.getElementById("tab_trace")) return;
  try {
    const stats = await invoke("get_mining_stats");
    renderTraceTab(stats);
  } catch (_) {}
}

function renderTraceTab(stats) {
  const el = document.getElementById("tab_trace");
  if (!el) return;
  const s = stats || {};
  const running = s.running || false;
  const mode = s.mining_mode || "";
  const active = mode !== "" && mode !== null;
  const isPot = mode === "pot_o";
  const isHex = mode === "hexchain";
  const pct = s.total_nonces > 0 ? Math.min(100, Math.round((s.current_nonce / s.total_nonces) * 100)) : 0;
  const elapsed = s.start_time ? fmtDuration(Math.floor(Date.now() / 1000) - s.start_time) : "—";
  const challengeLabel = s.last_challenge_id || "—";
  const shortChallenge = challengeLabel.length > 32 ? challengeLabel.slice(0, 16) + "…" + challengeLabel.slice(-8) : challengeLabel;
  const bestDist = s.best_distance === 4294967295 ? "—" : s.best_distance;

  let html = `<div class="container">`;
  html += `<header><h1>Trace</h1></header>`;

  // ── Status ──
  html += `<section><h2>Status</h2>`;
  html += `<div class="trace-status-bar"><span class="trace-led ${active ? 'running' : ''}"></span>`;
  html += `<span class="trace-status-text">${active ? '● Mining Active — ' + mode.toUpperCase() : '○ Idle'}</span></div>`;
  html += `<div class="trace-grid">
    <div><strong>Uptime:</strong> ${elapsed}</div>
    <div><strong>Challenges:</strong> ${s.challenges || 0}</div>
    <div><strong>Found:</strong> ${s.proofs_found || 0}</div>
    <div><strong>Submitted:</strong> ${s.proofs_submitted || 0}</div>
    <div><strong>Accepted:</strong> ${s.proofs_accepted || 0}</div>`;
  if (running) {
    html += `<div class="trace-stat-run" style="color:#00d4aa;font-weight:600">● Mining Loop Running</div>`;
  }
  html += `</div></section>`;

  if (!active) {
    if (running) {
      html += `<section><p class="dim">○ Awaiting next challenge...</p></section>`;
    } else {
      html += `<section><p class="dim">No mining in progress. Start mining from the Dashboard tab to see live trace data.</p></section>`;
    }
    html += `</div>`;
    el.innerHTML = html;
    return;
  }

  // ── Challenge Info ──
  html += `<section><h2>Challenge</h2>`;
  html += `<div class="trace-grid">
    <div><strong>ID:</strong> <span class="mono">${esc(shortChallenge)}</span></div>`;
  if (isPot) {
    html += `<div><strong>Operation:</strong> ${esc(s.current_operation || "—")}</div>
      <div><strong>Tensor:</strong> ${esc(s.current_tensor_dims || "—")}</div>`;
  }
  if (isHex) {
    html += `<div><strong>Coord:</strong> ${esc(s.hexchain_coord || "—")}</div>
      <div><strong>Target:</strong> <span class="mono path-sig">${esc((s.hexchain_target || "—").slice(0, 32))}…</span></div>`;
  }
  html += `</div></section>`;

  // ── Progress ──
  html += `<section><h2>Progress</h2>`;
  html += `<div class="trace-grid">
    <div><strong>Nonce:</strong> ${s.current_nonce || 0} / ${s.total_nonces || 0}</div>
    <div><strong>Best Distance:</strong> ${bestDist}</div>`;
  if (isPot) {
    html += `<div><strong>MML Score:</strong> ${s.current_mml_score != null ? s.current_mml_score.toFixed(4) : "—"}</div>`;
  }
  html += `</div>`;

  // ── Progress bar ──
  html += `<div class="progress-bar"><div class="progress-fill" style="width:${pct}%"></div></div>`;
  html += `<div class="pct-label">${pct}%</div>`;

  // ── Path Signature ──
  if (isPot && s.current_path_sig) {
    html += `<div class="trace-path-line"><strong>Path Sig:</strong> <span class="mono path-sig">${esc(s.current_path_sig)}</span></div>`;
  }

  html += `</section>`;

  html += `</div>`;
  el.innerHTML = html;
}

// ── WebSocket Event Listeners ────────────────────────────

async function setupWsListeners() {
  if (wsListenersSetup) return;
  wsListenersSetup = true;
  for (const unsub of wsEventUnlisten) {
    try { unsub(); } catch (_) {}
  }
  wsEventUnlisten = [];
  wsChallengeHandler = null;

  const onChallenge = await listen("ws-challenge", (event) => {
    const c = event.payload;
    wsChallengeHandler = () => Promise.resolve(c);
    showToast("WS challenge received via push", "info");
  });
  wsEventUnlisten.push(onChallenge);

  const onConnected = await listen("ws-connected", () => {
    wsConnected = true;
    updateWsUi();
    showToast("WebSocket connected", "success");
  });
  wsEventUnlisten.push(onConnected);

  const onReconnecting = await listen("ws-reconnecting", (event) => {
    const { delay_secs } = event.payload;
    const el = document.getElementById("ws-dashboard-status");
    if (el) {
      el.textContent = `◐ Reconnecting in ${delay_secs}s...`;
      el.style.color = "#ffa500";
    }
    const statusEl = document.getElementById("ws-status");
    if (statusEl) {
      statusEl.textContent = `◐ Reconnecting in ${delay_secs}s...`;
      statusEl.style.color = "#ffa500";
    }
  });
  wsEventUnlisten.push(onReconnecting);

  const onDashboardUpdate = await listen("ws-dashboard-update", (event) => {
    const data = event.payload;
    if (data) {
      Object.assign(dashboardData, data);
      renderDashboard();
    }
  });
  wsEventUnlisten.push(onDashboardUpdate);

  const onDisconnect = await listen("ws-disconnected", () => {
    wsConnected = false;
    wsChallengeHandler = null;
    updateWsUi();
    showToast("WebSocket disconnected", "info");
  });
  wsEventUnlisten.push(onDisconnect);

  const onError = await listen("ws-error", (event) => {
    const { message } = event.payload;
    showToast(`WS error: ${message}`, "error");
  });
  wsEventUnlisten.push(onError);
}

function updateWsUi() {
  const connectBtn = document.getElementById("ws-connect-btn");
  const disconnectBtn = document.getElementById("ws-disconnect-btn");
  const statusEl = document.getElementById("ws-status");
  if (connectBtn) connectBtn.disabled = wsConnected;
  if (disconnectBtn) disconnectBtn.disabled = !wsConnected;
  if (statusEl) {
    statusEl.textContent = wsConnected ? "● Connected" : "○ Disconnected";
    statusEl.style.color = wsConnected ? "#00d4aa" : "#666";
  }
  const wsSection = document.getElementById("ws-dashboard-status");
  if (wsSection) {
    wsSection.textContent = wsConnected ? "● Connected — receiving push challenges" : "○ Not connected";
    wsSection.style.color = wsConnected ? "#00d4aa" : "#666";
  }
}

// ── Init ─────────────────────────────────────────────────

document.addEventListener("DOMContentLoaded", async () => {
  initToasts();
  await loadConfig();
  autoBootstrap();
  setupWsListeners();
});
