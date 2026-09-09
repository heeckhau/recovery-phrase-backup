import init, { convert } from "./pkg/pkc_wasm.js";

const form = document.getElementById("convert-form");
const submitButton = form.querySelector("button[type=submit]");
const keyInput = document.getElementById("input-key");
const networkSelect = document.getElementById("input-network");
const compressedCheckbox = document.getElementById("input-compressed");
const errorBox = document.getElementById("error");

const outputs = {
  hex: document.getElementById("out-hex"),
  wif: document.getElementById("out-wif"),
  btc: document.getElementById("out-btc"),
  eth: document.getElementById("out-eth"),
};

for (const field of Object.values(outputs)) {
  field.addEventListener("focus", () => field.select());
}

const tabs = [
  { button: document.getElementById("tab-eth"), panel: document.getElementById("panel-eth") },
  { button: document.getElementById("tab-btc"), panel: document.getElementById("panel-btc") },
];

for (const { button } of tabs) {
  button.addEventListener("click", () => {
    for (const tab of tabs) {
      const active = tab.button === button;
      tab.button.setAttribute("aria-selected", String(active));
      tab.panel.hidden = !active;
    }
  });
}

function clearOutputs() {
  for (const field of Object.values(outputs)) {
    field.value = "";
  }
}

function showError(message) {
  errorBox.textContent = message;
  errorBox.style.display = "block";
  clearOutputs();
}

function clearError() {
  errorBox.style.display = "none";
  errorBox.textContent = "";
}

submitButton.disabled = true;
submitButton.textContent = "Loading…";

init()
  .then(() => {
    submitButton.disabled = false;
    submitButton.textContent = "Convert";
  })
  .catch((err) => {
    submitButton.textContent = "Failed to load";
    showError(`Could not load the WebAssembly module: ${err}`);
  });

form.addEventListener("submit", (event) => {
  event.preventDefault();
  clearError();

  const input = keyInput.value.trim();
  if (!input) {
    showError("Enter a private key first.");
    return;
  }

  try {
    const result = convert(input, networkSelect.value, compressedCheckbox.checked);
    outputs.hex.value = result.private_key_hex;
    outputs.wif.value = result.wif;
    outputs.btc.value = result.btc_address;
    outputs.eth.value = result.eth_address;
    networkSelect.value = result.network;
    compressedCheckbox.checked = result.format === "compressed";
  } catch (err) {
    showError(String(err));
  }
});
