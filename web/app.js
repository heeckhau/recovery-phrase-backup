import init, { convert, slip39_split, slip39_combine, slip39_inspect } from "./pkg/pkc_wasm.js";

function setupTabs(tabs) {
  for (const { button } of tabs) {
    button.addEventListener("click", () => {
      for (const tab of tabs) {
        const active = tab.button === button;
        tab.button.setAttribute("aria-selected", String(active));
        tab.panel.hidden = !active;
      }
    });
  }
}

setupTabs([
  { button: document.getElementById("page-tab-convert"), panel: document.getElementById("page-panel-convert") },
  { button: document.getElementById("page-tab-slip39"), panel: document.getElementById("page-panel-slip39") },
]);

setupTabs([
  { button: document.getElementById("tab-eth"), panel: document.getElementById("panel-eth") },
  { button: document.getElementById("tab-btc"), panel: document.getElementById("panel-btc") },
]);

setupTabs([
  { button: document.getElementById("tab-split"), panel: document.getElementById("panel-split") },
  { button: document.getElementById("tab-combine"), panel: document.getElementById("panel-combine") },
]);

// ---- Convert tab ----

const form = document.getElementById("convert-form");
const submitButton = form.querySelector("button[type=submit]");
const keyInput = document.getElementById("input-key");
const networkSelect = document.getElementById("input-network");
const compressedCheckbox = document.getElementById("input-compressed");
const errorBox = document.getElementById("error");

const outputs = {
  hex: document.getElementById("out-hex"),
  mnemonic: document.getElementById("out-mnemonic"),
  wif: document.getElementById("out-wif"),
  btc: document.getElementById("out-btc"),
  eth: document.getElementById("out-eth"),
};

function selectOnFocus(field) {
  field.addEventListener("focus", () => field.select());
}

for (const field of Object.values(outputs)) {
  selectOnFocus(field);
}

function clearOutputs() {
  for (const field of Object.values(outputs)) {
    field.value = "";
  }
}

function showError(box, message) {
  box.textContent = message;
  box.style.display = "block";
}

function clearError(box) {
  box.style.display = "none";
  box.textContent = "";
}

function runConversion() {
  clearError(errorBox);

  const input = keyInput.value.trim();
  if (!input) {
    showError(errorBox, "Enter a private key first.");
    clearOutputs();
    return;
  }

  try {
    const result = convert(input, networkSelect.value, compressedCheckbox.checked);
    outputs.hex.value = result.private_key_hex;
    outputs.mnemonic.value = result.mnemonic;
    outputs.wif.value = result.wif;
    outputs.btc.value = result.btc_address;
    outputs.eth.value = result.eth_address;
    networkSelect.value = result.network;
    compressedCheckbox.checked = result.format === "compressed";
  } catch (err) {
    showError(errorBox, String(err));
    clearOutputs();
  }
}

form.addEventListener("submit", (event) => {
  event.preventDefault();
  runConversion();
});

// ---- SLIP-39 Split tab ----

const splitForm = document.getElementById("split-form");
const splitKeyInput = document.getElementById("split-key");
const splitPassphraseInput = document.getElementById("split-passphrase");
const splitErrorBox = document.getElementById("split-error");
const splitResults = document.getElementById("split-results");
const groupRowsContainer = document.getElementById("group-rows");
const addGroupButton = document.getElementById("add-group");
const groupThresholdField = document.getElementById("group-threshold-field");
const groupThresholdSelect = document.getElementById("group-threshold");
const iterationExponentSelect = document.getElementById("iteration-exponent");

function fillNumericOptions(select, min, max, defaultValue) {
  select.innerHTML = "";
  for (let i = min; i <= max; i++) {
    const option = document.createElement("option");
    option.value = String(i);
    option.textContent = String(i);
    select.appendChild(option);
  }
  select.value = String(defaultValue);
}

// 1 matches the reference implementation, python-slip39 and seedkit (the
// spec sets no default).
fillNumericOptions(iterationExponentSelect, 0, 15, 1);

function relabelGroupRows() {
  const rows = groupRowsContainer.querySelectorAll(".group-row");
  rows.forEach((row, i) => {
    row.querySelector(".field-label").textContent = `Group ${i + 1}`;
    row.querySelector(".btn-remove").hidden = rows.length <= 1;
  });

  const previous = groupThresholdSelect.value || "1";
  const clamped = Math.min(Number(previous) || 1, rows.length);
  fillNumericOptions(groupThresholdSelect, 1, rows.length, clamped || 1);
  groupThresholdField.hidden = rows.length <= 1;
}

