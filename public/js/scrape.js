// Docs MCP Web UI - Scrape Form Handling

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
    scrape_mode: formData.get("scrape_mode") || "browser",
  };

  const result = await fetchAPI("/jobs", {
    method: "POST",
    body: JSON.stringify(data),
  });

  if (result.success) {
    showToast(t("toast.jobStarted"), "success");
    e.target.reset();

    // Scroll to jobs section and refresh
    document
      .getElementById("jobs-section")
      .scrollIntoView({ behavior: "smooth" });
    loadJobs();
  } else {
    showToast(`Error: ${result.error}`, "error");
  }
}

// Export functions
window.handleScrapeSubmit = handleScrapeSubmit;
