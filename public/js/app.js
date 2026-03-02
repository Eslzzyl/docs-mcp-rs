// Docs MCP Web UI - Main Application Entry Point

// Initialize
document.addEventListener("DOMContentLoaded", async () => {
  // Cache DOM elements
  window.elements.librariesList = document.getElementById("libraries-list");
  window.elements.jobsList = document.getElementById("jobs-list");
  window.elements.searchLibrary = document.getElementById("search-library");
  window.elements.searchQuery = document.getElementById("search-query");
  window.elements.searchResults = document.getElementById("search-results");
  window.elements.scrapeForm = document.getElementById("scrape-form");
  window.elements.toast = document.getElementById("toast");

  // Initialize i18n first
  await initI18n();

  // Initialize theme
  initTheme();

  // Setup event listeners
  setupEventListeners();

  // Initial data load
  loadLibraries();
  loadJobs();

  // Connect to SSE
  connectSSE();
});
