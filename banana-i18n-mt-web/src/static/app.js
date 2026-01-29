// Global state
let sourceMessages = {};
let translations = {};
let savedTranslations = {}; // Tracks which messages have been explicitly saved
let currentTargetLang = "es";
let isLoaded = false;

// DOM elements
const fileInput = document.getElementById("fileInput");
const fileName = document.getElementById("fileName");
const targetLangSelect = document.getElementById("targetLang");
const exportBtn = document.getElementById("exportBtn");
const messageList = document.getElementById("messageList");
const emptyState = document.getElementById("emptyState");
const statusBar = document.getElementById("statusBar");

// Event listeners
fileInput.addEventListener("change", handleFileUpload);
targetLangSelect.addEventListener("change", handleLanguageChange);
exportBtn.addEventListener("click", exportTranslations);

/**
 * Handle file upload
 */
async function handleFileUpload(event) {
  const file = event.target.files[0];
  if (!file) return;

  try {
    const text = await file.text();
    const data = JSON.parse(text);

    // Filter out @metadata and empty values
    sourceMessages = Object.entries(data)
      .filter(([key, value]) => key !== "@metadata" && value)
      .reduce((acc, [key, value]) => {
        acc[key] = value;
        return acc;
      }, {});

    if (Object.keys(sourceMessages).length === 0) {
      showStatus("! No messages found in file", "error");
      return;
    }

    // Reset translations and saved state for new file
    translations = {};
    savedTranslations = {};

    // Update UI
    fileName.textContent = `✓ ${file.name} (${Object.keys(sourceMessages).length} messages)`;
    emptyState.style.display = "none";
    messageList.style.display = "grid";
    isLoaded = true;

    renderMessageList();
    showStatus(
      `✓ Loaded ${Object.keys(sourceMessages).length} messages from ${file.name}`,
    );

    // Disable export until translations exist
    updateExportButton();
  } catch (error) {
    showStatus(`❌ Error reading file: ${error.message}`, "error");
    console.error("File upload error:", error);
  }
}

/**
 * Handle language change
 */
function handleLanguageChange(event) {
  currentTargetLang = event.target.value;
  showStatus(`🌍 Target language changed to ${currentTargetLang}`);

  // Reset all internal trackers for fresh translations
  translations = {};
  savedTranslations = {};
  updateExportButton();

  // Close all expanded messages
  document.querySelectorAll(".message-item[open]").forEach((item) => {
    item.open = false;
  });

  // Update all textareas to be empty
  document.querySelectorAll(".translation-textarea").forEach((textarea) => {
    textarea.value = "";
  });

  // Reset all status indicators to "Pending"
  document.querySelectorAll(".message-status").forEach((status) => {
    status.textContent = "⏳ Pending";
    status.className = "message-status status-pending";
  });

  // Hide/reset all save buttons
  document.querySelectorAll(".save-button").forEach((btn) => {
    btn.style.display = "none";
    btn.disabled = false;
    btn.textContent = "💾 Save";
  });

  // Hide all error messages
  document.querySelectorAll(".error-message").forEach((error) => {
    error.style.display = "none";
  });

  // Re-render message list to reset the "once" event listeners for auto-translation
  renderMessageList();
}

/**
 * Render all messages as detail elements
 */
function renderMessageList() {
  messageList.innerHTML = "";

  for (const [key, sourceMessage] of Object.entries(sourceMessages)) {
    const messageItem = createMessageItem(key, sourceMessage);
    messageList.appendChild(messageItem);
  }
}

/**
 * Create a single message detail element
 */
