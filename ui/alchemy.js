(() => {
  const BASE = "http://127.0.0.1:38991";
  const comboState = {};

  const $ = (id) => document.getElementById(id);

  function setError(id, msg) {
    const el = $(id);
    if (!el) return;
    el.textContent = msg || "";
  }

  function titleCaseToken(token) {
    if (!token) return "";
    const lower = token.toLowerCase();
    return lower.charAt(0).toUpperCase() + lower.slice(1);
  }

  function deriveDisplayName(uniquename) {
    if (!uniquename) return "";
    let out = uniquename;
    if (out.startsWith("T")) {
      const m = out.match(/^T\d+(_\d+)?_/);
      if (m) out = out.slice(m[0].length);
    }
    out = out.replace(/@/g, " ");
    return out.split("_").filter(Boolean).map(titleCaseToken).join(" ");
  }

  function setComboValue(inputId, uniquename, displayName) {
    const state = comboState[inputId];
    if (!state) return;
    state.uniquename = uniquename || "";
    const shown = (displayName && displayName.length) ? displayName : deriveDisplayName(uniquename || "");
    state.input.value = shown;
    state.label.textContent = uniquename ? `${uniquename}` : "";
  }

  function renderMenu(inputId) {
    const state = comboState[inputId];
    if (!state) return;
    const { results, menu } = state;
    if (!results.length) {
      menu.innerHTML = `<div class="combo-empty">No items found</div>`;
      menu.style.display = "block";
      return;
    }
    menu.innerHTML = results
      .map((it, idx) => {
        const sub = `${it.shopcategory || ""} > ${it.shopsubcategory1 || ""}`.replace(/^ > | > $/g, "");
        const shownName = it.display_name && it.display_name.length ? it.display_name : deriveDisplayName(it.uniquename);
        return `<div class="search-row${idx === state.activeIndex ? " active" : ""}" data-idx="${idx}">
          <span class="tier-badge">T${it.tier}</span>
          <span class="item-name">${shownName}</span>
        </div>`;
      })
      .join("");
    menu.style.display = "block";
    menu.querySelectorAll(".search-row").forEach((row) => {
      row.addEventListener("mousedown", (e) => {
        e.preventDefault();
        const idx = Number(row.dataset.idx);
        const it = state.results[idx];
        if (!it) return;
        state.onSelect(it.uniquename);
        setComboValue(inputId, it.uniquename, it.display_name);
        menu.style.display = "none";
      });
    });
  }

  function makeAutocomplete(inputId, onSelect) {
    const input = $(inputId);
    const wrap = input.closest(".combo-wrap");
    const label = $(`${inputId}-label`);
    const menu = document.createElement("div");
    menu.className = "combo-menu";
    menu.style.display = "none";
    wrap.appendChild(menu);

    comboState[inputId] = {
      input, label, menu, onSelect,
      uniquename: "", results: [], activeIndex: -1, timer: null, reqId: 0
    };

    input.addEventListener("input", () => {
      const state = comboState[inputId];
      const q = input.value.trim();
      state.uniquename = "";
      if (!q) {
        clearTimeout(state.timer);
        state.label.textContent = "";
        state.results = [];
        menu.style.display = "none";
        return;
      }
      clearTimeout(state.timer);
      state.timer = setTimeout(async () => {
        const curReqId = ++state.reqId;
        try {
          const r = await fetch(`${BASE}/api/v1/marrow/search?q=${encodeURIComponent(q)}`);
          if (!r.ok) throw new Error();
          const items = await r.json();
          if (curReqId !== state.reqId) return;
          state.results = Array.isArray(items) ? items : [];
          state.activeIndex = state.results.length ? 0 : -1;
          renderMenu(inputId);
        } catch {
          if (curReqId !== state.reqId) return;
          state.results = [];
          state.activeIndex = -1;
          menu.style.display = "none";
        }
      }, 300);
    });

    input.addEventListener("keydown", (e) => {
      const state = comboState[inputId];
      if (e.key === "ArrowDown" && state.results.length) {
        e.preventDefault();
        state.activeIndex = Math.min(state.activeIndex + 1, state.results.length - 1);
        renderMenu(inputId);
      } else if (e.key === "ArrowUp" && state.results.length) {
        e.preventDefault();
        state.activeIndex = Math.max(state.activeIndex - 1, 0);
        renderMenu(inputId);
      } else if (e.key === "Enter" && state.activeIndex >= 0) {
        e.preventDefault();
        const it = state.results[state.activeIndex];
        state.onSelect(it.uniquename);
        setComboValue(inputId, it.uniquename, it.display_name);
        menu.style.display = "none";
      } else if (e.key === "Escape") {
        menu.style.display = "none";
      }
    });

    window.addEventListener("click", (e) => {
      if (!wrap.contains(e.target)) menu.style.display = "none";
    });
  }

  async function calculate() {
    const item = comboState["alchemy-item-id"]?.uniquename || $("alchemy-item-id").value.trim();
    const city = $("alchemy-city").value;
    const returnRate = $("alchemy-return-rate").value;
    const fee = $("alchemy-fee").value;
    const batchSize = $("alchemy-batch").value || 1;

    if (!item) return;

    const btn = $("alchemy-calc");
    btn.disabled = true;
    btn.textContent = "Analyzing...";

    try {
      const r = await fetch(`${BASE}/api/v1/marrow/recommend/${encodeURIComponent(item)}?city=${encodeURIComponent(city)}`);
      if (!r.ok) throw new Error("Fetch failed");
      const d = await r.json();

      renderResults(d, batchSize);
    } catch (err) {
      console.error(err);
    } finally {
      btn.disabled = false;
      btn.textContent = "Calculate Profit";
    }
  }

  function renderResults(d, batch) {
    const matList = $("materials-list");
    const resPanel = $("alchemy-result");

    if (!d.is_craftable) {
      matList.innerHTML = `<div class="error-msg">Item is not craftable (no recipe in DB)</div>`;
      resPanel.innerHTML = "";
      return;
    }

    // Render materials
    const rows = d.crafting_materials.map(m => `
      <tr>
        <td>${m.display_name}</td>
        <td style="text-align:right">x${m.quantity * batch}</td>
        <td style="text-align:right">${m.unit_price.toLocaleString()} s</td>
        <td style="text-align:right; font-weight:600">${(m.total_cost * batch).toLocaleString()} s</td>
      </tr>
    `).join("");

    matList.innerHTML = `
      <table class="materials-table">
        <thead>
          <tr style="color:var(--muted); font-size:10px; text-transform:uppercase">
            <th style="text-align:left">Material</th>
            <th style="text-align:right">Qty</th>
            <th style="text-align:right">Unit</th>
            <th style="text-align:right">Total</th>
          </tr>
        </thead>
        <tbody>${rows}</tbody>
      </table>
    `;

    // Render summary
    const totalProfit = (d.crafting_profit || 0) * batch;
    const profitCls = totalProfit > 0 ? "positive" : "negative";

    resPanel.innerHTML = `
      <div style="text-align:center; margin-bottom:16px">
        <div class="stat-label">Total Profit for ${batch} Batch(es)</div>
        <div class="profit-pill ${profitCls}">${totalProfit.toLocaleString()} s</div>
      </div>
      <div id="alchemy-result-stats">
        <div class="stat-box">
          <div class="stat-label">Unit Profit</div>
          <div class="stat-value">${d.crafting_profit ? d.crafting_profit.toLocaleString() : "—"} s</div>
        </div>
        <div class="stat-box">
          <div class="stat-label">Margin</div>
          <div class="stat-value" style="color:${totalProfit > 0 ? '#22c55e' : '#ef4444'}">${d.crafting_margin_pct}%</div>
        </div>
        <div class="stat-box">
          <div class="stat-label">Sale Price</div>
          <div class="stat-value">${d.output_price.toLocaleString()} s</div>
        </div>
      </div>
    `;
  }

  async function loadGold() {
    const pill = $("gold-pill");
    try {
      const r = await fetch(`${BASE}/api/v1/marrow/gold`);
      const d = await r.json();
      pill.classList.remove("skeleton");
      pill.innerHTML = `<span class="gold-dot"></span><span>Gold · ${d.price.toLocaleString()} s</span>`;
    } catch {}
  }

  async function loadSettings() {
    try {
      const r = await fetch(`${BASE}/api/v1/settings`);
      const s = await r.json();
      if (s.return_rate_pct) $("alchemy-return-rate").value = s.return_rate_pct;
      if (s.crafting_fee_pct) $("alchemy-fee").value = s.crafting_fee_pct;
      if (s.default_city) $("alchemy-city").value = s.default_city;
    } catch {}
  }

  document.addEventListener("DOMContentLoaded", () => {
    makeAutocomplete("alchemy-item-id", () => {});
    $("alchemy-calc").addEventListener("click", calculate);
    loadGold();
    loadSettings();
    setInterval(loadGold, 60000);

    // Auto-update batch calculation if already calculated
    $("alchemy-batch").addEventListener("input", () => {
      // If we have a result, we could re-render, but for now simple.
    });
  });
})();
