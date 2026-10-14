import init, { bip39_wordlist, phrase_check, slip39_combine, slip39_inspect, slip39_split, slip39_wordlist } from "./pkg/pkc_wasm.js";

// Everything secret lives in the DOM inputs and in `state`, and nowhere
// else: no storage, no clipboard writes. "Clear all" and closing the page
// wipe both.

const $ = (id) => document.getElementById(id);

function el(tag, className, text) {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (text !== undefined) node.textContent = text;
  return node;
}

function showError(box, message) {
  box.textContent = message;
  box.style.display = "block";
}

function clearError(box) {
  box.style.display = "none";
  box.textContent = "";
}

function fillOptions(select, values, selected) {
  select.innerHTML = "";
  for (const value of values) {
    const option = el("option", null, String(value));
    option.value = String(value);
    select.appendChild(option);
  }
  select.value = String(selected);
}

function range(from, to) {
  return Array.from({ length: to - from + 1 }, (_, i) => from + i);
}

// A uniformly random integer in [0, n), from the browser's CSPRNG.
function randomBelow(n) {
  const limit = Math.floor(0x100000000 / n) * n;
  const buffer = new Uint32Array(1);
  do {
    crypto.getRandomValues(buffer);
  } while (buffer[0] >= limit);
  return buffer[0] % n;
}

// ---- word lists and the word grid ----

const wordlists = { bip39: null, slip39: null };

function loadWordlist(kind, text, datalistId) {
  const words = text.split("\n");
  const datalist = $(datalistId);
  for (const word of words) {
    const option = el("option");
    option.value = word;
    datalist.appendChild(option);
  }
  // Both lists are designed so the first four letters identify a word.
  const byPrefix = new Map();
  for (const word of words) {
    const prefix = word.slice(0, 4);
    byPrefix.set(prefix, byPrefix.has(prefix) ? null : word);
  }
  wordlists[kind] = { set: new Set(words), byPrefix, datalistId };
}

// Completes a typed word from its unique 4-letter prefix, if it isn't a
// word already. Returns the word, or null if the input matches none.
function resolveWord(list, typed) {
  const value = typed.trim().toLowerCase();
  if (list.set.has(value)) return value;
  if (value.length >= 4) {
    const candidate = list.byPrefix.get(value.slice(0, 4));
    if (candidate && candidate.startsWith(value)) return candidate;
  }
  return null;
}

// A numbered grid of word inputs with autocomplete (a <datalist>), prefix
// completion on space/enter/blur, per-word validity, and paste-to-fill.
// `numbers` labels the cells (defaults to 1..count).
function createWordGrid(container, kind, { count, numbers, onChange, onPasteCount }) {
  const list = wordlists[kind];
  let inputs = [];

  function validate(input, complete) {
    if (complete) {
      const resolved = resolveWord(list, input.value);
      if (resolved) input.value = resolved;
    }
    const value = input.value.trim();
    input.classList.toggle("valid", value !== "" && list.set.has(value.toLowerCase()));
    input.classList.toggle("invalid", complete && value !== "" && !list.set.has(value.toLowerCase()));
  }

  function fillFrom(start, words) {
    words.forEach((word, i) => {
      const input = inputs[start + i];
      if (!input) return;
      input.value = word;
      validate(input, true);
    });
    const next = inputs[Math.min(start + words.length, inputs.length - 1)];
    next.focus();
    onChange?.();
  }

  function build(n, labels) {
    container.innerHTML = "";
    inputs = [];
    for (let i = 0; i < n; i++) {
      const cell = el("div", "word-cell");
      cell.appendChild(el("span", "num", String(labels ? labels[i] : i + 1)));
      const input = el("input");
      input.type = "text";
      input.autocomplete = "off";
      input.spellcheck = false;
      input.autocapitalize = "off";
      input.setAttribute("autocorrect", "off");
      input.setAttribute("list", list.datalistId);
      input.setAttribute("aria-label", `Word ${labels ? labels[i] : i + 1}`);
      input.addEventListener("input", () => {
        validate(input, false);
        onChange?.();
      });
      input.addEventListener("blur", () => {
        validate(input, true);
        onChange?.();
      });
      input.addEventListener("keydown", (event) => {
        if (event.key === " " || event.key === "Enter") {
          event.preventDefault();
          validate(input, true);
          onChange?.();
          if (i + 1 < inputs.length) inputs[i + 1].focus();
        }
      });
      input.addEventListener("paste", (event) => {
        const words = (event.clipboardData?.getData("text") || "").trim().split(/\s+/).filter(Boolean);
        if (words.length < 2) return;
        event.preventDefault();
        if (i === 0 && onPasteCount && words.length !== inputs.length && onPasteCount(words.length)) {
          // the grid was rebuilt for the pasted length
          inputs[0].focus();
          fillFrom(0, words);
          return;
        }
        fillFrom(i, words);
      });
      cell.appendChild(input);
      container.appendChild(cell);
      inputs.push(input);
    }
  }

  build(count, numbers);

  return {
    setCount(n) {
      build(n, null);
      onChange?.();
    },
    words() {
      return inputs.map((input) => input.value.trim().toLowerCase());
    },
    inputs() {
      return inputs;
    },
    complete() {
      return inputs.every((input) => list.set.has(input.value.trim().toLowerCase()));
    },
    clear() {
      for (const input of inputs) {
        input.value = "";
        input.classList.remove("valid", "invalid");
      }
      onChange?.();
    },
    focus() {
      inputs[0]?.focus();
    },
  };
}

