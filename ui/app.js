const API_BASE = "http://127.0.0.1:38991";

async function apiRequest(path, options = {}) {
  const res = await fetch(`${API_BASE}${path}`, {
    headers: { "Content-Type": "application/json" },
    ...options,
  });

  const text = await res.text();
  let data = null;
  if (text) {
    try {
      data = JSON.parse(text);
    } catch (err) {
      throw new Error(text);
    }
  }

  if (!res.ok) {
    const message = data && data.message ? data.message : res.statusText;
    throw new Error(message);
  }

  return data;
}

function $(id) {
  return document.getElementById(id);
}

function setOutput(id, payload) {
  const el = $(id);
  if (!el) return;
  el.textContent = JSON.stringify(payload, null, 2);
}

function parseLines(value) {
  return value
    .split("\n")
    .map((line) => line.trim())
    .filter(Boolean);
}

const ITEM_CATEGORIES = [
  { value: "sword",            label: "Sword",            city: "Lymhurst" },
  { value: "bow",              label: "Bow",              city: "Lymhurst" },
  { value: "arcane_staff",     label: "Arcane Staff",     city: "Lymhurst" },
  { value: "leather_headgear", label: "Leather Headgear", city: "Lymhurst" },
  { value: "leather_shoes",    label: "Leather Shoes",    city: "Lymhurst" },
  { value: "hammer",           label: "Hammer",           city: "Fort Sterling" },
  { value: "spear",            label: "Spear",            city: "Fort Sterling" },
  { value: "holy_staff",       label: "Holy Staff",       city: "Fort Sterling" },
  { value: "cloth_armor",      label: "Cloth Armor",      city: "Fort Sterling" },
  { value: "plate_headgear",   label: "Plate Headgear",   city: "Fort Sterling" },
  { value: "mace",             label: "Mace",             city: "Thetford" },
  { value: "nature_staff",     label: "Nature Staff",     city: "Thetford" },
  { value: "fire_staff",       label: "Fire Staff",       city: "Thetford" },
  { value: "leather_armor",    label: "Leather Armor",    city: "Thetford" },
  { value: "cloth_headgear",   label: "Cloth Headgear",   city: "Thetford" },
  { value: "axe",              label: "Axe",              city: "Martlock" },
  { value: "quarterstaff",     label: "Quarterstaff",     city: "Martlock" },
  { value: "frost_staff",      label: "Frost Staff",      city: "Martlock" },
  { value: "plate_shoes",      label: "Plate Shoes",      city: "Martlock" },
  { value: "offhand",          label: "Offhand",          city: "Martlock" },
  { value: "crossbow",         label: "Crossbow",         city: "Bridgewatch" },
  { value: "dagger",           label: "Dagger",           city: "Bridgewatch" },
  { value: "cursed_staff",     label: "Cursed Staff",     city: "Bridgewatch" },
  { value: "plate_armor",      label: "Plate Armor",      city: "Bridgewatch" },
  { value: "cloth_shoes",      label: "Cloth Shoes",      city: "Bridgewatch" },
];

const CITIES = ["Bridgewatch", "Caerleon", "Fort Sterling", "Lymhurst", "Martlock", "Thetford"];

/**
 * Renders the crafting-line checkboxes for one account card,
 * filtered to match the currently selected city.
 */
function renderCraftingLines(container, selectedCity, checkedLines) {
  const filtered = ITEM_CATEGORIES.filter(
    (cat) => cat.city.toLowerCase() === selectedCity.toLowerCase()
  );
  container.innerHTML = filtered.length === 0
    ? `<span style="color:var(--muted,#888);font-size:12px">No bonus categories for Caerleon</span>`
    : filtered.map((cat) => {
        const isChecked = checkedLines.includes(cat.value) ? "checked" : "";
        return `<label><input type="checkbox" value="${cat.value}" ${isChecked}> ${cat.label}</label>`;
      }).join("");
}

/**
 * Build a single account card element.
 * @param {number} idx  0-based index into settings.accounts
 * @param {object} acct AccountProfile data
 */
