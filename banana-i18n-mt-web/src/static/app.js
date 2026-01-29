// Global state
let sourceMessages = {};
let translations = {};
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

    // Reset translations for new file
    translations = {};

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

  // Reset translations when language changes
  translations = {};
  updateExportButton();

  // Update all textareas to be empty
  document.querySelectorAll(".translation-textarea").forEach((textarea) => {
    textarea.value = "";
  });

  // Reset all status indicators
  document.querySelectorAll(".message-status").forEach((status) => {
    status.textContent = "⏳ Pending";
    status.className = "message-status status-pending";
  });
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
  const div = document.createElement("div");
  div.className = "message-item";
  div.dataset.key = key;

  const summary = document.createElement("div");
  summary.className = "message-summary";
  summary.innerHTML = `
        <span class="message-key">${escapeHtml(key)}</span>
        <span class="message-status status-pending" id="status-${key}">⏳ Pending</span>
    `;

  // Toggle expand/collapse
  summary.addEventListener("click", () => {
    const content = div.querySelector(".message-content");
    if (content.style.display === "none") {
      content.style.display = "grid";
      summary.style.borderBottomColor = "#e9ecef";
    } else {
      content.style.display = "none";
    }
  });

  const content = document.createElement("div");
  content.className = "message-content";
  content.style.display = "none";

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
    `;

  // Listen for edits
  const textarea = transSection.querySelector("textarea");
  textarea.addEventListener("input", () => {
    handleTextareEdit(key);
  });

  content.appendChild(sourceSection);
  content.appendChild(transSection);

  div.appendChild(summary);
  div.appendChild(content);

  // Auto-expand and translate on first open
  summary.addEventListener(
    "click",
    async () => {
      const isNowOpen = content.style.display !== "none";
      if (isNowOpen && !translations[key] && !textarea.value) {
        await translateMessage(key);
      }
    },
    { once: true },
  );

  return div;
}

/**
 * Handle textarea edit
 */
function handleTextareEdit(key) {
  const status = document.getElementById(`status-${key}`);
  const textarea = document.getElementById(`trans-${key}`);

  if (textarea.value !== translations[key]) {
    status.textContent = "✏ Edited";
    status.className = "message-status status-edited";
  } else if (textarea.value) {
    status.textContent = "✓ Translated";
    status.className = "message-status status-translated";
  }

  updateExportButton();
}

/**
 * Translate a single message
 */
async function translateMessage(key) {
  const textarea = document.getElementById(`trans-${key}`);
  const status = document.getElementById(`status-${key}`);
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

    status.textContent = "✓ Translated";
    status.className = "message-status status-translated";

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

  // Collect all translations from textareas
  let translationCount = 0;
  for (const key of Object.keys(sourceMessages)) {
    const textarea = document.getElementById(`trans-${key}`);
    if (textarea && textarea.value) {
      output[key] = textarea.value;
      translationCount++;
    }
  }

  if (translationCount === 0) {
    showStatus("❌ No translations to export", "error");
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
    `✓ Exported ${translationCount} translations to ${currentTargetLang}.json`,
  );
}

/**
 * Update export button state
 */
function updateExportButton() {
  const hasTranslations = Object.values(
    document.querySelectorAll(".translation-textarea"),
  ).some((textarea) => textarea.value);
  exportBtn.disabled = !hasTranslations;
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

// Initialize
showStatus("👋 Ready to upload an i18n JSON file");