// A read-only numbered grid, for showing a share or a phrase.
function renderWords(container, words) {
  container.innerHTML = "";
  words.forEach((word, i) => {
    const cell = el("div", "word-cell");
    cell.appendChild(el("span", "num", String(i + 1)));
    cell.appendChild(el("span", "word", word));
    container.appendChild(cell);
  });
}

// ---- share entry: one share at a time, with progress towards the threshold ----

const SHARE_LENGTHS = [20, 23, 27, 30, 33, 59];

// Builds the "type one share, add it, repeat" UI inside `container`.
// `fixedLength` hides the length picker (the backup's own test knows it).
function createShareEntry(container, { fixedLength, onProgress }) {
  container.innerHTML = "";
  let shares = [];

  const lengthLabel = el("label", null, "Words in this share");
  const lengthSelect = el("select", "inline");
  lengthSelect.id = `${container.id}-length`;
  lengthLabel.htmlFor = lengthSelect.id;
  fillOptions(lengthSelect, SHARE_LENGTHS, fixedLength || 20);
  if (fixedLength) {
    lengthLabel.hidden = true;
    lengthSelect.hidden = true;
  }
  const gridBox = el("div", "word-grid");
  const error = el("div", "error");
  const addButton = el("button", "btn-secondary", "Add this share");
  addButton.type = "button";
  addButton.disabled = true;
  const progress = el("p", "notice mt-075");
  const collected = el("ul", "collected");

  container.append(lengthLabel, lengthSelect, gridBox, error, addButton, progress, collected);

  const grid = createWordGrid(gridBox, "slip39", {
    count: Number(lengthSelect.value),
    onChange: () => {
      addButton.disabled = !grid.complete();
    },
    onPasteCount: fixedLength
      ? null
      : (n) => {
          if (!SHARE_LENGTHS.includes(n)) return false;
          lengthSelect.value = String(n);
          grid.setCount(n);
          return true;
        },
  });
  lengthSelect.addEventListener("change", () => grid.setCount(Number(lengthSelect.value)));

  function status() {
    if (shares.length === 0) return { enough: false, text: "" };
    const first = shares[0].info;
    const groups = new Map();
    for (const { info } of shares) {
      const group = groups.get(info.group_number) || { have: 0, need: info.member_threshold };
      group.have += 1;
      groups.set(info.group_number, group);
    }
    const complete = [...groups.values()].filter((g) => g.have >= g.need).length;
    const enough = complete >= first.group_threshold;
    let text;
    if (first.group_count === 1) {
      const g = groups.get(1);
      text = enough ? `You have ${g.have} of the ${g.need} shares needed. That's enough.` : `You have ${g.have} of the ${g.need} shares needed.`;
    } else {
      const parts = [...groups.entries()].map(([n, g]) => `group ${n}: ${g.have} of ${g.need}`);
      text = `${parts.join(", ")} · ${complete} of ${first.group_threshold} groups complete${enough ? ". That's enough." : "."}`;
    }
    return { enough, text };
  }

  function refresh() {
    collected.innerHTML = "";
    shares.forEach(({ info }) => {
      collected.appendChild(el("li", null, `Group ${info.group_number} of ${info.group_count} · share ${info.member_number}`));
    });
    const s = status();
    progress.textContent = s.text;
    onProgress?.(s.enough, shares.length);
  }

  addButton.addEventListener("click", () => {
    clearError(error);
    const mnemonic = grid.words().join(" ");
    let info;
    try {
      info = slip39_inspect(mnemonic);
    } catch (err) {
      showError(error, `This share doesn't check out (${err}). Compare each word with your copy, in order.`);
      return;
    }
    if (shares.some((s) => s.mnemonic === mnemonic)) {
      showError(error, "You've already added this share.");
      return;
    }
    if (shares.length > 0 && shares[0].info.group_count !== info.group_count) {
      showError(error, "This share is from a different backup than the ones already added.");
      return;
    }
    shares.push({ mnemonic, info });
    grid.clear();
    grid.focus();
    refresh();
  });

  return {
    mnemonics: () => shares.map((s) => s.mnemonic),
    reset() {
      shares = [];
      grid.clear();
      clearError(error);
      refresh();
    },
    grid,
  };
}

