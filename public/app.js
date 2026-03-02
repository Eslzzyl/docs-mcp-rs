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
    const fetchOptions = {
      ...options,
    };

    // Only set Content-Type if there's a body and it's not FormData
    if (options.body && !(options.body instanceof FormData)) {
      fetchOptions.headers = {
        "Content-Type": "application/json",
        ...options.headers,
      };
    }

    const url = `${API_BASE}${endpoint}`;
    console.log(`[API] ${options.method || 'GET'} ${url}`, fetchOptions);

    const response = await fetch(url, fetchOptions);

    console.log(`[API] Response status: ${response.status} ${response.statusText}`);
    console.log(`[API] Response headers:`, [...response.headers.entries()]);

    // Get response text first to check if it's empty
    const responseText = await response.text();
    console.log(`[API] Response body:`, responseText);

    // Try to parse as JSON if there's content
    if (responseText.trim()) {
      try {
        const json = JSON.parse(responseText);
        console.log(`[API] Parsed JSON:`, json);
        return json;
      } catch (parseError) {
        console.error(`[API] JSON parse error:`, parseError);
        return { success: false, error: `Invalid JSON: ${parseError.message}` };
      }
    }

    // If no content, return success based on status
    if (response.ok) {
      return { success: true };
    }

    return { success: false, error: `HTTP ${response.status}: ${response.statusText}` };
  } catch (error) {
    console.error("[API] Error:", error);
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
            (v) => {
              const versionName = v.name || "";
              const displayName = versionName || "latest";
              return `
            <div class="version-item">
                <span class="status ${v.status}"></span>
                <span>${displayName}</span>
                <span class="card-meta">${v.page_count} pages</span>
                <button class="btn btn-danger btn-sm" onclick="deleteVersion('${lib.name}', '${versionName}')">Delete</button>
            </div>
        `;
            },
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

// Render progress HTML based on job status and phase
function renderProgressHtml(status, progress) {
  // Show progress for running and queued jobs
  if (status !== "running" && status !== "queued") {
    return "";
  }

  // If no progress yet, show initial state
  if (!progress) {
    return `
      <div class="progress-container">
        <div class="progress-bar indeterminate">
          <div class="progress-fill"></div>
        </div>
        <p class="card-meta progress-text">Initializing...</p>
      </div>
    `;
  }

  const phase = progress.phase || "discovering";
  const isDiscovering = phase === "discovering" || progress.is_discovering;
  
  // Calculate progress percentage
  let progressPercent = 0;
  let progressText = "";
  
  if (isDiscovering) {
    // During discovery phase, show indeterminate progress
    return `
      <div class="progress-container">
        <div class="progress-bar indeterminate">
          <div class="progress-fill"></div>
        </div>
        <p class="card-meta progress-text">🔍 Discovering pages... (${progress.total_discovered} found, ${progress.queue_length} in queue)</p>
        <p class="card-meta current-url">Current: ${progress.current_url || "Scanning..."}</p>
      </div>
    `;
  } else {
    // During scraping phase, show actual progress
    const total = progress.total_pages || 1;
    const scraped = progress.pages_scraped || 0;
    progressPercent = Math.round((scraped / total) * 100);
    progressText = `${scraped}/${total} pages (${progressPercent}%)`;
    
    return `
      <div class="progress-container">
        <div class="progress-bar">
          <div class="progress-fill" style="width: ${progressPercent}%"></div>
        </div>
        <p class="card-meta progress-text">📄 ${progressText}</p>
        <p class="card-meta current-url">Current: ${progress.current_url || "Processing..."}</p>
      </div>
    `;
  }
}

// Render Job Card
function renderJobCard(job) {
  const progress = job.progress;
  const progressHtml = renderProgressHtml(job.status, progress);

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
  console.log(`[deleteVersion] Starting deletion for ${library}@${version}`);

  const displayVersion = version || "latest";
  if (!confirm(`Delete ${library}@${displayVersion}? This cannot be undone.`)) {
    console.log(`[deleteVersion] Cancelled by user`);
    return;
  }

  const encodedLibrary = encodeURIComponent(library);
  // Use special marker for empty version to avoid routing issues
  const versionParam = version || "_default_";
  const encodedVersion = encodeURIComponent(versionParam);
  const endpoint = `/libraries/${encodedLibrary}/versions/${encodedVersion}`;

  console.log(`[deleteVersion] Library: "${library}" -> encoded: "${encodedLibrary}"`);
  console.log(`[deleteVersion] Version: "${version}" -> param: "${versionParam}" -> encoded: "${encodedVersion}"`);
  console.log(`[deleteVersion] Full endpoint: ${endpoint}`);

  const result = await fetchAPI(endpoint, {
    method: "DELETE",
  });

  console.log(`[deleteVersion] Result:`, result);

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
      console.log("[SSE] Raw data:", e.data);
      const event = JSON.parse(e.data);
      console.log("[SSE] Parsed event:", event);
      handleSSEEvent(event);
    } catch (err) {
      console.error("Failed to parse SSE event:", err);
      console.error("[SSE] Raw data that failed:", e.data);
    }
  };
}

// Handle SSE Events
function handleSSEEvent(event) {
  console.log("SSE event:", event.type);

  // Handle nested payload structure
  const payload = event.payload?.payload || event.payload;

  switch (event.type) {
    case "JOB_STATUS_CHANGE":
      loadJobs();
      break;
    case "JOB_PROGRESS":
      if (payload?.job && payload?.progress) {
        updateJobProgress(payload.job, payload.progress);
      } else {
        console.warn("JOB_PROGRESS event missing job or progress:", event);
      }
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

  // Skip if no progress
  if (!progress) {
    return;
  }

  const phase = progress.phase || "discovering";
  const isDiscovering = phase === "discovering" || progress.is_discovering;

  // Find or create progress container
  let progressContainer = card.querySelector(".progress-container");
  if (!progressContainer) {
    progressContainer = document.createElement("div");
    progressContainer.className = "progress-container";
    card.querySelector(".card-body").appendChild(progressContainer);
  }

  if (isDiscovering) {
    // Discovery phase - show indeterminate progress
    progressContainer.innerHTML = `
      <div class="progress-bar indeterminate">
        <div class="progress-fill"></div>
      </div>
      <p class="card-meta progress-text">🔍 Discovering pages... (${progress.total_discovered} found, ${progress.queue_length} in queue)</p>
      <p class="card-meta current-url">Current: ${progress.current_url || "Scanning..."}</p>
    `;
  } else {
    // Scraping phase - show actual progress
    const total = progress.total_pages || 1;
    const scraped = progress.pages_scraped || 0;
    const percent = Math.round((scraped / total) * 100);

    progressContainer.innerHTML = `
      <div class="progress-bar">
        <div class="progress-fill" style="width: ${percent}%"></div>
      </div>
      <p class="card-meta progress-text">📄 ${scraped}/${total} pages (${percent}%)</p>
      <p class="card-meta current-url">Current: ${progress.current_url || "Processing..."}</p>
    `;
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