function createMessageItem(key, sourceMessage) {
  const messageContainerEl = document.createElement("details");
  messageContainerEl.name = "message-item";
  messageContainerEl.className = "message-item";
  messageContainerEl.dataset.key = key;

  const summary = document.createElement("summary");
  summary.className = "message-summary";
  summary.innerHTML = `
        <span class="message-key">${escapeHtml(key)}</span>
        <span class="message-status status-pending" id="status-${key}">⏳ Pending</span>
    `;

  const content = document.createElement("div");
  content.className = "message-content";

  // Source message section
  const sourceSection = document.createElement("div");
  sourceSection.className = "message-section";
  sourceSection.innerHTML = `
        <label>Source (en)</label>
        <div class="source-text">${escapeHtml(sourceMessage)}</div>
    `;

  // Translation section
  const transSection = document.createElement("div");
  transSection.className = "message-section";
  transSection.innerHTML = `
         <label>Translation (${currentTargetLang})</label>
         <textarea 
             class="translation-textarea" 
             id="trans-${key}" 
             placeholder="Translation will appear here..."
             data-key="${key}"
         ></textarea>
         <div class="error-message" id="error-${key}" style="display: none;"></div>
         <button class="save-button" id="save-${key}" data-key="${key}" style="display: none;">💾 Save</button>
     `;

  // Listen for edits
  const textarea = transSection.querySelector("textarea");
  textarea.addEventListener("input", () => {
    handleTextareEdit(key);
  });

  // Listen for save button clicks
  const saveButton = transSection.querySelector(".save-button");
  saveButton.addEventListener("click", () => {
    saveTranslation(key);
  });

  content.appendChild(sourceSection);
  content.appendChild(transSection);

  messageContainerEl.appendChild(summary);
  messageContainerEl.appendChild(content);

  // Auto-expand and translate on first open
  messageContainerEl.addEventListener(
    "toggle",
    async () => {
      if (!translations[key] && !textarea.value) {
        await translateMessage(key);
      }
    },
    { once: true },
  );

  return messageContainerEl;
}

/**
 * Handle textarea edit
 */
function handleTextareEdit(key) {
  const status = document.getElementById(`status-${key}`);
  const textarea = document.getElementById(`trans-${key}`);
  const saveButton = document.getElementById(`save-${key}`);

  // Compare against the saved version
  const isSaved = key in savedTranslations;
  const savedValue = savedTranslations[key] || "";

  if (textarea.value !== savedValue) {
    // Value differs from saved version
    if (isSaved) {
      // Was saved before, now edited
      status.textContent = "✏ Edited";
      status.className = "message-status status-edited";
    } else if (textarea.value === translations[key]) {
      // MT translation, not saved yet
      status.textContent = "! Needs Review";
      status.className = "message-status status-needs-review";
    } else {
      // Modified from MT or empty
      status.textContent = "✏ Edited";
      status.className = "message-status status-edited";
    }
    // Show save button when there's content
    if (textarea.value) {
      saveButton.style.display = "block";
    } else {
      saveButton.style.display = "none";
    }
  } else if (isSaved && textarea.value) {
    // Matches saved version and is not empty
    status.textContent = "✓ Translated";
    status.className = "message-status status-translated";
    saveButton.style.display = "none";
  }

  updateExportButton();
}

/**
 * Translate a single message
 */
async function translateMessage(key) {
  const textarea = document.getElementById(`trans-${key}`);
  const status = document.getElementById(`status-${key}`);
  const saveButton = document.getElementById(`save-${key}`);
  const errorDiv = document.getElementById(`error-${key}`);

  status.textContent = "🔄 Translating...";
  status.className = "message-status status-translating";
  errorDiv.style.display = "none";

  try {
    const response = await fetch("/api/translate", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        message: sourceMessages[key],
        target_language: currentTargetLang,
        key: key,
      }),
    });

    if (!response.ok) {
      const errorData = await response.json();
      throw new Error(errorData.error || "Translation failed");
    }

    const data = await response.json();
    textarea.value = data.translated;
    translations[key] = data.translated;

    // Show "Needs Review" status instead of "Translated"
    status.textContent = "! Needs Review";
    status.className = "message-status status-needs-review";

    // Show save button
    saveButton.style.display = "block";

    updateExportButton();
  } catch (error) {
    console.error("Translation error:", error);
    status.textContent = "! Error";
    status.className = "message-status status-error";

    const errorDiv = document.getElementById(`error-${key}`);
    errorDiv.textContent = `Error: ${error.message}`;
    errorDiv.style.display = "block";

    showStatus(`❌ Failed to translate "${key}": ${error.message}`, "error");
  }
}
/**
 * Save a translated message
 */