// ---- screens ----

const screens = { home: $("home"), backup: $("backup"), recover: $("recover") };

function showScreen(name) {
  for (const [key, node] of Object.entries(screens)) node.hidden = key !== name;
  window.scrollTo(0, 0);
}

function showStep(wizard, stepId, label) {
  for (const step of wizard.querySelectorAll(".step")) step.hidden = step.id !== stepId;
  wizard.querySelector(".progress").textContent = label;
  window.scrollTo(0, 0);
}

for (const button of document.querySelectorAll("[data-home]")) {
  button.addEventListener("click", () => clearAll());
}

// ---- back up: state ----

const BACKUP_STEPS = ["Your phrase", "Split", "Before you start", "Shares", "Test", "Done"];
const backupStep = (n) => showStep(screens.backup, `b${n}`, `Step ${n} of 6 · ${BACKUP_STEPS[n - 1]}`);

let state = freshState();

function freshState() {
  return {
    phrase: "",
    fingerprint: "",
    shareWords: 0,
    scheme: null, // { groupThreshold, groups: [[t, n], ...] }
    passphrase: "",
    shares: [], // [{ mnemonic, group, groupCount, member, members, threshold }]
    current: 0,
    checkPositions: [],
    checked: new Set(), // indices of shares whose spot check passed
    method: "hand", // how the shares are recorded: "hand" or "print"
    printed: false,
  };
}

// ---- step 1: the phrase ----

const b1Count = $("b1-count");
const b1Bip39 = $("b1-bip39");
const b1Ack = $("b1-ack");
let b1Grid;

const SHARE_WORDS_FOR_PHRASE = { 12: 20, 15: 23, 18: 27, 21: 30, 24: 33 };

function b1Update() {
  clearError($("b1-error"));
  const words = b1Grid.words();
  const filled = words.every((w) => w !== "");
  let check = null;
  if (b1Grid.complete()) {
    try {
      check = phrase_check(words.join(" "), b1Bip39.value);
    } catch {
      showError($("b1-error"), "Every word is valid, but the phrase's checksum doesn't match: one word is wrong or two are swapped. Compare each box with your copy.");
    }
  } else if (filled) {
    showError($("b1-error"), "Some words aren't on the BIP39 word list (marked in red).");
  }

  $("b1-check").hidden = !check;
  const warnings = $("b1-warnings");
  warnings.innerHTML = "";
  if (check) {
    $("b1-fingerprint").textContent = check.fingerprint;
    $("b1-eth").value = check.eth_address;
    $("b1-btc").value = check.btc_address;
    for (const warning of check.warnings) warnings.appendChild(el("div", "banner warning mt-075", warning));
    $("b1-ack-row").hidden = check.warnings.length === 0;
    state.phrase = words.join(" ");
    state.fingerprint = check.fingerprint;
    state.shareWords = SHARE_WORDS_FOR_PHRASE[check.word_count];
  } else {
    $("b1-eth").value = "";
    $("b1-btc").value = "";
    $("b1-ack-row").hidden = true;
    state.phrase = "";
  }
  const needsAck = check && check.warnings.length > 0;
  $("b1-next").disabled = !check || (needsAck && !b1Ack.checked);
}

b1Bip39.addEventListener("input", b1Update);
b1Ack.addEventListener("change", b1Update);
b1Count.addEventListener("change", () => b1Grid.setCount(Number(b1Count.value)));
$("b1-next").addEventListener("click", () => backupStep(2));

// ---- step 2: the scheme ----

const groupRowsContainer = $("group-rows");
const groupThresholdField = $("group-threshold-field");
const groupThresholdSelect = $("group-threshold");
const iterationExponentSelect = $("iteration-exponent");

// 1 matches the reference implementation, python-slip39 and seedkit (the
// spec sets no default).
fillOptions(iterationExponentSelect, range(0, 15), 1);

function relabelGroupRows() {
  const rows = groupRowsContainer.querySelectorAll(".group-row");
  rows.forEach((row, i) => {
    row.querySelector(".field-label").textContent = `Group ${i + 1}`;
    row.querySelector(".btn-remove").hidden = rows.length <= 1;
  });
  const clamped = Math.min(Number(groupThresholdSelect.value) || 1, rows.length);
  fillOptions(groupThresholdSelect, range(1, rows.length), clamped || 1);
  groupThresholdField.hidden = rows.length <= 1;
  b2Guard();
}

