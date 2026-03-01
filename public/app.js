// Docs MCP Web UI - Pure JavaScript (no framework)

// API base URL
const API_BASE = "/api";

// State
let eventSource = null;

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

// Initialize
document.addEventListener("DOMContentLoaded", () => {
  // Cache DOM elements
  elements.librariesList = document.getElementById("libraries-list");
  elements.jobsList = document.getElementById("jobs-list");
  elements.searchLibrary = document.getElementById("search-library");
  elements.searchQuery = document.getElementById("search-query");
  elements.searchResults = document.getElementById("search-results");
  elements.scrapeForm = document.getElementById("scrape-form");
  elements.toast = document.getElementById("toast");

  // Initialize theme
  initTheme();

  // Setup tab navigation
  setupTabs();

  // Setup event listeners
  setupEventListeners();

  // Initial data load
  loadLibraries();
  loadJobs();

  // Connect to SSE
  connectSSE();
});

// Theme Management
function initTheme() {
  // Check for saved theme preference
  const savedTheme = localStorage.getItem("theme");

  if (savedTheme) {
    // Use saved preference
    document.documentElement.setAttribute("data-theme", savedTheme);
  }
  // If no saved preference, let CSS media query handle it (auto-detect)
}

function toggleTheme() {
  const currentTheme = document.documentElement.getAttribute("data-theme");
  const prefersDark = window.matchMedia("(prefers-color-scheme: dark)").matches;

  let newTheme;
  if (currentTheme === "light") {
    newTheme = "dark";
  } else if (currentTheme === "dark") {
    newTheme = "light";
  } else {
    // No explicit theme set, check system preference
    newTheme = prefersDark ? "light" : "dark";
  }

  document.documentElement.setAttribute("data-theme", newTheme);
  localStorage.setItem("theme", newTheme);
}

// Tab Navigation
function setupTabs() {
  const tabs = document.querySelectorAll(".tab");
  tabs.forEach((tab) => {
    tab.addEventListener("click", () => {
      // Update active tab
      tabs.forEach((t) => t.classList.remove("active"));
      tab.classList.add("active");

      // Update active content
      const tabId = tab.dataset.tab;
      document.querySelectorAll(".tab-content").forEach((content) => {
        content.classList.remove("active");
      });
      document.getElementById(`${tabId}-tab`).classList.add("active");
    });
  });
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
  elements.scrapeForm.addEventListener("submit", handleScrapeSubmit);

  // Search
  document.getElementById("search-btn").addEventListener("click", handleSearch);
  elements.searchQuery.addEventListener("keypress", (e) => {
    if (e.key === "Enter") handleSearch();
  });

  // Theme toggle
  const themeToggle = document.getElementById("theme-toggle");
  if (themeToggle) {
    themeToggle.addEventListener("click", toggleTheme);
  }
}

// API Functions
async function fetchAPI(endpoint, options = {}) {
  try {
    const response = await fetch(`${API_BASE}${endpoint}`, {
      headers: {
        "Content-Type": "application/json",
      },
      ...options,
    });
    return await response.json();
  } catch (error) {
    console.error("API Error:", error);
    return { success: false, error: error.message };
  }
}

// Load Libraries
async function loadLibraries() {
  elements.librariesList.innerHTML = '<p class="loading">Loading...</p>';

  const result = await fetchAPI("/libraries");

  if (!result.success) {
    elements.librariesList.innerHTML = `<p class="empty-state">Error: ${result.error}</p>`;
    return;
  }

  if (!result.data || result.data.length === 0) {
    elements.librariesList.innerHTML =
      '<p class="empty-state">No libraries indexed yet. Add one to get started!</p>';
    updateSearchLibrarySelect([]);
    return;
  }

  elements.librariesList.innerHTML = result.data
    .map((lib) => renderLibraryCard(lib))
    .join("");
  updateSearchLibrarySelect(result.data);
}

// Render Library Card
function renderLibraryCard(lib) {
  const versionsHtml =
    lib.versions.length > 0
      ? lib.versions
          .map(
            (v) => `
            <div class="version-item">
                <span class="status ${v.status}"></span>
                <span>${v.name || "latest"}</span>
                <span class="card-meta">${v.page_count} pages</span>
                <button class="btn btn-danger btn-sm" onclick="deleteVersion('${lib.name}', '${v.name}')">Delete</button>
            </div>
        `,
          )
          .join("")
      : '<span class="card-meta">No versions</span>';

  return `
        <div class="card">
            <div class="card-header">
                <span class="card-title">${lib.name}</span>
                <span class="card-meta">${lib.versions.length} version(s)</span>
            </div>
            <div class="card-body">
                <div class="versions-list">${versionsHtml}</div>
            </div>
        </div>
    `;
}

