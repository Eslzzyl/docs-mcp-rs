// Docs MCP Web UI - Job Management

// Load Jobs
async function loadJobs() {
  const result = await fetchAPI("/jobs");

  if (!result.success) {
    window.elements.jobsList.innerHTML = `<p class="empty-state">Error: ${result.error}</p>`;
    return;
  }

  if (!result.data || result.data.length === 0) {
    window.elements.jobsList.innerHTML = `<p class="empty-state">${t("jobs.empty")}</p>`;
    return;
  }

  window.elements.jobsList.innerHTML = result.data
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
        <p class="card-meta progress-text">${t("progress.initializing")}</p>
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
        <p class="card-meta progress-text">${t("progress.discovering")} (${progress.total_discovered} ${t("progress.discoveredInfo")}, ${progress.queue_length} ${t("progress.inQueue")})</p>
        <p class="card-meta current-url">${t("progress.current")}: ${progress.current_url || t("progress.scanning")}</p>
      </div>
    `;
  } else {
    // During scraping phase, show actual progress
    const total = progress.total_pages || 1;
    const scraped = progress.pages_scraped || 0;
    progressPercent = Math.round((scraped / total) * 100);
    progressText = `${scraped}/${total} ${t("progress.pagesProgress")} (${progressPercent}%)`;

    return `
      <div class="progress-container">
        <div class="progress-bar">
          <div class="progress-fill" style="width: ${progressPercent}%"></div>
        </div>
        <p class="card-meta progress-text">📄 ${progressText}</p>
        <p class="card-meta current-url">${t("progress.current")}: ${progress.current_url || t("progress.processing")}</p>
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
      ? `<button class="btn btn-danger btn-sm" onclick="cancelJob('${job.id}')">${t("jobs.cancel")}</button>`
      : "";

  return `
        <div class="card" data-job-id="${job.id}">
            <div class="card-header">
                <span class="card-title">${job.library}${job.version ? "@" + job.version : ""}</span>
                <span class="job-status ${job.status}">${t(`status.${job.status}`)}</span>
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

// Cancel Job
async function cancelJob(jobId) {
  const result = await fetchAPI(`/jobs/${jobId}/cancel`, {
    method: "POST",
  });

  if (result.success) {
    showToast(t("toast.jobCancelled"), "info");
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
    showToast(t("toast.jobsCleared", { count: result.data }), "info");
    loadJobs();
  } else {
    showToast(`Error: ${result.error}`, "error");
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
    statusEl.textContent = t(`status.${job.status}`);
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
      <p class="card-meta progress-text">${t("progress.discovering")} (${progress.total_discovered} ${t("progress.discoveredInfo")}, ${progress.queue_length} ${t("progress.inQueue")})</p>
      <p class="card-meta current-url">${t("progress.current")}: ${progress.current_url || t("progress.scanning")}</p>
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
      <p class="card-meta progress-text">📄 ${scraped}/${total} ${t("progress.pagesProgress")} (${percent}%)</p>
      <p class="card-meta current-url">${t("progress.current")}: ${progress.current_url || t("progress.processing")}</p>
    `;
  }
}

// Export functions
window.loadJobs = loadJobs;
window.renderProgressHtml = renderProgressHtml;
window.renderJobCard = renderJobCard;
window.cancelJob = cancelJob;
window.clearJobs = clearJobs;
window.updateJobProgress = updateJobProgress;