function addGroupRow(threshold, count) {
  const row = el("div", "group-row");
  const column = el("div");
  column.style.flex = "1";
  row.appendChild(column);
  column.appendChild(el("div", "field-label"));

  const wrap = el("div");
  wrap.style.display = "flex";
  wrap.style.alignItems = "center";
  wrap.style.gap = "0.5rem";
  const thresholdSelect = el("select", "inline group-threshold-input");
  const countSelect = el("select", "inline group-count-input");
  function syncThresholdOptions() {
    const n = Number(countSelect.value);
    const current = Math.min(Number(thresholdSelect.value) || n, n);
    fillOptions(thresholdSelect, range(1, n), current || n);
    b2Guard();
  }
  fillOptions(countSelect, range(1, 16), count);
  syncThresholdOptions();
  thresholdSelect.value = String(threshold);
  countSelect.addEventListener("change", syncThresholdOptions);
  thresholdSelect.addEventListener("change", b2Guard);
  wrap.append(thresholdSelect, el("span", null, "of"), countSelect, el("span", null, "shares"));
  column.appendChild(wrap);

  const removeButton = el("button", "btn-remove", "Remove");
  removeButton.type = "button";
  removeButton.addEventListener("click", () => {
    row.remove();
    relabelGroupRows();
  });
  row.appendChild(removeButton);
  groupRowsContainer.appendChild(row);
  relabelGroupRows();
}

$("add-group").addEventListener("click", () => addGroupRow(2, 3));
groupThresholdSelect.addEventListener("change", b2Guard);

function chosenScheme() {
  const choice = document.querySelector("input[name=scheme]:checked").value;
  if (choice === "2of3") return { groupThreshold: 1, groups: [[2, 3]] };
  if (choice === "3of5") return { groupThreshold: 1, groups: [[3, 5]] };
  const groups = [...groupRowsContainer.querySelectorAll(".group-row")].map((row) => [
    Number(row.querySelector(".group-threshold-input").value),
    Number(row.querySelector(".group-count-input").value),
  ]);
  return { groupThreshold: Number(groupThresholdSelect.value) || 1, groups };
}

// Plain-language warnings for schemes that are weaker than they look.
function schemeWarnings({ groupThreshold, groups }) {
  const warnings = [];
  const single = groups.length === 1;
  groups.forEach(([t, n], i) => {
    const where = single ? "" : ` in group ${i + 1}`;
    if (t === n && n > 1) warnings.push(`${t} of ${n}${where}: every share is needed, so losing any one of them loses ${single ? "everything" : "that group"}. That is riskier than a single copy.`);
    if (t === 1 && n === 1 && single) warnings.push("1 of 1: one share that is just your phrase in other words, with no protection against losing it.");
  });
  if (!single && groupThreshold === groups.length && groups.length > 1) {
    warnings.push(`Every group is needed (${groupThreshold} of ${groups.length}): losing any one group loses everything.`);
  }
  return warnings;
}

function b2Guard() {
  const guard = $("b2-guard");
  guard.innerHTML = "";
  for (const warning of schemeWarnings(chosenScheme())) guard.appendChild(el("div", "banner warning mt-075", warning));
}

for (const radio of document.querySelectorAll("input[name=scheme]")) {
  radio.addEventListener("change", () => {
    $("custom-scheme").hidden = radio.value !== "custom" || !radio.checked;
    b2Guard();
  });
}

$("b2-back").addEventListener("click", () => backupStep(1));
$("b2-next").addEventListener("click", () => {
  clearError($("b2-error"));
  state.scheme = chosenScheme();
  state.passphrase = $("split-passphrase").value;
  $("b3-summary").innerHTML = "";
  const { groupThreshold, groups } = state.scheme;
  const total = groups.reduce((sum, [, n]) => sum + n, 0);
  const lines =
    groups.length === 1
      ? [`You'll make ${total} shares. Any ${groups[0][0]} of them give your phrase back.`]
      : [
          `You'll make ${total} shares in ${groups.length} groups. Recovery needs ${groupThreshold} of the ${groups.length} groups:`,
          ...groups.map(([t, n], i) => `Group ${i + 1}: any ${t} of its ${n} shares.`),
        ];
  lines.push(`Every share carries the fingerprint ${state.fingerprint}.`);
  if (state.passphrase) lines.push("You set a SLIP-39 passphrase: recovery will need it exactly, and it is not written on the shares.");
  for (const line of lines) $("b3-summary").appendChild(el("p", null, line));
  backupStep(3);
});