function saveTranslation(key) {
  const textarea = document.getElementById(`trans-${key}`);
  const status = document.getElementById(`status-${key}`);
  const saveButton = document.getElementById(`save-${key}`);

  if (!textarea.value.trim()) {
    showStatus(`! Cannot save empty translation for "${key}"`, "error");
    return;
  }

  // Store the current textarea value as saved
  savedTranslations[key] = textarea.value;

  // Update status to "Translated" (green)
  status.textContent = "✓ Translated";
  status.className = "message-status status-translated";

  // Hide save button
  saveButton.style.display = "none";

  updateExportButton();
  showStatus(`✓ Saved translation for "${key}"`);

  // Close current message and open next one
  const currentMessageItem = document.querySelector(
    `.message-item[data-key="${key}"]`,
  );
  if (currentMessageItem) {
    currentMessageItem.open = false;

    // Find and open the next message item
    const nextMessageItem = currentMessageItem.nextElementSibling;
    if (nextMessageItem && nextMessageItem.classList.contains("message-item")) {
      nextMessageItem.open = true;
    }
  }
}

/**
 * Export translations to JSON file
 */
function exportTranslations() {
  if (!isLoaded || Object.keys(sourceMessages).length === 0) {
    showStatus("❌ No messages to export", "error");
    return;
  }

  const output = {
    "@metadata": {
      authors: ["Machine Translation"],
      "last-updated": new Date().toISOString().split("T")[0],
      locale: currentTargetLang,
    },
  };

  // Collect only saved translations
  let translationCount = 0;
  for (const key of Object.keys(savedTranslations)) {
    const savedValue = savedTranslations[key];
    if (savedValue && savedValue.trim()) {
      output[key] = savedValue;
      translationCount++;
    }
  }

  if (translationCount === 0) {
    showStatus(
      "❌ No saved translations to export. Please save at least one translation.",
      "error",
    );
    return;
  }

  // Create blob and download
  const blob = new Blob([JSON.stringify(output, null, 4)], {
    type: "application/json",
  });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = `${currentTargetLang}.json`;
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
  URL.revokeObjectURL(url);

  showStatus(
    `✓ Exported ${translationCount} saved translations to ${currentTargetLang}.json`,
  );
}

/**
 * Update export button state
 */
function updateExportButton() {
  const hasSavedTranslations =
    Object.keys(savedTranslations).length > 0 &&
    Object.values(savedTranslations).some((value) => value && value.trim());
  exportBtn.disabled = !hasSavedTranslations;
}

/**
 * Show status message
 */
function showStatus(message, type = "info") {
  const statusDiv = document.createElement("div");
  statusDiv.className = "status-text";
  statusDiv.textContent = message;

  statusBar.innerHTML = "";
  statusBar.appendChild(statusDiv);

  // Auto-clear after 5 seconds
  if (type !== "error") {
    setTimeout(() => {
      statusDiv.style.opacity = "0";
      setTimeout(() => statusDiv.remove(), 300);
    }, 5000);
  }
}

/**
 * Escape HTML special characters
 */
function escapeHtml(text) {
  const map = {
    "&": "&amp;",
    "<": "&lt;",
    ">": "&gt;",
    '"': "&quot;",
    "'": "&#039;",
  };
  return text.replace(/[&<>"']/g, (m) => map[m]);
}

async function populateWikiDropdown() {
  try {
    const response = await fetch("/static/languages.json");
    if (!response.ok) {
      throw new Error(`HTTP error! status: ${response.status}`);
    }

    const wikis = await response.json();
    const wikiSelect = document.getElementById("targetLang");

    wikiSelect.innerHTML = "";

    wikis.forEach((wiki) => {
      const option = document.createElement("option");
      option.value = wiki.langcode;
      const displayName = `${wiki.langcode} - ${wiki.name}`;
      option.textContent = displayName;
      wikiSelect.appendChild(option);
    });

    console.log(`Loaded ${wikis.length} wikis to dropdown`);
  } catch (error) {
    console.error("Failed to load wiki list:", error);
    console.log("📋 Using fallback wiki list");
  }
}
document.addEventListener("DOMContentLoaded", async () => {
  // Initialize
  showStatus("👋 Ready to upload an i18n JSON file");
  await populateWikiDropdown();
});
