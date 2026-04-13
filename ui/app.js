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

async function initSettings() {
  try {
    const settings = await apiRequest("/api/v1/settings");
    $("settings-language").value = settings.language;
    $("settings-theme").value = settings.theme;
    $("settings-city").value = settings.default_city;
    $("settings-return-rate").value = settings.return_rate_pct;
    $("settings-fee").value = settings.crafting_fee_pct;
    $("settings-party-size").value = settings.seance_party_size;
    $("settings-split-type").value = settings.seance_split_type;
  } catch (err) {
    setOutput("settings-output", { error: err.message });
  }

  $("settings-save").addEventListener("click", async () => {
    const payload = {
      language: $("settings-language").value,
      theme: $("settings-theme").value,
      default_city: $("settings-city").value,
      return_rate_pct: Number($("settings-return-rate").value || 0),
      crafting_fee_pct: Number($("settings-fee").value || 0),
      seance_party_size: Number($("settings-party-size").value || 0),
      seance_split_type: $("settings-split-type").value,
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