function addGroupRow(threshold, count) {
  const row = document.createElement("div");
  row.className = "group-row";

  const column = document.createElement("div");
  column.style.flex = "1";
  row.appendChild(column);

  const label = document.createElement("div");
  label.className = "field-label";
  column.appendChild(label);

  const wrap = document.createElement("div");
  wrap.style.display = "flex";
  wrap.style.alignItems = "center";
  wrap.style.gap = "0.5rem";

  const thresholdSelect = document.createElement("select");
  thresholdSelect.className = "inline group-threshold-input";
  const countSelect = document.createElement("select");
  countSelect.className = "inline group-count-input";

  function syncThresholdOptions() {
    const count = Number(countSelect.value);
    const current = Math.min(Number(thresholdSelect.value) || count, count);
    fillNumericOptions(thresholdSelect, 1, count, current || count);
  }

  fillNumericOptions(countSelect, 1, 16, count);
  syncThresholdOptions();
  thresholdSelect.value = String(threshold);
  countSelect.addEventListener("change", syncThresholdOptions);

  const ofText = document.createElement("span");
  ofText.textContent = "of";

  const membersText = document.createElement("span");
  membersText.textContent = "members";

  wrap.append(thresholdSelect, ofText, countSelect, membersText);
  column.appendChild(wrap);

  const removeButton = document.createElement("button");
  removeButton.type = "button";
  removeButton.className = "btn-remove";
  removeButton.textContent = "Remove";
  removeButton.addEventListener("click", () => {
    row.remove();
    relabelGroupRows();
  });
  row.appendChild(removeButton);

  groupRowsContainer.appendChild(row);
  relabelGroupRows();
}

addGroupButton.addEventListener("click", () => addGroupRow(2, 3));

function readGroupSpecs() {
  const rows = groupRowsContainer.querySelectorAll(".group-row");
  const specs = [];
  for (const row of rows) {
    specs.push(Number(row.querySelector(".group-threshold-input").value));
    specs.push(Number(row.querySelector(".group-count-input").value));
  }
  return new Uint8Array(specs);
}

function clearSplitResults() {
  splitResults.hidden = true;
  splitResults.innerHTML = "";
}

function renderSplitResults(groups) {
  splitResults.innerHTML = "";
  groups.forEach((groupText, groupIndex) => {
    const members = groupText.split("\n");
    const groupBox = document.createElement("div");
    groupBox.className = "group-result";

    const title = document.createElement("div");
    title.className = "group-result-title";
    title.textContent = `Group ${groupIndex + 1}`;
    groupBox.appendChild(title);

    members.forEach((mnemonic, memberIndex) => {
      const field = document.createElement("div");
      field.className = "field";

      const label = document.createElement("div");
      label.className = "field-label";
      label.textContent = `Share ${memberIndex + 1} of ${members.length}`;
      field.appendChild(label);

      const textarea = document.createElement("textarea");
      textarea.readOnly = true;
      textarea.spellcheck = false;
      textarea.className = "mono-output";
      textarea.rows = 4; // 33 words needs 4 lines at this width, not 3 — same clipping bug as the BIP39 field earlier
      textarea.value = mnemonic;
      selectOnFocus(textarea);
      field.appendChild(textarea);

      groupBox.appendChild(field);
    });

    splitResults.appendChild(groupBox);
  });
  splitResults.hidden = false;
}

splitForm.addEventListener("submit", (event) => {
  event.preventDefault();
  clearError(splitErrorBox);

  const key = splitKeyInput.value.trim();
  if (!key) {
    showError(splitErrorBox, "Enter a private key first.");
    clearSplitResults();
    return;
  }

  try {
    const groups = slip39_split(
      key,
      Number(groupThresholdSelect.value),
      readGroupSpecs(),
      splitPassphraseInput.value,
      Number(iterationExponentSelect.value),
    );
    renderSplitResults(groups);
  } catch (err) {
    showError(splitErrorBox, String(err));
    clearSplitResults();
  }
});

// ---- SLIP-39 Combine tab ----

const combineForm = document.getElementById("combine-form");
const combinePassphraseInput = document.getElementById("combine-passphrase");
const combineErrorBox = document.getElementById("combine-error");
const combineResults = document.getElementById("combine-results");
const mnemonicRowsContainer = document.getElementById("mnemonic-rows");
const addMnemonicButton = document.getElementById("add-mnemonic");

const combineHexLabel = document.getElementById("combine-out-hex-label");
const combineEthField = document.getElementById("combine-field-eth");
const combineBtcField = document.getElementById("combine-field-btc");
const combineOutputs = {
  hex: document.getElementById("combine-out-hex"),
  eth: document.getElementById("combine-out-eth"),
  btc: document.getElementById("combine-out-btc"),
};
for (const field of Object.values(combineOutputs)) {
  selectOnFocus(field);
}

