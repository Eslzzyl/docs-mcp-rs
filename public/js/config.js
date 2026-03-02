// Docs MCP Web UI - Configuration

// API base URL
const API_BASE = "/api";

// State
let eventSource = null;
let currentLang = "en";
let translations = {};

// DOM Elements
const elements = {
  librariesList: null,
  jobsList: null,
  searchLibrary: null,
  searchQuery: null,
  searchResults: null,
  scrapeForm: null,
  toast: null,
};

// Export for other modules
window.API_BASE = API_BASE;
window.currentLang = currentLang;
window.translations = translations;
window.elements = elements;