// Update Search Library Select
function updateSearchLibrarySelect(libraries) {
  elements.searchLibrary.innerHTML =
    '<option value="">Select library...</option>';
  libraries.forEach((lib) => {
    const option = document.createElement("option");
    option.value = lib.name;
    option.textContent = lib.name;
    elements.searchLibrary.appendChild(option);
  });
}

// Load Jobs
async function loadJobs() {
  const result = await fetchAPI("/jobs");

  if (!result.success) {
    elements.jobsList.innerHTML = `<p class="empty-state">Error: ${result.error}</p>`;
    return;
  }

  if (!result.data || result.data.length === 0) {
    elements.jobsList.innerHTML = '<p class="empty-state">No jobs running</p>';
    return;
  }

  elements.jobsList.innerHTML = result.data
    .map((job) => renderJobCard(job))
    .join("");
}

// Render Job Card
function renderJobCard(job) {
  const progress = job.progress;
  const progressPercent = progress
    ? Math.round((progress.pages_scraped / progress.total_pages) * 100)
    : 0;
  const progressHtml =
    job.status === "running" && progress
      ? `
            <div class="progress-bar">
                <div class="progress-fill" style="width: ${progressPercent}%"></div>
            </div>
            <p class="card-meta">${progress.pages_scraped}/${progress.total_pages} pages (${progressPercent}%)</p>
            <p class="card-meta">Current: ${progress.current_url || "Starting..."}</p>
        `
      : "";

  const cancelBtn =
    job.status === "running" || job.status === "queued"
      ? `<button class="btn btn-danger btn-sm" onclick="cancelJob('${job.id}')">Cancel</button>`
      : "";

  return `
        <div class="card" data-job-id="${job.id}">
            <div class="card-header">
                <span class="card-title">${job.library}${job.version ? "@" + job.version : ""}</span>
                <span class="job-status ${job.status}">${job.status}</span>
            </div>
            <div class="card-body">
                <p class="card-meta">${job.source_url || "No URL"}</p>
                ${job.error ? `<p class="card-meta" style="color: var(--error)">Error: ${job.error}</p>` : ""}
                ${progressHtml}
                ${cancelBtn}
            </div>
        </div>
    `;
}

// Handle Scrape Submit
async function handleScrapeSubmit(e) {
  e.preventDefault();

  const formData = new FormData(e.target);
  const data = {
    url: formData.get("url"),
    library: formData.get("library"),
    version: formData.get("version") || "",
    max_pages: parseInt(formData.get("max_pages")) || 1000,
    max_depth: parseInt(formData.get("max_depth")) || 3,
  };

  const result = await fetchAPI("/jobs", {
    method: "POST",
    body: JSON.stringify(data),
  });

  if (result.success) {
    showToast("Scraping job started successfully!", "success");
    e.target.reset();

    // Switch to jobs tab
    document.querySelector('[data-tab="jobs"]').click();
    loadJobs();
  } else {
    showToast(`Error: ${result.error}`, "error");
  }
}

// Handle Search
async function handleSearch() {
  const library = elements.searchLibrary.value;
  const query = elements.searchQuery.value.trim();

  if (!library) {
    showToast("Please select a library", "error");
    return;
  }

  if (!query) {
    showToast("Please enter a search query", "error");
    return;
  }

  elements.searchResults.innerHTML = '<p class="loading">Searching...</p>';

  const result = await fetchAPI(
    `/libraries/${encodeURIComponent(library)}/search?q=${encodeURIComponent(query)}&limit=5`,
  );

  if (!result.success) {
    elements.searchResults.innerHTML = `<p class="empty-state">Error: ${result.error}</p>`;
    return;
  }

  if (!result.data || result.data.length === 0) {
    elements.searchResults.innerHTML =
      '<p class="empty-state">No results found</p>';
    return;
  }

  elements.searchResults.innerHTML = result.data
    .map((r) => renderSearchResult(r))
    .join("");
}

