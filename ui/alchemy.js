(() => {
  const BASE = "http://127.0.0.1:38991";
  function $(id) {
    const el = document.getElementById(id);
    if (el) return el;
    const proxyTarget = {};
    return new Proxy(proxyTarget, {
      get(target, prop) {
        if (prop === 'addEventListener') return () => {};
        if (prop === 'closest') return () => null;
        if (prop === 'querySelectorAll') return () => [];
        if (prop === 'classList') return { add() {}, remove() {} };
        if (prop === 'style') return { display: 'none' };
        if (prop === 'appendChild') return () => {};
        if (prop === 'innerHTML' || prop === 'textContent' || prop === 'value') return '';
        if (prop === 'getAttribute') return () => null;
        if (prop === 'setAttribute') return () => {};
        return target[prop];
      },
      set(target, prop, value) { target[prop] = value; return true; },
    });
  }

  // ── State ────────────────────────────────────────────────────────────────
  let accounts = [];           // loaded from settings
  let queue = [];              // { uniquename, display_name, craft_amount, quantity_out }
  let currentSessionId = null; // session_id after planning
  let comboSelectedUniquename = "";
  let comboSelectedDisplay = "";
  let comboTimer = null;
  let comboResults = [];
  let comboActiveIdx = -1;
  let comboReqId = 0;

  // ── Helpers ───────────────────────────────────────────────────────────────
  function setError(id, msg) {
    const el = $(id);
    if (el) el.textContent = msg || "";
  }

  function fmt(n) {
    if (n == null) return "—";
    return Number(n).toLocaleString();
  }

  // ── Account selector ─────────────────────────────────────────────────────
  async function loadAccounts() {
    try {
      const r = await fetch(`${BASE}/api/v1/settings`);
      if (!r.ok) throw new Error();
      const settings = await r.json();
      accounts = settings.accounts || [];

      const sel = $("alchemy-account");
      sel.innerHTML = accounts.map((a, i) =>
        `<option value="${i}">${a.name} — ${a.city}${a.use_focus ? " (focus)" : ""}</option>`
      ).join("");

      updateRRRBar();
    } catch {
      $("alchemy-account").innerHTML = `<option value="">No accounts configured</option>`;
    }
  }

  function selectedAccount() {
    const idx = Number($("alchemy-account").value);
    return accounts[idx] || null;
  }

  function updateRRRBar() {
    const acc = selectedAccount();
    const bar = $("alchemy-rrr-bar");
    if (!acc) { bar.style.display = "none"; return; }

    // Compute RRR client-side for display (same formula as Rust)
    const base = 18.0;
    const cityBonus = acc.crafting_lines && acc.crafting_lines.length > 0 ? 29.0 : 0.0;
    const focusBonus = acc.use_focus ? 59.0 : 0.0;
    const pb = base + cityBonus + focusBonus;
    const rrr = 1.0 - 1.0 / (1.0 + pb / 100.0);

    $("rrr-city").textContent = acc.city;
    $("rrr-focus").textContent = acc.use_focus ? "Yes" : "No";
    $("rrr-value").textContent = `${(rrr * 100).toFixed(1)}%`;
    bar.style.display = "flex";
  }

  // ── Autocomplete ──────────────────────────────────────────────────────────
  function setupCombo() {
    const input = $("alchemy-item-search");
    let wrap = (input && typeof input.closest === 'function') ? input.closest(".combo-wrap") : null;
    if (!wrap) wrap = (input && input.parentElement) ? input.parentElement : document.body;
    let label = $("alchemy-item-search-label");
    if (!label) label = { textContent: '' };

    const menu = document.createElement("div");
    menu.className = "combo-menu";
    menu.style.display = "none";
    wrap.appendChild(menu);

    function closeMenu() {
      menu.style.display = "none";
      comboActiveIdx = -1;
    }

    function renderMenu() {
      if (!comboResults.length) {
        menu.innerHTML = `<div class="combo-empty">No items found</div>`;
        menu.style.display = "block";
        return;
      }
      menu.innerHTML = comboResults.map((it, idx) => {
        const sub = [it.shopcategory, it.shopsubcategory1].filter(Boolean).join(" > ");
        const name = it.display_name || it.uniquename;
        return `<div class="search-row${idx === comboActiveIdx ? " active" : ""}" data-idx="${idx}">
          <span class="tier-badge">T${it.tier}</span>
          <span class="item-name">${name}</span>
          <span class="item-cat">${sub}</span>
        </div>`;
      }).join("");
      menu.style.display = "block";
      menu.querySelectorAll(".search-row").forEach(row => {
        row.addEventListener("mousedown", e => {
          e.preventDefault();
          const it = comboResults[Number(row.dataset.idx)];
          if (!it) return;
          comboSelectedUniquename = it.uniquename;
          comboSelectedDisplay = it.display_name || it.uniquename;
          input.value = comboSelectedDisplay;
          label.textContent = it.uniquename;
          closeMenu();
        });
      });
    }

    input.addEventListener("input", () => {
      const q = input.value.trim();
      comboSelectedUniquename = "";
      comboSelectedDisplay = "";
      label.textContent = "";
      if (!q) { clearTimeout(comboTimer); closeMenu(); return; }
      clearTimeout(comboTimer);
      comboTimer = setTimeout(async () => {
        const rid = ++comboReqId;
        try {
          const r = await fetch(`${BASE}/api/v1/marrow/search?q=${encodeURIComponent(q)}`);
          if (!r.ok || rid !== comboReqId) return;
          comboResults = await r.json();
          comboActiveIdx = comboResults.length ? 0 : -1;
          renderMenu();
        } catch { /* ignore */ }
      }, 300);
    });

    input.addEventListener("keydown", e => {
      if (e.key === "ArrowDown" && comboResults.length) {
        e.preventDefault();
        comboActiveIdx = Math.min(comboActiveIdx + 1, comboResults.length - 1);
        renderMenu();
      } else if (e.key === "ArrowUp" && comboResults.length) {
        e.preventDefault();
        comboActiveIdx = Math.max(comboActiveIdx - 1, 0);
        renderMenu();
      } else if (e.key === "Enter" && comboActiveIdx >= 0 && comboResults[comboActiveIdx]) {
        e.preventDefault();
        const it = comboResults[comboActiveIdx];
        comboSelectedUniquename = it.uniquename;
        comboSelectedDisplay = it.display_name || it.uniquename;
        input.value = comboSelectedDisplay;
        label.textContent = it.uniquename;
        closeMenu();
      } else if (e.key === "Escape") {
        closeMenu();
      }
    });

    document.addEventListener("click", e => {
      if (!wrap.contains(e.target)) closeMenu();
    });
  }

  // ── Queue management ──────────────────────────────────────────────────────
  function renderQueue() {
    const tbody = $("alchemy-queue-body");
    const table = $("alchemy-queue-table");
    const empty = $("alchemy-queue-empty");
    const planBtn = $("alchemy-plan-btn");
    const clearBtn = $("alchemy-clear-btn");

    if (!queue.length) {
      table.style.display = "none";
      empty.style.display = "block";
      planBtn.disabled = true;
      clearBtn.style.display = "none";
      return;
    }

    table.style.display = "table";
    empty.style.display = "none";
    planBtn.disabled = false;
    clearBtn.style.display = "inline-block";

    tbody.innerHTML = queue.map((item, idx) => `
      <tr>
        <td>
          <div style="font-weight:500">${item.display_name}</div>
          <div style="font-size:11px;color:#aaa">${item.uniquename}</div>
        </td>
        <td style="text-align:right">
          <input class="input qty-input" type="number" min="1" value="${item.quantity_out}"
            data-idx="${idx}" style="width:70px" />
        </td>
        <td style="text-align:right">${item.craft_amount}</td>
        <td style="text-align:right">${Math.ceil(item.quantity_out / item.craft_amount)}</td>
        <td style="text-align:right">
          <button class="btn-danger" data-remove="${idx}">✕</button>
        </td>
      </tr>
    `).join("");

    tbody.querySelectorAll("input[data-idx]").forEach(input => {
      input.addEventListener("change", () => {
        const idx = Number(input.dataset.idx);
        const val = Math.max(1, parseInt(input.value) || 1);
        queue[idx].quantity_out = val;
        renderQueue();
      });
    });

    tbody.querySelectorAll("button[data-remove]").forEach(btn => {
      btn.addEventListener("click", () => {
        queue.splice(Number(btn.dataset.remove), 1);
        renderQueue();
      });
    });
  }

  async function addItem() {
    setError("alchemy-add-error", "");
    if (!comboSelectedUniquename) {
      setError("alchemy-add-error", "Select an item from the dropdown first");
      return;
    }
    const qty = Math.max(1, parseInt($("alchemy-qty").value) || 1);

    if (queue.find(q => q.uniquename === comboSelectedUniquename)) {
      setError("alchemy-add-error", "Item already in queue — change quantity there");
      return;
    }

    queue.push({
      uniquename: comboSelectedUniquename,
      display_name: comboSelectedDisplay,
      craft_amount: 1, // placeholder; real value comes from DB after planning
      quantity_out: qty,
    });

    // Reset combo
    $("alchemy-item-search").value = "";
    $("alchemy-item-search-label").textContent = "";
    comboSelectedUniquename = "";
    comboSelectedDisplay = "";
    $("alchemy-qty").value = "1";

    renderQueue();
  }

  // ── Planning ──────────────────────────────────────────────────────────────
  async function planSession() {
    setError("alchemy-plan-error", "");
    const acc = selectedAccount();
    if (!acc) { setError("alchemy-plan-error", "Select an account first"); return; }
    if (!queue.length) { setError("alchemy-plan-error", "Queue is empty"); return; }

    const btn = $("alchemy-plan-btn");
    btn.disabled = true;
    btn.textContent = "Planning…";

    try {
      const r = await fetch(`${BASE}/api/v1/alchemy/plan`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          account_name: acc.name,
          items: queue.map(q => ({ uniquename: q.uniquename, quantity_out: q.quantity_out })),
        }),
      });
      if (!r.ok) {
        const err = await r.json().catch(() => ({}));
        throw new Error(err.message || `Error ${r.status}`);
      }
      const session = await r.json();
      currentSessionId = session.session_id;

      // Update queue with real craft_amounts from the planned items
      session.items.forEach(pi => {
        const qi = queue.find(q => q.uniquename === pi.uniquename);
        if (qi) qi.craft_amount = pi.craft_amount;
      });
      renderQueue();

      renderShoppingList(session);
      loadHistory();
    } catch (err) {
      setError("alchemy-plan-error", err.message || "Planning failed");
    } finally {
      btn.disabled = false;
      btn.textContent = "Plan Shopping List";
    }
  }

  // ── Shopping list ─────────────────────────────────────────────────────────
  function renderShoppingList(session) {
    const section = $("alchemy-shopping-section");
    const tbody = $("alchemy-shop-body");
    const rrrBar = $("alchemy-shopping-rrr");

    rrrBar.innerHTML = `
      <span>Account: <strong>${session.account_name}</strong></span>
      <span>City: <strong>${session.city}</strong></span>
      <span>Focus: <strong>${session.use_focus ? "Yes" : "No"}</strong></span>
      <span>RRR: <strong>${session.rrr_pct}%</strong></span>
    `;

    tbody.innerHTML = session.materials.map(m => `
      <tr data-uniquename="${m.uniquename}">
        <td>
          <div style="font-weight:500">${m.display_name}</div>
          <div style="font-size:11px;color:#aaa">${m.uniquename}</div>
        </td>
        <td style="text-align:right;font-weight:600">${fmt(m.quantity_needed)}</td>
        <td style="text-align:right">
          <input class="input price-input" type="number" min="0"
            value="${m.unit_price || ""}" placeholder="Enter price"
            data-uniquename="${m.uniquename}" />
        </td>
        <td style="text-align:right" id="cost-${m.uniquename.replace(/@/g, '-')}">
          ${m.total_cost != null ? fmt(Math.round(m.total_cost)) + " silver" : "—"}
        </td>
      </tr>
    `).join("");

    updateTotalCost(session.materials);

    tbody.querySelectorAll("input.price-input").forEach(input => {
      input.addEventListener("change", async () => {
        const uniquename = input.dataset.uniquename;
        const price = parseInt(input.value) || 0;
        if (price <= 0 || !currentSessionId) return;

        try {
          await fetch(`${BASE}/api/v1/alchemy/sessions/${currentSessionId}/price`, {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ uniquename, unit_price: price }),
          });

          // Update cost cell immediately
          const row = tbody.querySelector(`tr[data-uniquename="${uniquename}"]`);
          const qtyCell = row ? row.cells[1].textContent.replace(/,/g, "") : "0";
          const qty = parseInt(qtyCell) || 0;
          const total = qty * price;
          const costCell = $(`cost-${uniquename.replace(/@/g, '-')}`);
          if (costCell) costCell.textContent = fmt(total) + " silver";

          // Reload full session to get updated total
          const updated = await fetch(`${BASE}/api/v1/alchemy/sessions/${currentSessionId}`);
          if (updated.ok) {
            const s = await updated.json();
            updateTotalCost(s.materials);
          }
        } catch { /* ignore transient errors */ }
      });
    });

    section.style.display = "block";
    section.scrollIntoView({ behavior: "smooth", block: "start" });
  }

  function updateTotalCost(materials) {
    const total = materials.reduce((sum, m) => sum + (m.total_cost || 0), 0);
    const allPriced = materials.length > 0 && materials.every(m => m.unit_price != null);
    $("alchemy-total-cost").textContent = allPriced
      ? fmt(Math.round(total)) + " silver"
      : "— (enter all prices)";
  }

  // ── Send to Marrow ────────────────────────────────────────────────────────
  async function sendToMarrow() {
    if (!currentSessionId) return;
    const btn = $("alchemy-send-marrow");
    btn.disabled = true;
    btn.textContent = "Sending…";
    try {
      const r = await fetch(`${BASE}/api/v1/alchemy/sessions/${currentSessionId}/send`, {
        method: "POST",
      });
      if (!r.ok) throw new Error();
      sessionStorage.setItem("alchemy_session_id", String(currentSessionId));
      window.location.href = "./marrow.html";
    } catch {
      btn.disabled = false;
      btn.textContent = "Send to Marrow →";
    }
  }

  // ── Session history ───────────────────────────────────────────────────────
  async function loadHistory() {
    const list = $("alchemy-history-list");
    try {
      const r = await fetch(`${BASE}/api/v1/alchemy/sessions?limit=10`);
      if (!r.ok) throw new Error();
      const sessions = await r.json();
      if (!sessions.length) {
        list.className = "muted";
        list.textContent = "No sessions yet";
        return;
      }
      list.className = "";
      list.innerHTML = sessions.map(s => `
        <div class="hist-row" data-id="${s.id}">
          <div>
            <div style="font-weight:500">${s.account_name} — ${s.city}</div>
            <div class="hist-row-meta">
              ${s.item_count} item${s.item_count !== 1 ? "s" : ""} ·
              RRR ${(s.rrr * 100).toFixed(1)}% ·
              ${s.total_cost != null ? fmt(Math.round(s.total_cost)) + " silver total" : "prices pending"} ·
              ${s.sent_to_marrow ? "✓ sent to Marrow" : "draft"}
            </div>
          </div>
          <div style="font-size:11px;color:#aaa">
            ${new Date(s.created_at * 1000).toLocaleDateString()}
          </div>
        </div>
      `).join("");

      list.querySelectorAll(".hist-row").forEach(row => {
        row.addEventListener("click", async () => {
          const id = Number(row.dataset.id);
          try {
            const r = await fetch(`${BASE}/api/v1/alchemy/sessions/${id}`);
            if (!r.ok) throw new Error();
            const session = await r.json();
            currentSessionId = session.session_id;
            renderShoppingList(session);
          } catch { /* ignore */ }
        });
      });
    } catch {
      list.className = "error-msg";
      list.textContent = "Failed to load sessions";
    }
  }

  // ── Init ──────────────────────────────────────────────────────────────────
  document.addEventListener("DOMContentLoaded", () => {
    loadAccounts();
    setupCombo();
    loadHistory();

    $("alchemy-account").addEventListener("change", updateRRRBar);
    $("alchemy-add-item").addEventListener("click", addItem);
    $("alchemy-plan-btn").addEventListener("click", planSession);
    $("alchemy-clear-btn").addEventListener("click", () => {
      queue = [];
      currentSessionId = null;
      $("alchemy-shopping-section").style.display = "none";
      renderQueue();
    });
    $("alchemy-send-marrow").addEventListener("click", sendToMarrow);

    // Allow pressing Enter in search to add item
    $("alchemy-item-search").addEventListener("keydown", e => {
      if (e.key === "Enter" && comboSelectedUniquename) addItem();
    });
  });
})();
