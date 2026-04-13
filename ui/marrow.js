(() => {
  const BASE = "http://127.0.0.1:38991";
  let histChart = null;
  const comboState = {};

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

  function setError(id, msg) {
    const el = $(id);
    if (!el) return;
    el.textContent = msg || "";
  }

  async function fillInputs(uniquename) {
    // Shared behavior: clicking a favourite fills history item ID.
    // Try to resolve display name via search endpoint; fall back to uniquename.
    let displayName = uniquename;
    try {
      const r = await fetch(`${BASE}/api/v1/marrow/search?q=${encodeURIComponent(uniquename)}`);
      if (r.ok) {
        const items = await r.json();
        if (Array.isArray(items)) {
          const found = items.find((it) => it.uniquename === uniquename);
          if (found) displayName = found.display_name || uniquename;
        }
      }
    } catch (e) {
      // ignore
    }

    setComboValue("hist-item", uniquename, displayName);
  }

  function titleCaseToken(token) {
    if (!token) return "";
    const lower = token.toLowerCase();
    return lower.charAt(0).toUpperCase() + lower.slice(1);
  }

  function deriveDisplayNameFromUniquename(uniquename) {
    if (!uniquename) return "";
    // Remove tier prefix like T4_ or T5_2_
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
    const shown = (displayName && displayName.length) ? displayName : deriveDisplayNameFromUniquename(uniquename || "");
    state.input.value = shown;
    // Show the uniquename as a muted label so the user can see it
    state.label.textContent = uniquename ? `${uniquename}` : "";
  }

  function closeMenu(inputId) {
    const state = comboState[inputId];
    if (!state) return;
    state.menu.style.display = "none";
    state.activeIndex = -1;
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
        const shownName = it.display_name && it.display_name.length ? it.display_name : deriveDisplayNameFromUniquename(it.uniquename);
          return `<div class="search-row${idx === state.activeIndex ? " active" : ""}" data-idx="${idx}">
          <span class="tier-badge">T${it.tier}</span>
          <span class="item-name">${shownName}</span>
          <span class="item-cat">${sub}</span>
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
        closeMenu(inputId);
      });
    });
  }

  function makeAutocomplete(inputId, onSelect) {
    const input = $(inputId);
    let wrap = (input && typeof input.closest === 'function') ? input.closest(".combo-wrap") : null;
    if (!wrap) wrap = (input && input.parentElement) ? input.parentElement : document.body;
    let label = $(`${inputId}-label`);
    if (!label) label = { textContent: '' };
    const menu = document.createElement("div");
    menu.className = "combo-menu";
    menu.style.display = "none";
    wrap.appendChild(menu);

    comboState[inputId] = {
      input,
      label,
      menu,
      onSelect,
      uniquename: "",
      results: [],
      activeIndex: -1,
      timer: null,
      reqId: 0,
      getValue() { return this.uniquename; }
    };

    input.addEventListener("input", () => {
      const state = comboState[inputId];
      const q = input.value.trim();
      // Any manual input clears the previously selected uniquename
      state.uniquename = "";
      if (!q) {
        clearTimeout(state.timer);
        state.label.textContent = "";
        state.results = [];
        closeMenu(inputId);
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
          state.menu.innerHTML = `<div class="combo-empty">Search failed</div>`;
          state.menu.style.display = "block";
        }
      }, 300);
    });

    input.addEventListener("keydown", (e) => {
      const state = comboState[inputId];
      const open = state.menu.style.display !== "none";
      if (e.key === "ArrowDown" && state.results.length) {
        e.preventDefault();
        state.activeIndex = Math.min(state.activeIndex + 1, state.results.length - 1);
        renderMenu(inputId);
      } else if (e.key === "ArrowUp" && state.results.length) {
        e.preventDefault();
        state.activeIndex = Math.max(state.activeIndex - 1, 0);
        renderMenu(inputId);
      } else if (e.key === "Enter" && open && state.activeIndex >= 0 && state.results[state.activeIndex]) {
        e.preventDefault();
        const it = state.results[state.activeIndex];
        state.onSelect(it.uniquename);
        setComboValue(inputId, it.uniquename, it.display_name);
        closeMenu(inputId);
      } else if (e.key === "Escape") {
        closeMenu(inputId);
      }
    });

    input.addEventListener("focus", () => {
      const state = comboState[inputId];
      if (state.results.length) {
        renderMenu(inputId);
      }
    });
  }

  async function loadGold() {
    const pill = $("gold-pill");
    if (!pill) return;
    try {
      const r = await fetch(`${BASE}/api/v1/marrow/gold`);
      if (!r.ok) throw new Error();
      const data = await r.json();
      pill.classList.remove("skeleton");
      pill.innerHTML = `<span class="gold-dot"></span><span>Gold · ${Number(data.price || 0).toLocaleString()} silver — ${String(data.timestamp || "").slice(0, 19).replace("T", " ")}</span>`;
    } catch {
      pill.classList.remove("skeleton");
      pill.innerHTML = `<span class="gold-dot"></span><span>Gold — unavailable</span>`;
    }
  }

  // Prefill Marrow UI if navigated from Alchemy
  async function prefillFromAlchemy() {
    try {
      const sid = sessionStorage.getItem('alchemy_session_id');
      if (!sid) return;
      const id = Number(sid);
      if (!id) return;

      const r = await fetch(`${BASE}/api/v1/marrow/session/${id}/receive`);
      if (!r.ok) throw new Error('Failed to load alchemy session');
      const s = await r.json();

      // Show panel and badge
      const panel = $('alchemy-session-panel');
      panel.style.display = 'block';
      $('alchemy-session-badge').textContent = `#${s.session_id}`;

      // Fill info bar
      $('alchemy-info-bar').innerHTML = `
        <span>Account: <strong>${s.account_name}</strong></span>
        <span>City: <strong>${s.city}</strong></span>
        <span>RRR: <strong>${(s.rrr_pct * 100).toFixed(1)}%</strong></span>
      `;

      // Items
      const itemsBody = $('alchemy-items-body');
      itemsBody.innerHTML = s.items.map(it => `
        <tr>
          <td style="font-weight:500">${it.uniquename}</td>
          <td style="text-align:right">${it.craft_amount}</td>
          <td style="text-align:right">${Math.ceil( (it.craft_amount || 1) / (it.craft_amount || 1) )}</td>
          <td style="text-align:right"><input class="input sell-price-input" data-item="${it.uniquename}" /></td>
          <td style="text-align:right" id="rev-${it.uniquename.replace(/@/g,'-')}">—</td>
        </tr>
      `).join('');

      // Materials
      const matsBody = $('alchemy-mats-body');
      matsBody.innerHTML = s.materials.map(m => `
        <tr data-uniquename="${m.uniquename}">
          <td style="font-weight:500">${m.display_name || m.uniquename}</td>
          <td style="text-align:right">${m.quantity_needed}</td>
          <td style="text-align:right"><input class="input" type="number" min="0" value="${m.unit_price || ''}" data-uniquename="${m.uniquename}" /></td>
          <td style="text-align:right" id="mat-${m.uniquename.replace(/@/g,'-')}">${m.total_cost ? Number(m.total_cost).toLocaleString() + ' s' : '—'}</td>
        </tr>
      `).join('');

      // Prefill crafting fee
      $('alchemy-craft-fee').value = s.crafting_fee_pct || 1.5;

      // Wire calculate button to use these values
      $('alchemy-calc-btn').addEventListener('click', () => calculateAlchemyProfit(s.session_id));

    } catch (e) {
      // ignore gracefully
    }
  }

  function setMetric(valueId, dateId, value, dateStr) {
    $(valueId).textContent = value == null ? "—" : Number(value).toLocaleString();
    $(dateId).textContent = dateStr ? String(dateStr).slice(0, 10) : "";
  }


  async function fetchRecommendation(item, city, days) {
    const panel = $("recommend-panel");
    const verdict = $("recommend-verdict");
    const details = $("recommend-details");
    panel.style.display = "none";
    verdict.innerHTML = "";
    details.innerHTML = "";

    try {
      const r = await fetch(
        `${BASE}/api/v1/marrow/recommend/${encodeURIComponent(item)}?city=${encodeURIComponent(city)}&quality=1&days=${encodeURIComponent(days)}`
      );
      if (!r.ok) return;
      const d = await r.json();

      const cls = d.recommended ? "verdict-yes" : "verdict-no";
      const icon = d.recommended ? "✅" : "❌";
      const trendText = d.bullish === true ? "📈 Rising" : d.bullish === false ? "📉 Falling" : "— No data";

      const staleWarning = d.stale_data ? `<div style="color: #dc2626; font-size: 0.85em; margin-top: 4px;">⚠️ Warning: Market data is >24h old</div>` : "";
      const historyStaleWarning = d.history_stale ? `<div style="color: #d97706; font-size: 0.85em; margin-top: 4px;">⚠️ History data is stale (>48h gap)</div>` : "";
      const sparseWarning = (d.history_count < 5 && d.history_count > 0) ? `<div style="color: #d97706; font-size: 0.85em; margin-top: 4px;">⚠️ Sparse history data (${d.history_count} points)</div>` : "";
      const transportWarning = d.transport_warning ? `<div style="color: #d97706; font-size: 0.85em; margin-top: 4px;">🚚 ${d.transport_warning}</div>` : "";
      const daysToSell = d.estimated_days_to_sell ? ` (~${d.estimated_days_to_sell} days)` : "";

      const confDisp = (d.confidence * 100).toFixed(0);
      const diffStr = d.price_diff_pct != null ? ` <span style="font-size: 0.8em; font-weight: normal; color: ${d.price_diff_pct > 50 ? '#dc2626' : '#6b7280'}">(${d.price_diff_pct > 0 ? '+' : ''}${d.price_diff_pct.toFixed(0)}%)</span>` : "";
      const priceColor = (d.price_diff_pct > 150) ? "#b91c1c" : (d.price_diff_pct > 50) ? "#b45309" : "#166534";

      let craftingSection = "";
      if (d.is_craftable) {
        const marginCls = d.crafting_margin_pct > 15 ? "green" : d.crafting_margin_pct > 0 ? "blue" : "red";
        const rows = d.crafting_materials.map(m => `
          <tr>
            <td style="color:#666">${m.display_name}</td>
            <td style="text-align:right">x${m.quantity}</td>
            <td style="text-align:right">${m.unit_price.toLocaleString()} s</td>
          </tr>
        `).join("");

        craftingSection = `
          <div class="verdict-stat" style="grid-column: span 2; margin-top: 8px; border-top: 1px dashed #e5e7eb; padding-top: 12px;">
            <div class="verdict-stat-label">Crafting Analysis (Batch)</div>
            <div class="verdict-stat-value" style="color:${d.crafting_margin_pct > 0 ? '#166534' : '#b91c1c'}">
              ${d.crafting_profit ? d.crafting_profit.toLocaleString() : '—'} s profit
              <span style="font-size:0.8em; font-weight:normal">(${d.crafting_margin_pct}% margin)</span>
            </div>
            <div class="verdict-materials">
              <table>
                ${rows}
              </table>
            </div>
          </div>
        `;
      }

      verdict.innerHTML = `
        <div class="${cls}">
          <div class="verdict-title">${icon} ${d.recommended ? `OPPORTUNITY (Q${d.quality})` : "SKIP"} <span style="font-size:0.75em;font-weight:normal;opacity:0.7;margin-left:8px">Score: ${confDisp}%</span></div>
          <div class="verdict-reason" style="margin-bottom: 8px;">${d.reason}</div>
          ${staleWarning}
          ${historyStaleWarning}
          ${sparseWarning}
          ${transportWarning}
          <div class="verdict-stats" style="display: grid; grid-template-columns: 1fr 1fr; gap: 8px;">
            <div class="verdict-stat">
              <div class="verdict-stat-label">Current Price</div>
              <div class="verdict-stat-value" style="color:${priceColor}">${Number(d.output_price).toLocaleString()} s${diffStr}</div>
            </div>
            <div class="verdict-stat">
              <div class="verdict-stat-label">Historical Avg</div>
              <div class="verdict-stat-value" style="color:#4b5563">${d.historical_avg ? Number(d.historical_avg).toLocaleString() + ' s' : '—'}</div>
            </div>
            <div class="verdict-stat">
              <div class="verdict-stat-label">Suggested Qty</div>
              <div class="verdict-stat-value">${d.suggested_qty}${daysToSell}</div>
            </div>
            <div class="verdict-stat">
              <div class="verdict-stat-label">Trend Line</div>
              <div class="verdict-stat-value">${trendText}</div>
            </div>
            <div class="verdict-stat">
              <div class="verdict-stat-label">Avg Volume</div>
              <div class="verdict-stat-value">${d.avg_daily_volume !== null ? d.avg_daily_volume.toFixed(1) : "—"}</div>
            </div>
            <div class="verdict-stat">
              <div class="verdict-stat-label">Volatility</div>
              <div class="verdict-stat-value">${d.price_volatility_pct !== null ? d.price_volatility_pct.toFixed(1) + '%' : "—"}</div>
            </div>
            ${craftingSection}
          </div>
        </div>
      `;

      panel.style.display = "block";
    } catch (e) {
      // silently fail
    }
  }

  async function fetchHistory() {
    const item = comboState["hist-item"]?.uniquename || "";
    const city = $("hist-city").value;
    const days = $("hist-days").value;
    const btn = $("hist-fetch");
    const placeholder = $("hist-placeholder");

    setError("hist-error", "");

    if (!item) {
      setError("hist-error", "Enter an item ID");
      return;
    }

    const old = btn.textContent;
    btn.disabled = true;
    btn.textContent = "Fetching…";

    try {
      const r = await fetch(
        `${BASE}/api/v1/marrow/history_bulk/${encodeURIComponent(item)}?city=${encodeURIComponent(city)}&days=${encodeURIComponent(days)}`
      );
      if (!r.ok) throw new Error("History fetch failed");
      const results = await r.json();
      
      const qualityNames = ["Normal", "Good", "Outstanding", "Excellent", "Masterpiece"];
      const colors = ["#9ca3af", "#22c55e", "#3b82f6", "#a855f7", "#eab308"];
      
      let allLabels = new Set();
      results.forEach(d => {
        if (d && d.points) d.points.forEach(p => allLabels.add(String(p.timestamp || "").slice(0, 10)));
      });
      const labels = Array.from(allLabels).sort();
      
      const datasets = [];
      let totalVolumes = new Array(labels.length).fill(0);
      
      // Calculate volumes first for the background dataset
      results.forEach((d) => {
        if (!d || !d.points) return;
        labels.forEach((label, idx) => {
            const pt = d.points.find(p => String(p.timestamp || "").slice(0, 10) === label);
            if (pt) totalVolumes[idx] += pt.item_count;
        });
      });

      // Volume as the first dataset (rendered background)
      datasets.push({
          label: "Total Volume",
          data: totalVolumes,
          type: "bar",
          backgroundColor: "rgba(107, 70, 193, 0.12)",
          yAxisID: "y1",
          order: 2
      });
      
      results.forEach((d, i) => {
        if (!d || !d.points || d.points.length === 0) return;
        
        const qPrices = labels.map(label => {
            const pt = d.points.find(p => String(p.timestamp || "").slice(0, 10) === label);
            return pt ? pt.avg_price : null;
        });
        
        datasets.push({
            label: `Q${d.quality}`,
            data: qPrices,
            borderColor: colors[d.quality-1],
            backgroundColor: "transparent",
            borderWidth: 2,
            tension: 0.3,
            spanGaps: true,
            yAxisID: "y",
            order: 1
        });
      });

      if (histChart) {
        histChart.destroy();
        histChart = null;
      }

      if (labels.length === 0) {
        placeholder.textContent = "No history points";
        placeholder.style.display = "flex";
      } else {
        placeholder.style.display = "none";
        const ctx = $("hist-chart").getContext("2d");
        histChart = new Chart(ctx, {
            type: "line",
            data: { labels: labels, datasets: datasets },
            options: {
                responsive: true, maintainAspectRatio: false, interaction: { mode: 'index', intersect: false },
                scales: {
                    y: { type: "linear", display: true, position: "left", title: { display: true, text: "Silver" }, ticks: { callback(value) { return Number(value).toLocaleString(); } } },
                    y1: { type: "linear", display: true, position: "right", title: { display: true, text: "Volume" }, grid: { drawOnChartArea: false }, beginAtZero: true }
                }
            }
        });
      }

      await fetchRecommendation(item, city, days);
    } catch (err) {
      console.error(err);
      setError("hist-error", err?.message || "Network error");
    } finally {
      btn.disabled = false;
      btn.textContent = old;
    }
  }


  async function addFavourite() {
    const input = $("fav-item");
    const id = input.value.trim();
    setError("fav-error", "");
    if (!id) return;

    try {
      // If the user typed a display name, try to resolve it to a uniquename via search.
      let payloadId = id;
      try {
        const sr = await fetch(`${BASE}/api/v1/marrow/search?q=${encodeURIComponent(id)}`);
        if (sr.ok) {
          const items = await sr.json();
          if (Array.isArray(items) && items.length > 0) {
            // Prefer exact match on display_name or uniquename, otherwise take first result.
            const exact = items.find(it => (it.display_name || '').toLowerCase() === id.toLowerCase() || it.uniquename.toLowerCase() === id.toLowerCase());
            payloadId = exact ? exact.uniquename : items[0].uniquename;
          }
        }
      } catch (e) {
        // ignore search failures and fall back to raw input
      }

      const r = await fetch(`${BASE}/api/v1/marrow/favourites`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ uniquename: payloadId }),
      });
      if (!r.ok) throw new Error("Failed to add favourite");
      input.value = "";
      await loadFavourites();
    } catch (err) {
      setError("fav-error", err?.message || "Failed to add favourite");
    }
  }

  async function removeFavourite(id) {
    const list = $("fav-list");
    try {
      const r = await fetch(`${BASE}/api/v1/marrow/favourites/${encodeURIComponent(id)}`, {
        method: "DELETE",
      });
      if (!r.ok) throw new Error();
      await loadFavourites();
    } catch {
      list.className = "error-msg";
      list.textContent = "Failed to remove favourite";
    }
  }

  async function loadFavourites() {
    const list = $("fav-list");
    list.className = "muted";
    list.textContent = "Loading...";
    try {
      const r = await fetch(`${BASE}/api/v1/marrow/favourites`);
      if (!r.ok) throw new Error();
      const favs = await r.json();
      if (!favs.length) {
        list.className = "muted";
        list.textContent = "No favourites yet";
        return;
      }

      list.className = "";
      // Resolve display names for each favourite; fall back to uniquename
      const rows = await Promise.all(
        favs.map(async (id) => {
          try {
            const r = await fetch(`${BASE}/api/v1/marrow/search?q=${encodeURIComponent(id)}`);
            if (r.ok) {
              const items = await r.json();
              const found = Array.isArray(items) ? items.find((it) => it.uniquename === id) : null;
              const disp = found ? (found.display_name || id) : id;
              return { id, disp };
            }
          } catch (e) {
            // ignore and fall back
          }
          return { id, disp: id };
        })
      );

      list.innerHTML = rows
        .map(
          ({ id, disp }) => `
          <div class="fav-row" data-uniquename="${id}">
            <span class="fav-name">${disp}</span>
            <button class="fav-remove" data-id="${id}">✕</button>
          </div>`
        )
        .join("");

      list.querySelectorAll(".fav-row").forEach((row) => {
        row.addEventListener("click", (e) => {
          if (e.target.closest(".fav-remove")) return;
          fillInputs(row.dataset.uniquename);
        });
      });
      list.querySelectorAll(".fav-remove").forEach((btn) => {
        btn.addEventListener("click", (e) => {
          e.stopPropagation();
          removeFavourite(btn.dataset.id);
        });
      });
    } catch {
      list.className = "error-msg";
      list.textContent = "Failed to load favourites";
    }
  }

  document.addEventListener("DOMContentLoaded", () => {
    $("hist-fetch").addEventListener("click", fetchHistory);
    $("fav-add").addEventListener("click", addFavourite);
    makeAutocomplete("hist-item", () => {});
    document.addEventListener("click", (e) => {
      ["hist-item"].forEach((id) => {
        const state = comboState[id];
        if (!state) return;
        if (!state.input.closest(".combo-wrap").contains(e.target)) {
          closeMenu(id);
        }
      });
    });

    loadGold();
    loadFavourites();

    // Direct-Link: Listen for live data from the sniffer
    if (window.__TAURI__) {
      const { listen } = window.__TAURI__.event;
      listen("marrow-ingest", (event) => {
        const itemIds = event.payload;
        const currentItem = comboState["price-item"]?.uniquename;
        
        if (currentItem && itemIds.includes(currentItem)) {
          console.log("[marrow-direct] Live update detected for", currentItem);
          const city = $("price-city").value;
          const days = $("hist-days").value;
          fetchRecommendation(currentItem, city, days);
        }
      });
    }

    setInterval(loadGold, 60_000);
  });

  // ── Alchemy session profit calculator ───────────────────────────────────────

  let alchemySession = null;

  function fmtS(n) {
    if (n == null) return "—";
    return Number(n).toLocaleString() + " s";
  }

  function renderAlchemySession(session) {
    alchemySession = session;
    const panel      = $("alchemy-session-panel");
    const badge      = $("alchemy-session-badge");
    const infoBar    = $("alchemy-info-bar");
    const itemsTbody = $("alchemy-items-body");
    const matsTbody  = $("alchemy-mats-body");

    badge.textContent = `Session #${session.session_id}`;
    infoBar.innerHTML = `
      <span>Account: <strong>${session.account_name}</strong></span>
      <span>City: <strong>${session.city}</strong></span>
      <span>Focus: <strong>${session.use_focus ? "Yes" : "No"}</strong></span>
      <span>RRR: <strong>${session.rrr_pct}%</strong></span>
      <span>${session.items.length} item${session.items.length !== 1 ? "s" : ""}</span>
    `;

    // Items table with sell-price inputs
    itemsTbody.innerHTML = session.items.map((item, idx) => `
      <tr data-idx="${idx}">
        <td>
          <div style="font-weight:500">${item.display_name}</div>
          <div style="font-size:11px;color:#aaa">${item.uniquename}</div>
        </td>
        <td style="text-align:right">${Number(item.quantity_out).toLocaleString()}</td>
        <td style="text-align:right">${Number(item.runs_needed).toLocaleString()}</td>
        <td style="text-align:right">
          <input class="input sell-price-input" type="number" min="0"
            id="sell-price-${idx}" placeholder="Enter price" />
        </td>
        <td style="text-align:right" id="revenue-${idx}">—</td>
      </tr>
    `).join("");

    itemsTbody.querySelectorAll(".sell-price-input").forEach(input => {
      input.addEventListener("input", () => {
        const idx   = Number(input.closest("tr").dataset.idx);
        const price = parseFloat(input.value) || 0;
        const qty   = session.items[idx].quantity_out;
        $(`revenue-${idx}`).textContent = price > 0 ? fmtS(price * qty) : "—";
      });
    });

    // Materials table — read-only from session
    matsTbody.innerHTML = session.materials.map(m => `
      <tr>
        <td>${m.display_name}</td>
        <td style="text-align:right">${Number(m.quantity_needed).toLocaleString()}</td>
        <td style="text-align:right">${m.unit_price != null ? fmtS(m.unit_price) : '<span style="color:#dc2626">not set</span>'}</td>
        <td style="text-align:right">${m.total_cost != null ? fmtS(Math.round(m.total_cost)) : "—"}</td>
      </tr>
    `).join("");

    panel.style.display = "block";
    panel.scrollIntoView({ behavior: "smooth", block: "start" });
  }

  function calcAlchemyProfit() {
    if (!alchemySession) return;

    const craftFeePct = (parseFloat($("alchemy-craft-fee").value) || 1.5) / 100;

    const itemResults = alchemySession.items.map((item, idx) => ({
      ...item,
      sell_price: parseFloat(document.getElementById(`sell-price-${idx}`)?.value) || 0,
    }));

    const missing = itemResults.filter(i => i.sell_price <= 0).map(i => i.display_name);
    if (missing.length) {
      alert(`Enter sell prices for:\n${missing.join("\n")}`);
      return;
    }

    let gross     = 0;
    let list_fee  = 0;
    let sales_tax = 0;
    for (const item of itemResults) {
      const rev  = item.sell_price * item.quantity_out;
      gross     += rev;
      list_fee  += rev * 0.025;
      sales_tax += rev * 0.03;
    }

    const mat_cost      = alchemySession.materials.reduce((s, m) => s + (m.total_cost || 0), 0);
    const mats_missing  = alchemySession.materials.some(m => m.unit_price == null);
    const craft_fee     = gross * craftFeePct;
    const net           = gross - mat_cost - craft_fee - list_fee - sales_tax;
    const margin        = gross > 0 ? ((net / gross) * 100).toFixed(1) : "0.0";
    const cls           = net >= 0 ? "green" : "red";

    $("alchemy-profit-grid").innerHTML = `
      <div class="profit-card ${cls}">
        <div class="profit-label">Net Profit</div>
        <div class="profit-value ${cls}">${net >= 0 ? "+" : ""}${fmtS(Math.round(net))}</div>
      </div>
      <div class="profit-card">
        <div class="profit-label">Gross Revenue</div>
        <div class="profit-value">${fmtS(Math.round(gross))}</div>
      </div>
      <div class="profit-card ${cls}">
        <div class="profit-label">Margin</div>
        <div class="profit-value ${cls}">${margin}%</div>
      </div>
      <div class="profit-card">
        <div class="profit-label">Material Cost</div>
        <div class="profit-value">${mats_missing ? "⚠ incomplete" : fmtS(Math.round(mat_cost))}</div>
      </div>
    `;

    $("alchemy-breakdown").innerHTML = `
      <div class="bd-row"><span>Gross Revenue</span><span>+ ${fmtS(Math.round(gross))}</span></div>
      <div class="bd-row"><span>Material Cost</span><span>− ${fmtS(Math.round(mat_cost))}${mats_missing ? " ⚠" : ""}</span></div>
      <div class="bd-row"><span>Crafting Fee (${(craftFeePct * 100).toFixed(1)}%)</span><span>− ${fmtS(Math.round(craft_fee))}</span></div>
      <div class="bd-row"><span>Listing Fee (2.5%)</span><span>− ${fmtS(Math.round(list_fee))}</span></div>
      <div class="bd-row"><span>Sales Tax (3%)</span><span>− ${fmtS(Math.round(sales_tax))}</span></div>
      <div class="bd-row"><span>Net Profit</span><span style="color:${net >= 0 ? "#166534" : "#b91c1c"}">${net >= 0 ? "+" : ""}${fmtS(Math.round(net))}</span></div>
    `;

    $("alchemy-profit-result").style.display = "block";
  }

  async function loadAlchemySessionIfPending() {
    const sessionId = sessionStorage.getItem("alchemy_session_id");
    if (!sessionId) return;
    const id = Number(sessionId);
    if (!id) return;
    try {
      // Prefer marrow's receive endpoint which returns the session payload
      const r = await fetch(`${BASE}/api/v1/marrow/session/${id}/receive`);
      if (!r.ok) return;
      const session = await r.json();
      renderAlchemySession(session);

      // Try to fetch settings to prefill crafting fee from the account profile
      try {
        const s = await fetch(`${BASE}/api/v1/settings`);
        if (s.ok) {
          const settings = await s.json();
          const account = (settings.accounts || []).find(a => a.name === session.account_name) || (settings.accounts || [])[0];
          if (account && account.crafting_fee_pct != null) {
            $("alchemy-craft-fee").value = account.crafting_fee_pct;
          }
        }
      } catch (e) {
        // ignore
      }

    } catch (e) {
      // server may not be up yet — silently ignore
    }
  }

  document.addEventListener("DOMContentLoaded", () => {
    $("alchemy-dismiss")?.addEventListener("click", () => {
      sessionStorage.removeItem("alchemy_session_id");
      $("alchemy-session-panel").style.display = "none";
      alchemySession = null;
    });
    $("alchemy-calc-btn")?.addEventListener("click", calcAlchemyProfit);
    loadAlchemySessionIfPending();
  });

})();

