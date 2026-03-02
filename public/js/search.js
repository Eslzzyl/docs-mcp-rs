// Docs MCP Web UI - Search Functionality

// Handle Search
async function handleSearch() {
  const library = window.elements.searchLibrary.value;
  const query = window.elements.searchQuery.value.trim();

  if (!library) {
    showToast(t("toast.selectLibrary"), "error");
    return;
  }

  if (!query) {
    showToast(t("toast.enterQuery"), "error");
    return;
  }

  window.elements.searchResults.innerHTML = `<p class="loading">${t("search.loading")}</p>`;

  const result = await fetchAPI(
    `/libraries/${encodeURIComponent(library)}/search?q=${encodeURIComponent(query)}&limit=5`,
  );

  if (!result.success) {
    window.elements.searchResults.innerHTML = `<p class="empty-state">Error: ${result.error}</p>`;
    return;
  }

  if (!result.data || result.data.length === 0) {
    window.elements.searchResults.innerHTML = `<p class="empty-state">${t("search.noResults")}</p>`;
    return;
  }

  window.elements.searchResults.innerHTML = result.data
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

// Export functions
window.handleSearch = handleSearch;
window.renderSearchResult = renderSearchResult;
