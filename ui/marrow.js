(() => {
  const BASE = "http://127.0.0.1:38991";
  let histChart = null;
  const comboState = {};

  const $ = (id) => document.getElementById(id);

  function setError(id, msg) {
    const el = $(id);
    if (!el) return;
    el.textContent = msg || "";
  }

  async function fillInputs(uniquename) {
    // Shared behavior: clicking a favourite fills both price and history item IDs.
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

    setComboValue("price-item", uniquename, displayName);
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
    const wrap = input.closest(".combo-wrap");
    const label = $(`${inputId}-label`);
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

  function setMetric(valueId, dateId, value, dateStr) {
    $(valueId).textContent = value == null ? "—" : Number(value).toLocaleString();
    $(dateId).textContent = dateStr ? String(dateStr).slice(0, 10) : "";
  }

  async function fetchPrice() {
    const item = comboState["price-item"]?.uniquename || "";
    const city = $("price-city").value;
    const btn = $("price-fetch");
    const source = $("price-source");

    setError("price-error", "");
    if (!item) {
      setError("price-error", "Enter an item ID");
      return;
    }

    const old = btn.textContent;
    btn.disabled = true;
    btn.textContent = "Fetching…";

    try {
      const fetchPromises = [1, 2, 3, 4, 5].map(q => 
        fetch(`${BASE}/api/v1/marrow/item/${encodeURIComponent(item)}?city=${encodeURIComponent(city)}&quality=${q}`)
          .then(res => res.ok ? res.json() : null)
      );
      const results = await Promise.all(fetchPromises);
      
      const qualityNames = ["Normal", "Good", "Outstanding", "Excellent", "Masterpiece"];
      const colors = ["#9ca3af", "#22c55e", "#3b82f6", "#a855f7", "#eab308"];
      
      const out = document.getElementById("price-output");
      out.style.display = "block";
      
      let tableRows = results.map((d, i) => {
        if (!d || !d.sell_price_min) return "";
        let sellPrice = d.sell_price_min ? d.sell_price_min.toLocaleString() : "—";
        let buyPrice = d.buy_price_max ? d.buy_price_max.toLocaleString() : "—";
        return `
          <tr style="border-bottom:1px solid #f3f4f6">
            <td style="padding:8px 0; font-weight:bold; color: ${colors[i]}">Q${i+1} ${qualityNames[i]}</td>
            <td style="text-align:right;color:#166534;">${sellPrice}</td>
            <td style="text-align:right;color:#991b1b;">${buyPrice}</td>
          </tr>
        `;
      }).join("");

      out.innerHTML = `
        <table style="width:100%; border-collapse:collapse;">
          <tr style="border-bottom:1px solid #e5e7eb">
            <th style="text-align:left;padding:8px 0">Quality</th>
            <th style="text-align:right;padding:8px 0">Sell (Min)</th>
            <th style="text-align:right;padding:8px 0">Buy (Max)</th>
          </tr>
          ${tableRows}
        </table>
      `;
      source.style.display = "none";
    } catch (err) {
      setError("price-error", err?.message || "Network error");
    } finally {
      btn.disabled = false;
      btn.textContent = old;
    }
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
    $("price-fetch").addEventListener("click", fetchPrice);
    $("hist-fetch").addEventListener("click", fetchHistory);
    $("fav-add").addEventListener("click", addFavourite);
    makeAutocomplete("price-item", () => {});
    makeAutocomplete("hist-item", () => {});
    document.addEventListener("click", (e) => {
      ["price-item", "hist-item"].forEach((id) => {
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
})();