// Render Search Result
function renderSearchResult(result) {
  // Truncate content
  const maxLen = 300;
  const content =
    result.content.length > maxLen
      ? result.content.substring(0, maxLen) + "..."
      : result.content;

  return `
        <div class="search-result">
            <div class="search-result-header">
                <a href="${result.url}" target="_blank" class="search-result-title">${result.title || result.url}</a>
                <span class="search-result-score">${(result.score * 100).toFixed(1)}%</span>
            </div>
            <p class="search-result-url">${result.url}</p>
            <p class="search-result-content">${escapeHtml(content)}</p>
        </div>
    `;
}

// Delete Version
async function deleteVersion(library, version) {
  if (!confirm(`Delete ${library}@${version}? This cannot be undone.`)) {
    return;
  }

  const result = await fetchAPI(
    `/libraries/${encodeURIComponent(library)}/versions/${encodeURIComponent(version)}`,
    {
      method: "DELETE",
    },
  );

  if (result.success) {
    showToast("Version deleted", "success");
    loadLibraries();
  } else {
    showToast(`Error: ${result.error}`, "error");
  }
}

// Cancel Job
async function cancelJob(jobId) {
  const result = await fetchAPI(`/jobs/${jobId}/cancel`, {
    method: "POST",
  });

  if (result.success) {
    showToast("Job cancelled", "info");
    loadJobs();
  } else {
    showToast(`Error: ${result.error}`, "error");
  }
}

// Clear Jobs
async function clearJobs() {
  const result = await fetchAPI("/jobs/clear", {
    method: "POST",
  });

  if (result.success) {
    showToast(`Cleared ${result.data} completed job(s)`, "info");
    loadJobs();
  } else {
    showToast(`Error: ${result.error}`, "error");
  }
}

// SSE Connection
function connectSSE() {
  if (eventSource) {
    eventSource.close();
  }

  eventSource = new EventSource(`${API_BASE}/events`);

  eventSource.onopen = () => {
    console.log("SSE connected");
  };

  eventSource.onerror = (e) => {
    console.error("SSE error:", e);
    // Reconnect after 5 seconds
    setTimeout(connectSSE, 5000);
  };

  eventSource.onmessage = (e) => {
    try {
      const event = JSON.parse(e.data);
      handleSSEEvent(event);
    } catch (err) {
      console.error("Failed to parse SSE event:", err);
    }
  };
}

// Handle SSE Events
function handleSSEEvent(event) {
  console.log("SSE event:", event.type);

  switch (event.type) {
    case "JOB_STATUS_CHANGE":
      loadJobs();
      break;
    case "JOB_PROGRESS":
      updateJobProgress(event.payload.job, event.payload.progress);
      break;
    case "LIBRARY_CHANGE":
      loadLibraries();
      break;
    case "JOB_LIST_CHANGE":
      loadJobs();
      break;
  }
}

// Update Job Progress (without full reload)
function updateJobProgress(job, progress) {
  const card = document.querySelector(`[data-job-id="${job.id}"]`);
  if (!card) {
    loadJobs();
    return;
  }

  // Update status
  const statusEl = card.querySelector(".job-status");
  if (statusEl) {
    statusEl.className = `job-status ${job.status}`;
    statusEl.textContent = job.status;
  }

  // Update progress bar
  if (progress) {
    const percent = Math.round(
      (progress.pages_scraped / progress.total_pages) * 100,
    );
    const progressFill = card.querySelector(".progress-fill");
    if (progressFill) {
      progressFill.style.width = `${percent}%`;
    }

    // Update progress text
    let progressText = card.querySelector(".progress-text");
    if (!progressText) {
      progressText = document.createElement("p");
      progressText.className = "card-meta progress-text";
      card.querySelector(".card-body").appendChild(progressText);
    }
    progressText.textContent = `${progress.pages_scraped}/${progress.total_pages} pages (${percent}%)`;
  }
}

// Toast Notification
function showToast(message, type = "info") {
  elements.toast.textContent = message;
  elements.toast.className = `toast ${type}`;

  // Show
  setTimeout(() => {
    elements.toast.classList.remove("hidden");
  }, 10);

  // Hide after 3 seconds
  setTimeout(() => {
    elements.toast.classList.add("hidden");
  }, 3000);
}

// Escape HTML
function escapeHtml(text) {
  const div = document.createElement("div");
  div.textContent = text;
  return div.innerHTML;
}

// Make functions available globally for inline handlers
window.deleteVersion = deleteVersion;
window.cancelJob = cancelJob;