// ---- step 3 → 4: make the shares ----

$("b3-back").addEventListener("click", () => backupStep(2));
$("b3-next").addEventListener("click", () => {
  state.method = document.querySelector("input[name=method]:checked").value;
  state.printed = false;
  const { groupThreshold, groups } = state.scheme;
  let result;
  try {
    result = slip39_split(state.phrase, groupThreshold, new Uint8Array(groups.flat()), state.passphrase, Number(iterationExponentSelect.value));
  } catch (err) {
    backupStep(2);
    showError($("b2-error"), String(err));
    return;
  }
  state.shares = [];
  result.forEach((groupText, g) => {
    const members = groupText.split("\n");
    members.forEach((mnemonic, m) => {
      state.shares.push({ mnemonic, group: g + 1, groupCount: groups.length, member: m + 1, members: members.length, threshold: groups[g][0] });
    });
  });
  state.current = 0;
  showShare();
});

// ---- step 4: one share at a time ----

let b4CheckGrid;

function shareLabel(share) {
  const { groupThreshold, groups } = state.scheme;
  if (groups.length === 1) return `Share ${share.member} of ${share.members} (any ${share.threshold} recover)`;
  return `Group ${share.group} of ${share.groupCount}, share ${share.member} of ${share.members} (any ${share.threshold} of this group; ${groupThreshold} of ${groups.length} groups recover)`;
}

function showShare() {
  const share = state.shares[state.current];
  backupStep(4);
  $("b4-title").textContent = shareLabel(share);
  // With groups, the label alone doesn't say how far along the user is.
  $("b4-sub").hidden = state.scheme.groups.length === 1;
  $("b4-sub").textContent = `Share ${state.current + 1} of ${state.shares.length}.`;
  const byHand = state.method === "hand";
  $("b4-howto").textContent = byHand
    ? "Write the words down in order, numbered as shown, on paper or a printed blank card."
    : "Print the shares (all at once), then check each printed card here.";
  $("b4-print-blank").hidden = !byHand;
  $("b4-print").hidden = byHand;
  $("b4-print-note").hidden = byHand;
  $("b4-written").textContent = byHand ? "I've written it down" : "Check this card";
  $("b4-fingerprint").textContent = state.fingerprint;
  renderWords($("b4-words"), share.mnemonic.split(" "));
  $("b4-show").hidden = false;
  $("b4-verify").hidden = true;
  $("b4-match").hidden = true;
  clearError($("b4-error"));
  updateShareNav();

  // Three distinct random positions, in order, for the spot check.
  const positions = new Set();
  while (positions.size < 3) positions.add(randomBelow(state.shareWords));
  state.checkPositions = [...positions].sort((a, b) => a - b);
}

// Back is always allowed; forward only past a share whose spot check has
// passed, so navigating can't become a way to skip checking.
function updateShareNav() {
  const last = state.shares.length - 1;
  const checked = state.checked.has(state.current);
  const allChecked = state.checked.size === state.shares.length;
  $("b4-checked").hidden = !checked;
  $("b4-written").hidden = checked;
  // A printed card can only be checked once something has been printed.
  $("b4-written").disabled = state.method === "print" && !state.printed;
  $("b4-print").textContent = state.printed ? "Print the shares again" : "Print the shares";
  $("b4-prev").hidden = state.current === 0;
  $("b4-skip").hidden = allChecked;
  const next = $("b4-next");
  next.textContent = state.current === last ? "Continue to the recovery test" : "Next share";
  next.disabled = state.current === last ? !allChecked : !checked;
}

$("b4-prev").addEventListener("click", () => {
  state.current -= 1;
  showShare();
});

$("b4-next").addEventListener("click", () => {
  if (state.current === state.shares.length - 1) startRecoveryTest();
  else {
    state.current += 1;
    showShare();
  }
});

$("b4-written").addEventListener("click", () => {
  $("b4-show").hidden = true;
  $("b4-verify").hidden = false;
  $("b4-match").hidden = true;
  clearError($("b4-error"));
  b4CheckGrid = createWordGrid($("b4-check-grid"), "slip39", {
    count: 3,
    numbers: state.checkPositions.map((p) => p + 1),
    onChange: spotCheck,
  });
  b4CheckGrid.focus();
});

$("b4-show-again").addEventListener("click", () => {
  $("b4-show").hidden = false;
  $("b4-verify").hidden = true;
  clearError($("b4-error"));
});

