// Docs MCP Web UI - Server-Sent Events

// SSE Connection
function connectSSE() {
  if (window.eventSource) {
    window.eventSource.close();
  }

  window.eventSource = new EventSource(`${window.API_BASE}/events`);

  window.eventSource.onopen = () => {
    console.log("SSE connected");
  };

  window.eventSource.onerror = (e) => {
    console.error("SSE error:", e);
    // Reconnect after 5 seconds
    setTimeout(connectSSE, 5000);
  };

  window.eventSource.onmessage = (e) => {
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

// Export functions
window.connectSSE = connectSSE;
window.handleSSEEvent = handleSSEEvent;