function relabelMnemonicRows() {
  const rows = mnemonicRowsContainer.querySelectorAll(".mnemonic-row");
  rows.forEach((row, i) => {
    row.querySelector(".field-label").textContent = `Share ${i + 1}`;
    row.querySelector(".btn-remove").hidden = rows.length <= 1;
  });
}

function addMnemonicRow() {
  const row = document.createElement("div");
  row.className = "mnemonic-row";

  const field = document.createElement("div");
  field.style.flex = "1";
  const label = document.createElement("div");
  label.className = "field-label";
  field.appendChild(label);
  const textarea = document.createElement("textarea");
  textarea.className = "mono-output";
  textarea.rows = 5; // narrower than the split-results textareas (shares width with the Remove button), so needs more lines for the same 33 words
  textarea.autocomplete = "off";
  textarea.spellcheck = false;
  textarea.autocapitalize = "off";
  textarea.placeholder = "paste a mnemonic share";
  field.appendChild(textarea);

  const badge = document.createElement("div");
  badge.className = "share-badge";
  field.appendChild(badge);

  textarea.addEventListener("input", () => {
    try {
      const info = slip39_inspect(textarea.value);
      badge.textContent = `Group ${info.group_number} of ${info.group_count} · share ${info.member_number} (needs ${info.member_threshold} from this group, ${info.group_threshold} of ${info.group_count} groups overall)`;
    } catch {
      badge.textContent = "";
    }
  });

  row.appendChild(field);

  const removeButton = document.createElement("button");
  removeButton.type = "button";
  removeButton.className = "btn-remove";
  removeButton.textContent = "Remove";
  removeButton.addEventListener("click", () => {
    row.remove();
    relabelMnemonicRows();
  });
  row.appendChild(removeButton);

  mnemonicRowsContainer.appendChild(row);
  relabelMnemonicRows();
}

addMnemonicButton.addEventListener("click", addMnemonicRow);

function readMnemonics() {
  return [...mnemonicRowsContainer.querySelectorAll(".mnemonic-row textarea")]
    .map((t) => t.value.trim())
    .filter((v) => v.length > 0);
}

function clearCombineResults() {
  combineResults.hidden = true;
  combineOutputs.hex.value = "";
  combineOutputs.eth.value = "";
  combineOutputs.btc.value = "";
}

combineForm.addEventListener("submit", (event) => {
  event.preventDefault();
  clearError(combineErrorBox);

  const mnemonics = readMnemonics();
  if (mnemonics.length === 0) {
    showError(combineErrorBox, "Enter at least one mnemonic share.");
    clearCombineResults();
    return;
  }

  try {
    const hex = slip39_combine(mnemonics, combinePassphraseInput.value);
    // Only a 32-byte secret can be a private key; shares from other tools
    // may hold e.g. 16-byte BIP39 entropy, which has no addresses to show.
    const isKey = hex.length === 64;
    combineOutputs.hex.value = hex;
    combineHexLabel.textContent = isKey ? "Recovered private key (hex)" : `Recovered secret (hex, ${hex.length * 4} bits — not a private key)`;
    combineEthField.hidden = !isKey;
    combineBtcField.hidden = !isKey;
    if (isKey) {
      const derived = convert(hex, "mainnet", true);
      combineOutputs.eth.value = derived.eth_address;
      combineOutputs.btc.value = derived.btc_address;
    }
    combineResults.hidden = false;
  } catch (err) {
    showError(combineErrorBox, String(err));
    clearCombineResults();
  }
});

// ---- wasm init ----

submitButton.disabled = true;
submitButton.textContent = "Loading…";

init()
  .then(() => {
    submitButton.disabled = false;
    submitButton.textContent = "Convert";

    addGroupRow(2, 3);
    addMnemonicRow();
    addMnemonicRow();

    // BEGIN dev-only (scripts/build.py strips this from the release file)
    // ?test prefills the well-known "private key = 1" vector and converts
    // it immediately, for quick manual/visual testing. Never a real key —
    // it's the most famous toy vector, so it demonstrably holds no funds.
    if (new URLSearchParams(window.location.search).has("test")) {
      keyInput.value = "0000000000000000000000000000000000000000000000000000000000000001";
      runConversion();
    }
    // END dev-only
  })
  .catch((err) => {
    submitButton.textContent = "Failed to load";
    showError(errorBox, `Could not load the WebAssembly module: ${err}`);
  });
