// Docs MCP Web UI - API Functions

// API helper function
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

    const url = `${window.API_BASE}${endpoint}`;
    console.log(`[API] ${options.method || "GET"} ${url}`, fetchOptions);

    const response = await fetch(url, fetchOptions);

    console.log(
      `[API] Response status: ${response.status} ${response.statusText}`,
    );
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

    return {
      success: false,
      error: `HTTP ${response.status}: ${response.statusText}`,
    };
  } catch (error) {
    console.error("[API] Error:", error);
    return { success: false, error: error.message };
  }
}

// Export functions
window.fetchAPI = fetchAPI;