// Live feedback on the spot check: a word turns green as soon as it
// matches, and red once it is a complete word that doesn't (a half-typed
// word is just unfinished). When all three match, it moves on by itself.
let advancing = false;

function spotCheck() {
  if (advancing) return;
  const expected = state.shares[state.current].mnemonic.split(" ");
  const inputs = b4CheckGrid.inputs();
  const wrong = [];
  let matches = 0;
  inputs.forEach((input, i) => {
    const typed = input.value.trim().toLowerCase();
    const right = typed === expected[state.checkPositions[i]];
    const isWrong = !right && typed !== "" && (wordlists.slip39.set.has(typed) || input.classList.contains("invalid"));
    input.classList.toggle("valid", right);
    input.classList.toggle("invalid", isWrong);
    if (right) matches += 1;
    if (isWrong) wrong.push(state.checkPositions[i] + 1);
  });
  if (wrong.length > 0) {
    showError($("b4-error"), `Word ${wrong.join(" and ")} doesn't match your share. Compare your copy with the share again.`);
  } else {
    clearError($("b4-error"));
  }
  if (matches === inputs.length) {
    advancing = true;
    for (const input of inputs) input.disabled = true;
    $("b4-match").hidden = false;
    const checkedShare = state.current;
    setTimeout(() => {
      advancing = false;
      markChecked(checkedShare);
    }, 700);
  }
}

function markChecked(index) {
  state.checked.add(index);
  // If the user navigated away during the pause, just record the check.
  if (state.current !== index || $("b4").hidden) {
    if (!$("b4").hidden) updateShareNav();
    return;
  }
  const unchecked = state.shares.findIndex((_, i) => !state.checked.has(i));
  if (unchecked === -1) startRecoveryTest();
  else {
    state.current = unchecked;
    showShare();
  }
}

// Skipping the checks takes an explicit second confirmation; the recovery
// test that follows is the real proof and can still be done (or skipped).
$("b4-skip").addEventListener("click", () => {
  $("skip-ack").checked = false;
  $("skip-confirm").disabled = true;
  $("skip-dialog").showModal();
});
$("skip-ack").addEventListener("change", () => {
  $("skip-confirm").disabled = !$("skip-ack").checked;
});
$("skip-cancel").addEventListener("click", () => $("skip-dialog").close());
$("skip-confirm").addEventListener("click", () => {
  $("skip-dialog").close();
  state.shares.forEach((_, i) => state.checked.add(i));
  startRecoveryTest();
});

// ---- step 5: test the recovery ----

let b5Entry;

function startRecoveryTest() {
  backupStep(5);
  const { groupThreshold, groups } = state.scheme;
  $("b5-intro").textContent =
    groups.length === 1
      ? `Take any ${groups[0][0]} of the shares you wrote down and type them in, one at a time. This proves your written copies really give your phrase back.`
      : `Take enough of the shares you wrote down (${groupThreshold} of the groups, each with its own threshold) and type them in, one at a time. This proves your written copies really give your phrase back.`;
  $("b5-result").hidden = true;
  $("b5-next").hidden = true;
  b5Entry = createShareEntry($("b5-entry"), {
    fixedLength: state.shareWords,
    onProgress: (enough) => {
      if (!enough) return;
      const result = $("b5-result");
      result.hidden = false;
      let recovered = null;
      try {
        recovered = slip39_combine(b5Entry.mnemonics(), state.passphrase, "");
      } catch (err) {
        result.className = "banner warning";
        result.textContent = `These shares don't recover: ${err}. Check them against your written copies.`;
        return;
      }
      if (recovered.check && recovered.check.fingerprint === state.fingerprint) {
        result.className = "banner success";
        result.textContent = `Recovery works: your written shares give back the phrase with fingerprint ${state.fingerprint}.`;
        $("b5-next").hidden = false;
      } else {
        result.className = "banner warning";
        result.textContent = "These shares recover something, but not your phrase: one of them differs from what was shown. Go back to the shares and compare each with your copy.";
      }
    },
  });
}

$("b5-next").addEventListener("click", finishBackup);
$("b5-back").addEventListener("click", () => {
  state.current = 0;
  showShare();
});
$("b5-skip").addEventListener("click", finishBackup);

function finishBackup() {
  backupStep(6);
  $("b6-fingerprint").textContent = state.fingerprint;
}

$("b6-finish").addEventListener("click", () => clearAll());

// ---- printing: one card or sheet at a time ----

const printArea = $("print-area");
const printDialog = $("print-dialog");

function printPage(page) {
  printArea.innerHTML = "";
  printArea.appendChild(page);
  window.print();
}

// The print area holds a secret only while the print dialog is open.
window.addEventListener("afterprint", () => {
  printArea.innerHTML = "";
});

