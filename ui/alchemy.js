(() => {
  const BASE = "http://127.0.0.1:38991";
  const comboState = {};

  const $ = (id) => document.getElementById(id);

  function getBestCity(cat, sub) {
    cat = (cat || "").toLowerCase();
    sub = (sub || "").toLowerCase();

    if (cat === "consumables") {
      if (sub.includes("potion")) return "Brecilien";
      if (sub.includes("food")) return "Caerleon";
      return "Caerleon";
    }
    if (cat === "weapons") {
      if (["sword", "bow", "arcanestaff"].some(s => sub.includes(s))) return "Lymhurst";
      if (["axe"].some(s => sub.includes(s))) return "Martlock";
      if (["crossbow", "dagger", "cursestaff"].some(s => sub.includes(s))) return "Bridgewatch";
      if (["mace", "naturestaff", "firestaff"].some(s => sub.includes(s))) return "Thetford";
      if (["hammer", "spear", "holystaff", "froststaff"].some(s => sub.includes(s))) return "FortSterling";
      return "Caerleon";
    }
    if (cat === "offhands") {
      if (sub.includes("shield")) return "Martlock";
      return "Martlock"; // Most offhands like torches go to Martlock
    }
    if (cat === "armors") {
      if (sub.includes("plate_armor")) return "FortSterling";
      if (sub.includes("leather_armor")) return "Thetford";
      if (sub.includes("cloth_armor")) return "Martlock";
    }
    if (cat === "head") {
      if (sub.includes("leather_helmet")) return "Lymhurst";
      if (sub.includes("plate_helmet")) return "Bridgewatch";
      if (sub.includes("cloth_helmet")) return "Thetford";
    }
    if (cat === "shoes") {
      if (sub.includes("leather_shoes")) return "Lymhurst";
      if (sub.includes("plate_shoes")) return "Martlock";
      if (sub.includes("cloth_shoes")) return "FortSterling";
    }
    if (cat === "tool") return "Caerleon";
    
    return "Caerleon";
  }

  function setComboValue(inputId, uniquename, displayName) {
    const state = comboState[inputId];
    if (!state) return;
    state.uniquename = uniquename || "";
    state.input.value = displayName || uniquename || "";
    state.label.textContent = uniquename ? `${uniquename}` : "";
  }

  function renderMenu(inputId) {
    const state = comboState[inputId];
    if (!state) return;
    const { results, menu } = state;
    if (!results.length) {
      menu.style.display = "none";
      return;
    }
    menu.innerHTML = results
      .map((it, idx) => `
        <div class="search-row${idx === state.activeIndex ? " active" : ""}" data-idx="${idx}">
          <span class="tier-badge">T${it.tier}</span>
          <span class="item-name">${it.display_name}</span>
        </div>`)
      .join("");
    menu.style.display = "block";

    menu.querySelectorAll(".search-row").forEach(row => {
      row.addEventListener("mousedown", (e) => {
        e.preventDefault();
        const it = state.results[row.dataset.idx];
        state.onSelect(it);
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

    comboState[inputId] = { input, label, menu, onSelect, results: [], activeIndex: -1, timer: null };

    input.addEventListener("input", () => {
      const state = comboState[inputId];
      const q = input.value.trim();
      if (!q) {
        state.results = [];
        menu.style.display = "none";
        return;
      }
      clearTimeout(state.timer);
      state.timer = setTimeout(async () => {
        try {
          const r = await fetch(`${BASE}/api/v1/marrow/search?q=${encodeURIComponent(q)}`);
          state.results = await r.json();
          state.activeIndex = state.results.length ? 0 : -1;
          renderMenu(inputId);
        } catch {}
      }, 200);
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
      } else if (e.key === "Enter") {
        if (state.activeIndex >= 0) {
          e.preventDefault();
          const it = state.results[state.activeIndex];
          state.onSelect(it);
          setComboValue(inputId, it.uniquename, it.display_name);
        }
        menu.style.display = "none";
      } else if (e.key === "Escape") {
        menu.style.display = "none";
      }
    });

    input.addEventListener("blur", () => {
      // Delay to allow mousedown on menu items to fire first
      setTimeout(() => { menu.style.display = "none"; }, 200);
    });

    window.addEventListener("click", (e) => {
      if (!wrap.contains(e.target)) menu.style.display = "none";
    });
  }

  async function calculate() {
    const item = comboState["alchemy-item-id"]?.uniquename;
    if (!item) return;

    const useFocus = $("alchemy-focus").checked;
    const dailyBonus = $("alchemy-daily").checked;
    const isHideout = document.querySelector('input[name="location-type"]:checked').value === "hideout";
    const hideoutPower = $("alchemy-ho-power").value;
    const batchSize = $("alchemy-batch").value || 1;

    const btn = $("alchemy-calc");
    btn.disabled = true;
    btn.textContent = "Analyzing Masterchemy...";

    try {
      const r = await fetch(`${BASE}/api/v1/alchemy/analyze?item_id=${encodeURIComponent(item)}&batch_size=${batchSize}&use_focus=${useFocus}&daily_bonus=${dailyBonus}&is_hideout=${isHideout}&hideout_power=${hideoutPower}`);
      const data = await r.json();
      renderResults(data);
    } catch (err) {
      console.error(err);
    } finally {
      btn.disabled = false;
      btn.textContent = "Calculate yield";
    }
  }

  function renderResults(d) {
    const matList = $("materials-list");
    const resPanel = $("alchemy-result");

    const matRows = d.materials.map(m => `
      <tr>
        <td>${m.uniquename}</td>
        <td style="text-align:right">x${m.total_required}</td>
        <td style="text-align:right; color:var(--accent-2)">-${m.net_consumed.toFixed(1)}</td>
        <td style="text-align:right; color:#22c55e">+${m.return_amount.toFixed(1)}</td>
      </tr>
    `).join("");

    matList.innerHTML = `
      <table class="materials-table">
        <thead>
          <tr style="color:var(--muted); font-size:10px; text-transform:uppercase">
            <th style="text-align:left">Material</th>
            <th style="text-align:right">Initial</th>
            <th style="text-align:right">Net Consumed</th>
            <th style="text-align:right">Returned</th>
          </tr>
        </thead>
        <tbody>${matRows}</tbody>
      </table>
    `;

    resPanel.innerHTML = `
      <div style="text-align:center; margin-bottom:16px">
        <div class="stat-label">Production Yield for ${d.batch_size} Runs</div>
        <div class="profit-pill positive">${(d.craft_amount * d.batch_size * d.yield_multiplier).toFixed(1)} Items</div>
      </div>
      <div id="alchemy-result-stats">
        <div class="stat-box">
          <div class="stat-label">Resource Return Rate</div>
          <div class="stat-value" style="color:var(--accent-2)">${(d.rrr * 100).toFixed(1)}%</div>
        </div>
        <div class="stat-box">
          <div class="stat-label">Yield Multiplier</div>
          <div class="stat-value">x${d.yield_multiplier.toFixed(2)}</div>
        </div>
        <div class="stat-box">
          <div class="stat-label">Bonus City</div>
          <div class="stat-value">${d.best_city}</div>
        </div>
      </div>
    `;
  }

  document.addEventListener("DOMContentLoaded", () => {
    makeAutocomplete("alchemy-item-id", (it) => {
      // Auto-select city
      const best = getBestCity(it.shopcategory, it.shopsubcategory1);
      const citySelect = $("alchemy-city");
      if (citySelect) citySelect.value = best;
    });

    $("alchemy-calc").addEventListener("click", calculate);

    document.querySelectorAll('input[name="location-type"]').forEach(radio => {
      radio.addEventListener("change", (e) => {
        const isHO = e.target.value === "hideout";
        $("hideout-controls").style.display = isHO ? "flex" : "none";
        $("city-controls").style.display = isHO ? "none" : "flex";
      });
    });
  });
})();