function buildAccountCard(idx, acct) {
  const card = document.createElement("div");
  card.className = "account-card";
  card.dataset.accountIdx = idx;

  // City options
  const cityOptions = CITIES.map((c) =>
    `<option value="${c}" ${c === acct.city ? "selected" : ""}>${c}</option>`
  ).join("");

  card.innerHTML = `
    <h3>${acct.name || "Account " + (idx + 1)}</h3>
    <div class="field">
      <label>Account Name</label>
      <input class="acct-name" type="text" value="${acct.name || ""}" placeholder="Warrior" />
    </div>
    <div class="field">
      <label>Home City</label>
      <select class="acct-city">${cityOptions}</select>
    </div>
    <div class="field">
      <label>Crafting Lines</label>
      <small>Only items your city bonus applies to are shown.</small>
      <div class="crafting-lines-group acct-crafting-lines"></div>
    </div>
    <div class="field">
      <label>
        <input class="acct-focus" type="checkbox" ${acct.use_focus ? "checked" : ""} />
        Use Focus
      </label>
    </div>
    <div class="field">
      <label>Crafting Fee %</label>
      <input class="acct-fee" type="number" min="0" max="10" step="0.1" value="${acct.crafting_fee_pct ?? 3.0}" />
    </div>
  `;

  // Render initial crafting lines
  const linesEl = card.querySelector(".acct-crafting-lines");
  renderCraftingLines(linesEl, acct.city, acct.crafting_lines || []);

  // Re-render when city changes (and clear selections)
  card.querySelector(".acct-city").addEventListener("change", (e) => {
    renderCraftingLines(linesEl, e.target.value, []);
  });

  return card;
}

/**
 * Reads the current form state of an account card into a plain object.
 */
function readAccountCard(card) {
  const checkedBoxes = Array.from(
    card.querySelectorAll(".acct-crafting-lines input[type='checkbox']:checked")
  ).map((cb) => cb.value);

  return {
    name: card.querySelector(".acct-name").value.trim() || "Account",
    city: card.querySelector(".acct-city").value,
    crafting_lines: checkedBoxes,
    use_focus: card.querySelector(".acct-focus").checked,
    crafting_fee_pct: Number(card.querySelector(".acct-fee").value || 3.0),
  };
}

async function initSettings() {
  let currentSettings = {};

  try {
    currentSettings = await apiRequest("/api/v1/settings");

    // General fields
    $("settings-language").value = currentSettings.language || "";
    $("settings-theme").value    = currentSettings.theme || "";
    $("settings-party-size").value  = currentSettings.seance_party_size ?? "";
    $("settings-split-type").value  = currentSettings.seance_split_type || "";

    // Server
    const serverEl = $("settings-server");
    if (serverEl && currentSettings.albion_server) {
      serverEl.value = currentSettings.albion_server;
    }

    // Account profiles
    const grid = $("accounts-grid");
    const accounts = currentSettings.accounts || [];
    // Ensure exactly 3 slots
    while (accounts.length < 3) {
      accounts.push({ name: `Account ${accounts.length + 1}`, city: "Lymhurst", crafting_lines: [], use_focus: false, crafting_fee_pct: 3.0 });
    }
    accounts.slice(0, 3).forEach((acct, idx) => {
      grid.appendChild(buildAccountCard(idx, acct));
    });

  } catch (err) {
    setOutput("settings-output", { error: err.message });
  }

  $("settings-save").addEventListener("click", async () => {
    // Collect accounts
    const accountCards = document.querySelectorAll(".account-card");
    const accounts = Array.from(accountCards).map(readAccountCard);

    const payload = {
      language:           $("settings-language").value,
      theme:              $("settings-theme").value,
      albion_server:      $("settings-server").value,
      seance_party_size:  Number($("settings-party-size").value || 0),
      seance_split_type:  $("settings-split-type").value,
      accounts,
    };

    try {
      const saved = await apiRequest("/api/v1/settings", {
        method: "PUT",
        body: JSON.stringify(payload),
      });
      setOutput("settings-output", saved);
    } catch (err) {
      setOutput("settings-output", { error: err.message });
    }
  });
}

