// Docs MCP Web UI - Theme Management

// Initialize theme
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

// Export functions
window.initTheme = initTheme;
window.toggleTheme = toggleTheme;
