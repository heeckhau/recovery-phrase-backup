import init, { phrase_check, slip39_split, slip39_combine, slip39_inspect } from "./pkg/pkc_wasm.js";

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
  { button: document.getElementById("tab-split"), panel: document.getElementById("panel-split") },
  { button: document.getElementById("tab-combine"), panel: document.getElementById("panel-combine") },
]);

function selectOnFocus(field) {
  field.addEventListener("focus", () => field.select());
}

function showError(box, message) {
  box.textContent = message;
  box.style.display = "block";
}

function clearError(box) {
  box.style.display = "none";
  box.textContent = "";
}

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

// ---- Back up a phrase ----

const splitForm = document.getElementById("split-form");
const splitSubmit = splitForm.querySelector("button[type=submit]");
const splitPhraseInput = document.getElementById("split-phrase");
const splitBip39PassphraseInput = document.getElementById("split-bip39-passphrase");
const splitPassphraseInput = document.getElementById("split-passphrase");
const splitErrorBox = document.getElementById("split-error");
const splitResults = document.getElementById("split-results");
const groupRowsContainer = document.getElementById("group-rows");
const addGroupButton = document.getElementById("add-group");
const groupThresholdField = document.getElementById("group-threshold-field");
const groupThresholdSelect = document.getElementById("group-threshold");
const iterationExponentSelect = document.getElementById("iteration-exponent");

const splitCheck = document.getElementById("split-check");
const splitCheckTitle = document.getElementById("split-check-title");
const splitOutputs = {
  fingerprint: document.getElementById("split-out-fingerprint"),
  eth: document.getElementById("split-out-eth"),
  btc: document.getElementById("split-out-btc"),
};
for (const field of Object.values(splitOutputs)) {
  selectOnFocus(field);
}

// 1 matches the reference implementation, python-slip39 and seedkit (the
// spec sets no default).
fillNumericOptions(iterationExponentSelect, 0, 15, 1);

// Shows what identifies the phrase as soon as it parses, so the user can
// confirm they typed their own phrase before anything is split. Silent
// while it doesn't parse yet: half-typed input isn't an error.
function updatePhraseCheck() {
  try {
    const check = phrase_check(splitPhraseInput.value, splitBip39PassphraseInput.value);
    splitCheckTitle.textContent = `${check.word_count}-word phrase · fingerprint (goes on every share)`;
    splitOutputs.fingerprint.value = check.fingerprint;
    splitOutputs.eth.value = check.eth_address;
    splitOutputs.btc.value = check.btc_address;
    splitCheck.hidden = false;
    return check;
  } catch {
    splitCheck.hidden = true;
    for (const field of Object.values(splitOutputs)) {
      field.value = "";
    }
    return null;
  }
}

splitPhraseInput.addEventListener("input", updatePhraseCheck);
splitBip39PassphraseInput.addEventListener("input", updatePhraseCheck);

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
  membersText.textContent = "shares";

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

function renderSplitResults(groups, fingerprint) {
  splitResults.innerHTML = "";

  const intro = document.createElement("p");
  intro.className = "notice mt-0";
  intro.textContent = `Fingerprint ${fingerprint}: write it on every share. It shows the shares belong together, and a recovery must reproduce it.`;
  splitResults.appendChild(intro);

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
      textarea.rows = 4; // a 24-word phrase gives 33-word shares: 4 lines at this width
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

  if (!splitPhraseInput.value.trim()) {
    showError(splitErrorBox, "Enter your recovery phrase first.");
    clearSplitResults();
    return;
  }

  try {
    // Parse first, so a typo is reported as a phrase error rather than
    // anything about shares.
    const check = phrase_check(splitPhraseInput.value, splitBip39PassphraseInput.value);
    const groups = slip39_split(
      splitPhraseInput.value,
      Number(groupThresholdSelect.value),
      readGroupSpecs(),
      splitPassphraseInput.value,
      Number(iterationExponentSelect.value),
    );
    renderSplitResults(groups, check.fingerprint);
  } catch (err) {
    showError(splitErrorBox, String(err));
    clearSplitResults();
  }
});

// ---- Recover a phrase ----

const combineForm = document.getElementById("combine-form");
const combinePassphraseInput = document.getElementById("combine-passphrase");
const combineBip39PassphraseInput = document.getElementById("combine-bip39-passphrase");
const combineErrorBox = document.getElementById("combine-error");
const combineResults = document.getElementById("combine-results");
const mnemonicRowsContainer = document.getElementById("mnemonic-rows");
const addMnemonicButton = document.getElementById("add-mnemonic");

const combineBlocks = {
  phrase: document.getElementById("combine-phrase-block"),
  rawKey: document.getElementById("combine-rawkey-block"),
  secret: document.getElementById("combine-secret-block"),
};
const combineWordsLabel = document.getElementById("combine-words-label");
const combineOutputs = {
  words: document.getElementById("combine-out-words"),
  fingerprint: document.getElementById("combine-out-fingerprint"),
  eth: document.getElementById("combine-out-eth"),
  btc: document.getElementById("combine-out-btc"),
  rawKey: document.getElementById("combine-out-rawkey"),
  rawKeyEth: document.getElementById("combine-out-rawkey-eth"),
  secret: document.getElementById("combine-out-secret"),
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
  textarea.placeholder = "type or paste one share";
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
  for (const field of Object.values(combineOutputs)) {
    field.value = "";
  }
}

combineForm.addEventListener("submit", (event) => {
  event.preventDefault();
  clearError(combineErrorBox);
  clearCombineResults();

  const mnemonics = readMnemonics();
  if (mnemonics.length === 0) {
    showError(combineErrorBox, "Enter at least one share.");
    return;
  }

  try {
    const recovered = slip39_combine(mnemonics, combinePassphraseInput.value, combineBip39PassphraseInput.value);

    const check = recovered.check;
    combineBlocks.phrase.hidden = !check;
    if (check) {
      combineWordsLabel.textContent = `Recovered phrase (${check.word_count} words)`;
      combineOutputs.words.value = recovered.words;
      combineOutputs.fingerprint.value = check.fingerprint;
      combineOutputs.eth.value = check.eth_address;
      combineOutputs.btc.value = check.btc_address;
    }

    combineBlocks.rawKey.hidden = !recovered.raw_key_hex;
    if (recovered.raw_key_hex) {
      combineOutputs.rawKey.value = recovered.raw_key_hex;
      combineOutputs.rawKeyEth.value = recovered.raw_key_eth_address;
    }

    combineBlocks.secret.hidden = !recovered.secret_hex;
    if (recovered.secret_hex) {
      combineOutputs.secret.value = recovered.secret_hex;
    }

    combineResults.hidden = false;
  } catch (err) {
    showError(combineErrorBox, String(err));
  }
});

// ---- wasm init ----

splitSubmit.disabled = true;
splitSubmit.textContent = "Loading…";

init()
  .then(() => {
    splitSubmit.disabled = false;
    splitSubmit.textContent = "Split";

    addGroupRow(2, 3);
    addMnemonicRow();
    addMnemonicRow();

    // BEGIN dev-only (scripts/build.py strips this from the release file)
    // ?test prefills the canonical all-zero test phrase, for quick
    // manual/visual testing. Never a real phrase — it's the most famous
    // test vector there is, so it demonstrably holds no funds.
    if (new URLSearchParams(window.location.search).has("test")) {
      splitPhraseInput.value =
        "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
      updatePhraseCheck();
    }
    // END dev-only
  })
  .catch((err) => {
    splitSubmit.textContent = "Failed to load";
    showError(splitErrorBox, `Could not load the WebAssembly module: ${err}`);
  });