async function initSeance() {
  let sessionId = null;

  $("seance-create").addEventListener("click", async () => {
    const payload = {
      party_size: Number($("seance-party-size").value || 0),
      total_loot_value: Number($("seance-total-loot").value || 0),
      split_type: $("seance-split-type").value,
      notes: $("seance-notes").value || null,
    };

    try {
      const result = await apiRequest("/api/v1/seance/session", {
        method: "POST",
        body: JSON.stringify(payload),
      });
      sessionId = result.id;
      $("seance-session-id").textContent = String(sessionId);
      setOutput("seance-output", result);
    } catch (err) {
      setOutput("seance-output", { error: err.message });
    }
  });

  $("seance-split").addEventListener("click", async () => {
    const players = parseLines($("seance-players").value).map((line) => {
      const parts = line.split(",").map((p) => p.trim());
      return {
        player_name: parts[0],
        weight: parts[1] ? Number(parts[1]) : null,
      };
    });

    const targetId = Number($("seance-session-input").value || sessionId || 0);

    try {
      const result = await apiRequest(`/api/v1/seance/session/${targetId}/split`, {
        method: "POST",
        body: JSON.stringify({ players }),
      });
      setOutput("seance-output", result);
    } catch (err) {
      setOutput("seance-output", { error: err.message });
    }
  });

  $("seance-wallet").addEventListener("click", async () => {
    const player = $("seance-wallet-player").value;
    try {
      const result = await apiRequest(`/api/v1/seance/wallet/${encodeURIComponent(player)}`);
      setOutput("seance-output", result);
    } catch (err) {
      setOutput("seance-output", { error: err.message });
    }
  });

  $("seance-withdraw").addEventListener("click", async () => {
    const payload = {
      player_name: $("seance-withdraw-player").value,
      amount: Number($("seance-withdraw-amount").value || 0),
      reason: $("seance-withdraw-reason").value,
      notes: $("seance-withdraw-notes").value || null,
    };

    try {
      const result = await apiRequest("/api/v1/seance/withdrawal", {
        method: "POST",
        body: JSON.stringify(payload),
      });
      setOutput("seance-output", result);
    } catch (err) {
      setOutput("seance-output", { error: err.message });
    }
  });

  $("seance-regear").addEventListener("click", async () => {
    const payload = {
      amount: Number($("seance-regear-amount").value || 0),
      reason: $("seance-regear-reason").value,
      notes: $("seance-regear-notes").value || null,
    };

    try {
      const result = await apiRequest("/api/v1/seance/regear", {
        method: "POST",
        body: JSON.stringify(payload),
      });
      setOutput("seance-output", result);
    } catch (err) {
      setOutput("seance-output", { error: err.message });
    }
  });

  $("seance-regear-refresh").addEventListener("click", async () => {
    try {
      const result = await apiRequest("/api/v1/seance/regear");
      setOutput("seance-output", result);
    } catch (err) {
      setOutput("seance-output", { error: err.message });
    }
  });
}

async function initAlchemy() {
  let lastCalculation = null;

  function buildPayload() {
    const materials = parseLines($("alchemy-materials").value).map((line) => {
      const parts = line.split(",").map((p) => p.trim());
      return {
        item_id: parts[0],
        quantity: Number(parts[1] || 0),
        unit_cost: parts[2] ? Number(parts[2]) : null,
      };
    });

    return {
      item_id: $("alchemy-item-id").value,
      city: $("alchemy-city").value,
      return_rate_pct: Number($("alchemy-return-rate").value || 0),
      crafting_fee_pct: Number($("alchemy-fee").value || 0),
      bonus_pct: Number($("alchemy-bonus").value || 0),
      materials,
    };
  }

  $("alchemy-calc").addEventListener("click", async () => {
    try {
      const result = await apiRequest("/api/v1/alchemy/calculate", {
        method: "POST",
        body: JSON.stringify(buildPayload()),
      });
      lastCalculation = result;
      setOutput("alchemy-output", result);
    } catch (err) {
      setOutput("alchemy-output", { error: err.message });
    }
  });

  $("alchemy-save").addEventListener("click", async () => {
    const name = $("alchemy-scenario-name").value;
    try {
      const result = await apiRequest("/api/v1/alchemy/scenarios", {
        method: "POST",
        body: JSON.stringify({ name, calculation: buildPayload() }),
      });
      setOutput("alchemy-output", result);
    } catch (err) {
      setOutput("alchemy-output", { error: err.message });
    }
  });

  $("alchemy-refresh").addEventListener("click", async () => {
    try {
      const result = await apiRequest("/api/v1/alchemy/scenarios");
      setOutput("alchemy-scenarios-output", result);
    } catch (err) {
      setOutput("alchemy-scenarios-output", { error: err.message });
    }
  });
}

window.addEventListener("DOMContentLoaded", () => {
  const page = document.body.dataset.page;
  if (page === "settings") initSettings();
  if (page === "seance") initSeance();

  if (page === "alchemy") initAlchemy();
});
