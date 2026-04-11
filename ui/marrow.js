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

  function fillInputs(uniquename) {
    // Shared behavior: clicking a favourite fills both price and history item IDs.
    setComboValue("price-item", uniquename);
    setComboValue("hist-item", uniquename);
  }

  function setComboValue(inputId, uniquename, displayName) {
    const state = comboState[inputId];
    if (!state) return;
    state.uniquename = uniquename || "";
    state.input.value = uniquename || "";
    state.label.textContent = uniquename
      ? `${displayName || uniquename} · ${uniquename}`
      : "";
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
        return `<div class="search-row${idx === state.activeIndex ? " active" : ""}" data-idx="${idx}">
          <span class="tier-badge">T${it.tier}</span>
          <span class="item-name">${it.display_name}</span>
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
    };

    input.addEventListener("input", () => {
      const state = comboState[inputId];
      const q = input.value.trim();
      if (!q) {
        clearTimeout(state.timer);
        state.uniquename = "";
        state.label.textContent = "";
        state.results = [];
        closeMenu(inputId);
        return;
      }
      if (state.uniquename && q !== state.uniquename) {
        state.uniquename = "";
        state.label.textContent = "";
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
        `${BASE}/api/v1/marrow/history/${encodeURIComponent(item)}?city=${encodeURIComponent(city)}&days=${encodeURIComponent(days)}`
      );
      if (!r.ok) throw new Error(`Failed to fetch history — ${r.status} ${r.statusText}`);
      const data = await r.json();

      const labels = (data.points || []).map((p) => String(p.timestamp || "").slice(0, 10));
      const values = (data.points || []).map((p) => Number(p.avg_price || 0));

      if (histChart) {
        // Destroy previous chart instance before creating a new one to avoid leaks.
        histChart.destroy();
        histChart = null;
      }

      if (!values.length) {
        placeholder.textContent = "No history points";
        placeholder.style.display = "flex";
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
            y: {
              ticks: {
                callback(value) {
                  return Number(value).toLocaleString();
                },
              },
            },
          },
          plugins: {
            tooltip: {
              callbacks: {
                label(context) {
                  const p = data.points[context.dataIndex] || {};
                  return `${Number(context.raw || 0).toLocaleString()} silver · vol ${p.item_count ?? 0}`;
                },
              },
            },
          },
        },
      });
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
      const r = await fetch(`${BASE}/api/v1/marrow/favourites`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ item_id: id }),
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
      list.innerHTML = favs
        .map(
          (id) => `
          <div class="fav-row" data-uniquename="${id}">
            <span class="fav-name">${id}</span>
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