// One cut-out card per share, written for the friend who keeps it. It
// carries nothing a finder could use to plan: no position, no threshold,
// no group, no mention of a passphrase or what the backup is for. (The
// share's own first words still encode its group and threshold for SLIP-39
// software; that is part of the standard.) The owner's "how to recover"
// sheet holds those details. `withWords` false gives blank cards.
function shareCard(share, withWords) {
  const card = el("div", "print-card");
  card.appendChild(el("h2", null, "Recovery share"));
  const fp = el("p", "meta", "Fingerprint: ");
  fp.appendChild(el("span", "fp", state.fingerprint));
  card.appendChild(fp);
  const words = share.mnemonic.split(" ");
  const grid = el("div", "words");
  for (let i = 0; i < state.shareWords; i++) {
    const cell = el("div");
    cell.appendChild(el("span", "n", `${i + 1}.`));
    if (withWords) cell.appendChild(document.createTextNode(words[i]));
    grid.appendChild(cell);
  }
  card.appendChild(grid);
  const note = el("p", "instructions");
  note.appendChild(el("strong", null, "To whoever keeps this card: "));
  note.appendChild(
    document.createTextNode(
      "someone trusted you with it. It is one piece of a backup: useless on its own, but needed to restore it, so please don't throw it away. " +
        "Keep it somewhere safe and private, and don't copy or photograph it. " +
        "Give it back only to the person who gave it to you or, if something happens to them, to whoever they asked you to give it to.",
    ),
  );
  card.appendChild(note);
  return card;
}

function cardsPage(withWords, layout = "sheet") {
  const page = el("div", layout === "pages" ? "print-page cards one-per-page" : "print-page cards");
  const hint = layout === "pages" ? "One card per page. Keep each page in a different place." : "Cut along the dashed lines, and keep each card in a different place.";
  page.appendChild(el("p", "print-cut", hint));
  for (const share of state.shares) page.appendChild(shareCard(share, withWords));
  return page;
}

function recoverySheet() {
  const page = el("div", "print-page");
  page.appendChild(el("h1", null, "How to recover this wallet"));
  const fp = el("p", "meta", "Fingerprint of this backup: ");
  fp.appendChild(el("span", "fp", state.fingerprint));
  page.appendChild(fp);
  const { groupThreshold, groups } = state.scheme;
  const needed =
    groups.length === 1
      ? `any ${groups[0][0]} of the ${groups[0][1]} shares`
      : `${groupThreshold} of the ${groups.length} groups (${groups.map(([t, n], i) => `group ${i + 1}: ${t} of ${n}`).join("; ")})`;
  const lines = [
    `A wallet's recovery phrase was split into SLIP-39 shares: numbered word lists on separate cards. Recovering it needs ${needed}. This sheet holds no secret.`,
    "Gather the shares. Use a computer with no network connection.",
    "Open the Recovery Phrase Backup file, choose \"Recover a phrase\", and type the shares in one at a time.",
    "Check the fingerprint it shows matches the one above. If it doesn't, a share or the passphrase is wrong; nothing is reported as an error.",
    "The result is a recovery phrase (12 to 24 words). Restore it in a wallet with its \"import\" or \"restore\" option.",
    "Without that file: SLIP-39 is an open standard. Trezor's reference tool (python-shamir-mnemonic, \"shamir recover\") gives the secret as hex; that hex is the phrase's entropy, which any BIP39 tool turns back into the words.",
  ];
  if (state.passphrase) lines.splice(1, 0, "This backup uses a SLIP-39 passphrase. Recovery needs it exactly; it is kept separately from the shares.");
  const list = el("ol", "instructions");
  for (const line of lines) list.appendChild(el("li", null, line));
  page.appendChild(list);
  return page;
}

$("b4-print-blank").addEventListener("click", () => printPage(cardsPage(false)));
$("b4-print").addEventListener("click", () => printDialog.showModal());
$("print-cancel").addEventListener("click", () => printDialog.close());
$("print-confirm").addEventListener("click", () => {
  printDialog.close();
  printPage(cardsPage(true, document.querySelector("input[name=layout]:checked").value));
  state.printed = true;
  updateShareNav();
});
$("b6-print-sheet").addEventListener("click", () => printPage(recoverySheet()));

// ---- recover ----

let r1Entry;

function startRecover() {
  showScreen("recover");
  showStep(screens.recover, "r1", "Step 1 of 2 · Your shares");
  r1Entry = createShareEntry($("r1-entry"), {
    onProgress: (enough, count) => {
      $("r1-recover").disabled = count === 0;
      $("r1-recover").textContent = enough || count === 0 ? "Recover" : "Recover (may need more shares)";
    },
  });
  $("r1-recover").disabled = true;
  r1Entry.grid.focus();
}

