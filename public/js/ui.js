// Docs MCP Web UI - UI Utilities

// Toast Notification
function showToast(message, type = "info") {
  window.elements.toast.textContent = message;
  window.elements.toast.className = `toast ${type}`;

  // Show
  setTimeout(() => {
    window.elements.toast.classList.remove("hidden");
  }, 10);

  // Hide after 3 seconds
  setTimeout(() => {
    window.elements.toast.classList.add("hidden");
  }, 3000);
}

// Escape HTML
function escapeHtml(text) {
  const div = document.createElement("div");
  div.textContent = text;
  return div.innerHTML;
}

// Event Listeners
function setupEventListeners() {
  // Refresh libraries
  document
    .getElementById("refresh-libraries")
    .addEventListener("click", loadLibraries);

  // Clear jobs
  document.getElementById("clear-jobs").addEventListener("click", clearJobs);

  // Scrape form
  window.elements.scrapeForm.addEventListener("submit", handleScrapeSubmit);

  // Search
  document.getElementById("search-btn").addEventListener("click", handleSearch);
  window.elements.searchQuery.addEventListener("keypress", (e) => {
    if (e.key === "Enter") handleSearch();
  });

  // Theme toggle
  const themeToggle = document.getElementById("theme-toggle");
  if (themeToggle) {
    themeToggle.addEventListener("click", toggleTheme);
  }

  // Language selector
  const langSelect = document.getElementById("language-select");
  if (langSelect) {
    langSelect.addEventListener("change", (e) => {
      setLanguage(e.target.value);
    });
  }
}

// Export functions
window.showToast = showToast;
window.escapeHtml = escapeHtml;
window.setupEventListeners = setupEventListeners;
