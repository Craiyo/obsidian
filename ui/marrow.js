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
    const quality = $("price-quality").value;
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
      const r = await fetch(
        `${BASE}/api/v1/marrow/item/${encodeURIComponent(item)}?city=${encodeURIComponent(city)}&quality=${encodeURIComponent(quality)}`
      );
      if (!r.ok) throw new Error(`Failed to fetch price — ${r.status} ${r.statusText}`);
      const data = await r.json();

      setMetric("price-sell-min", "price-sell-min-date", data.sell_price_min, data.sell_price_min_date);
      setMetric("price-sell-max", "price-sell-max-date", data.sell_price_max, null);
      setMetric("price-buy-min", "price-buy-min-date", data.buy_price_min, null);
      setMetric("price-buy-max", "price-buy-max-date", data.buy_price_max, data.buy_price_max_date);

      source.style.display = "inline-block";
      source.textContent = data.source;
      if (data.source === "cache") {
        source.style.background = "#ede9fe";
        source.style.color = "#6B46C1";
      } else {
        source.style.background = "#dcfce7";
        source.style.color = "#166534";
      }
    } catch (err) {
      setError("price-error", err?.message || "Network error");
    } finally {
      btn.disabled = false;
      btn.textContent = old;
    }
  }

  async function fetchRecommendation(item, city, quality, days) {
    const panel = $("recommend-panel");
    const verdict = $("recommend-verdict");
    const details = $("recommend-details");
    panel.style.display = "none";
    verdict.innerHTML = "";
    details.innerHTML = "";

    try {
      const r = await fetch(
        `${BASE}/api/v1/marrow/recommend/${encodeURIComponent(item)}?city=${encodeURIComponent(city)}&quality=${encodeURIComponent(quality)}&days=${encodeURIComponent(days)}`
      );
      if (!r.ok) return;
      const d = await r.json();

      const cls = d.recommended ? "verdict-yes" : "verdict-no";
      const icon = d.recommended ? "✅" : "❌";
      const trendText = d.bullish === true ? "📈 Rising" : d.bullish === false ? "📉 Falling" : "— No data";

      verdict.innerHTML = `
        <div class="${cls}">
          <div class="verdict-title">${icon} ${d.recommended ? "CRAFT IT" : "SKIP"}</div>
          <div class="verdict-reason">${d.reason}</div>
          <div class="verdict-stats">
            <div class="verdict-stat">
              <div class="verdict-stat-label">Profit / batch</div>
              <div class="verdict-stat-value" style="color:${d.unit_profit >= 0 ? '#166534' : '#dc2626'}">${Number(d.unit_profit.toFixed(0)).toLocaleString()} silver</div>
            </div>
            <div class="verdict-stat">
              <div class="verdict-stat-label">Suggested qty</div>
              <div class="verdict-stat-value">${d.suggested_qty}</div>
            </div>
            <div class="verdict-stat">
              <div class="verdict-stat-label">Trend</div>
              <div class="verdict-stat-value">${trendText}</div>
            </div>
            <div class="verdict-stat">
              <div class="verdict-stat-label">Material cost</div>
              <div class="verdict-stat-value">${Number(d.material_cost.toFixed(0)).toLocaleString()} silver</div>
            </div>
            <div class="verdict-stat">
              <div class="verdict-stat-label">Sell price</div>
              <div class="verdict-stat-value">${Number(d.output_price).toLocaleString()} silver</div>
            </div>
            <div class="verdict-stat">
              <div class="verdict-stat-label">Margin</div>
              <div class="verdict-stat-value">${d.profit_margin_pct.toFixed(1)}%</div>
            </div>
          </div>
          <div class="verdict-materials">
            <strong>Materials:</strong>
            <table>
              ${(d.materials || []).map(m => `
                <tr>
                  <td>${m.display_name}</td>
                  <td style="text-align:right">×${m.quantity}</td>
                  <td style="text-align:right">${Number(m.unit_price).toLocaleString()} ea</td>
                  <td style="text-align:right">${Number(m.total_cost.toFixed(0)).toLocaleString()} total</td>
                </tr>
              `).join("")}
            </table>
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
    const quality = $("hist-quality") ? $("hist-quality").value : ($("price-quality") ? $("price-quality").value : 1);
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
        `${BASE}/api/v1/marrow/history/${encodeURIComponent(item)}?city=${encodeURIComponent(city)}&days=${encodeURIComponent(days)}`
      );
      if (!r.ok) throw new Error(`Failed to fetch history — ${r.status} ${r.statusText}`);
      const data = await r.json();

      const labels = (data.points || []).map((p) => String(p.timestamp || "").slice(0, 10));
      const values = (data.points || []).map((p) => Number(p.avg_price || 0));

      if (histChart) {
        histChart.destroy();
        histChart = null;
      }

      if (!values.length) {
        placeholder.textContent = "No history points";
        placeholder.style.display = "flex";
        // still request recommendation even if history empty
        await fetchRecommendation(item, city, quality, days);
        btn.disabled = false;
        btn.textContent = old;
        return;
      }

      placeholder.style.display = "none";
      const ctx = $("hist-chart").getContext("2d");
      histChart = new Chart(ctx, {
        type: "line",
        data: {
          labels,
          datasets: [
            {
              label: `${data.uniquename} — ${data.city}`,
              data: values,
              borderColor: "#8b5cf6",
              borderWidth: 2,
              pointRadius: 0,
              fill: false,
              tension: 0.3,
            },
          ],
        },
        options: {
          responsive: true,
          maintainAspectRatio: false,
          scales: {
            y: { ticks: { callback(value) { return Number(value).toLocaleString(); } } },
          },
          plugins: {
            tooltip: { callbacks: { label(context) { const p = data.points[context.dataIndex] || {}; return `${Number(context.raw || 0).toLocaleString()} silver · vol ${p.item_count ?? 0}`; } } },
          },
        },
      });

      // request recommendation and render panel
      await fetchRecommendation(item, city, quality, days);

    } catch (err) {
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

    setInterval(loadGold, 60_000);
  });
})();
