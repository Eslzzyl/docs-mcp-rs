// Docs MCP Web UI - Library Management

// Load Libraries
async function loadLibraries() {
  window.elements.librariesList.innerHTML = `<p class="loading">${t("libraries.loading")}</p>`;

  const result = await fetchAPI("/libraries");

  if (!result.success) {
    window.elements.librariesList.innerHTML = `<p class="empty-state">Error: ${result.error}</p>`;
    return;
  }

  if (!result.data || result.data.length === 0) {
    window.elements.librariesList.innerHTML = `<p class="empty-state">${t("libraries.empty")}</p>`;
    updateSearchLibrarySelect([]);
    return;
  }

  window.elements.librariesList.innerHTML = result.data
    .map((lib) => renderLibraryCard(lib))
    .join("");
  updateSearchLibrarySelect(result.data);
}

// Render Library Card
function renderLibraryCard(lib) {
  const versionsHtml =
    lib.versions.length > 0
      ? lib.versions
          .map((v) => {
            const versionName = v.name || "";
            const displayName = versionName || "latest";
            return `
            <div class="version-item">
                <span class="status ${v.status}"></span>
                <span>${displayName}</span>
                <span class="card-meta">${v.page_count} ${t("libraries.pages")}</span>
                <button class="btn btn-danger btn-sm" onclick="deleteVersion('${lib.name}', '${versionName}')">${t("libraries.deleteVersion")}</button>
            </div>
        `;
          })
          .join("")
      : `<span class="card-meta">${t("libraries.noVersions")}</span>`;

  return `
        <div class="card">
            <div class="card-header">
                <span class="card-title">${lib.name}</span>
                <div class="card-actions">
                    <span class="card-meta">${lib.versions.length} ${t("libraries.versionCount")}</span>
                    <button class="btn btn-danger btn-sm" onclick="deleteLibrary('${lib.name}')">${t("libraries.deleteLibrary")}</button>
                </div>
            </div>
            <div class="card-body">
                <div class="versions-list">${versionsHtml}</div>
            </div>
        </div>
    `;
}

// Update Search Library Select
function updateSearchLibrarySelect(libraries) {
  window.elements.searchLibrary.innerHTML = "";

  if (libraries.length === 0) {
    const option = document.createElement("option");
    option.value = "";
    option.textContent = t("search.noLibraries");
    window.elements.searchLibrary.appendChild(option);
    return;
  }

  libraries.forEach((lib) => {
    const option = document.createElement("option");
    option.value = lib.name;
    option.textContent = lib.name;
    window.elements.searchLibrary.appendChild(option);
  });

  // Auto-select the first library
  window.elements.searchLibrary.selectedIndex = 0;
}

// Delete Version
async function deleteVersion(library, version) {
  console.log(`[deleteVersion] Starting deletion for ${library}@${version}`);

  const displayVersion = version || "latest";
  const confirmMessage = t("confirm.deleteVersion", {
    library,
    version: displayVersion,
  });
  if (!confirm(confirmMessage)) {
    console.log(`[deleteVersion] Cancelled by user`);
    return;
  }

  const encodedLibrary = encodeURIComponent(library);
  // Use special marker for empty version to avoid routing issues
  const versionParam = version || "_default_";
  const encodedVersion = encodeURIComponent(versionParam);
  const endpoint = `/libraries/${encodedLibrary}/versions/${encodedVersion}`;

  console.log(
    `[deleteVersion] Library: "${library}" -> encoded: "${encodedLibrary}"`,
  );
  console.log(
    `[deleteVersion] Version: "${version}" -> param: "${versionParam}" -> encoded: "${encodedVersion}"`,
  );
  console.log(`[deleteVersion] Full endpoint: ${endpoint}`);

  const result = await fetchAPI(endpoint, {
    method: "DELETE",
  });

  console.log(`[deleteVersion] Result:`, result);

  if (result.success) {
    showToast(t("toast.versionDeleted"), "success");
    loadLibraries();
  } else {
    showToast(`Error: ${result.error}`, "error");
  }
}

// Delete Library
async function deleteLibrary(library) {
  console.log(`[deleteLibrary] Starting deletion for ${library}`);

  const confirmMessage = t("confirm.deleteLibrary", { library });
  if (!confirm(confirmMessage)) {
    console.log(`[deleteLibrary] Cancelled by user`);
    return;
  }

  const encodedLibrary = encodeURIComponent(library);
  const endpoint = `/libraries/${encodedLibrary}`;

  console.log(
    `[deleteLibrary] Library: "${library}" -> encoded: "${encodedLibrary}"`,
  );
  console.log(`[deleteLibrary] Full endpoint: ${endpoint}`);

  const result = await fetchAPI(endpoint, {
    method: "DELETE",
  });

  console.log(`[deleteLibrary] Result:`, result);

  if (result.success) {
    showToast(t("toast.libraryDeleted"), "success");
    loadLibraries();
  } else {
    showToast(`Error: ${result.error}`, "error");
  }
}

// Export functions
window.loadLibraries = loadLibraries;
window.renderLibraryCard = renderLibraryCard;
window.updateSearchLibrarySelect = updateSearchLibrarySelect;
window.deleteVersion = deleteVersion;
window.deleteLibrary = deleteLibrary;