$("r1-recover").addEventListener("click", () => {
  clearError($("r1-error"));
  let recovered;
  try {
    recovered = slip39_combine(r1Entry.mnemonics(), $("r1-passphrase").value, $("r1-bip39").value);
  } catch (err) {
    showError($("r1-error"), String(err));
    return;
  }
  showStep(screens.recover, "r2", "Step 2 of 2 · Result");
  const check = recovered.check;
  $("r2-phrase-block").hidden = !check;
  $("r2-mismatch").hidden = true;
  $("r2-addresses").hidden = true;
  if (check) {
    $("r2-title").textContent = `Your recovery phrase (${check.word_count} words)`;
    renderWords($("r2-words"), recovered.words.split(" "));
    $("r2-fingerprint").textContent = check.fingerprint;
    $("r2-eth").value = check.eth_address;
    $("r2-btc").value = check.btc_address;
  }
  $("r2-rawkey-block").hidden = !recovered.raw_key_hex;
  $("r2-rawkey").value = recovered.raw_key_hex || "";
  $("r2-rawkey-eth").value = recovered.raw_key_eth_address || "";
  $("r2-secret-block").hidden = !recovered.secret_hex;
  $("r2-secret").value = recovered.secret_hex || "";
});

$("r2-yes").addEventListener("click", () => {
  $("r2-mismatch").hidden = true;
  $("r2-addresses").hidden = false;
});
$("r2-no").addEventListener("click", () => {
  $("r2-mismatch").hidden = false;
  $("r2-addresses").hidden = true;
});
$("r2-back").addEventListener("click", () => showStep(screens.recover, "r1", "Step 1 of 2 · Your shares"));
$("r2-finish").addEventListener("click", () => clearAll());

// ---- home, clearing, and the network banner ----

function startBackup() {
  state = freshState();
  showScreen("backup");
  backupStep(1);
  b1Grid.setCount(Number(b1Count.value));
  b1Grid.focus();
}

$("start-backup").addEventListener("click", startBackup);
$("start-recover").addEventListener("click", startRecover);

function wipeFields() {
  for (const field of document.querySelectorAll("input, textarea")) {
    if (field.type === "checkbox" || field.type === "radio") field.checked = field.defaultChecked;
    else field.value = "";
  }
  for (const node of [$("b4-words"), $("r2-words"), $("r1-entry"), $("b5-entry"), $("b4-check-grid"), printArea]) {
    node.innerHTML = "";
  }
}

function clearAll() {
  wipeFields();
  state = freshState();
  $("custom-scheme").hidden = true;
  showScreen("home");
}

$("clear-all").addEventListener("click", () => {
  if (confirm("Erase every word, share and passphrase on this page and go back to the start?")) clearAll();
});

// Closing or navigating away wipes the fields, so a back/forward cache or
// form restoration can't bring the words back.
window.addEventListener("pagehide", wipeFields);

function updateOnlineBanner() {
  $("online-banner").hidden = !navigator.onLine;
}
window.addEventListener("online", updateOnlineBanner);
window.addEventListener("offline", updateOnlineBanner);
updateOnlineBanner();

// ---- start ----

init()
  .then(() => {
    loadWordlist("bip39", bip39_wordlist(), "bip39-words");
    loadWordlist("slip39", slip39_wordlist(), "slip39-words");

    fillOptions(b1Count, [12, 15, 18, 21, 24], 12);
    b1Grid = createWordGrid($("b1-grid"), "bip39", {
      count: 12,
      onChange: b1Update,
      onPasteCount: (n) => {
        if (![12, 15, 18, 21, 24].includes(n)) return false;
        b1Count.value = String(n);
        b1Grid.setCount(n);
        return true;
      },
    });
    addGroupRow(2, 3);

    $("start-backup").disabled = false;
    $("start-recover").disabled = false;

    // BEGIN dev-only (scripts/build.py strips this from the release file)
    // ?test starts a backup of the canonical all-zero test phrase, for quick
    // manual/visual testing. Never a real phrase: it's the most famous test
    // vector there is, so it demonstrably holds no funds.
    if (new URLSearchParams(window.location.search).has("test")) {
      startBackup();
      const words = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about".split(" ");
      $("b1-grid").querySelectorAll("input").forEach((input, i) => {
        input.value = words[i];
      });
      b1Update();
    }
    // END dev-only
  })
  .catch((err) => {
    showError($("load-error"), `Could not load the WebAssembly module: ${err}`);
  });
