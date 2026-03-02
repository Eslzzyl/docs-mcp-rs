// Docs MCP Web UI - Internationalization

// Initialize i18n
async function initI18n() {
  // 1. Check saved language preference
  const savedLang = localStorage.getItem("language");

  // 2. If no saved preference, detect browser language
  if (savedLang) {
    window.currentLang = savedLang;
  } else {
    window.currentLang = detectLanguage();
  }

  // 3. Load translations and apply
  await loadTranslations(window.currentLang);
  updatePageLanguage();
}

function detectLanguage() {
  const browserLang = navigator.language || navigator.userLanguage;
  // If Chinese (zh), use Simplified Chinese
  if (browserLang.startsWith("zh")) {
    return "zh-CN";
  }
  return "en";
}

async function loadTranslations(lang) {
  try {
    const response = await fetch(`/lang/${lang}.json`);
    if (!response.ok) {
      throw new Error(`Failed to load translations for ${lang}`);
    }
    window.translations = await response.json();
  } catch (error) {
    console.error("Failed to load translations:", error);
    window.translations = {};
  }
}

function t(key, params = {}) {
  const keys = key.split(".");
  let value = window.translations;

  for (const k of keys) {
    if (value && typeof value === "object" && k in value) {
      value = value[k];
    } else {
      return key; // Return key if translation not found
    }
  }

  // Replace parameters
  if (typeof value === "string") {
    return value.replace(/\{(\w+)\}/g, (match, paramKey) => {
      return params[paramKey] !== undefined ? params[paramKey] : match;
    });
  }

  return key;
}

function updatePageLanguage() {
  // Update html lang attribute
  document.documentElement.lang = window.currentLang;

  // Update all elements with data-i18n attribute
  document.querySelectorAll("[data-i18n]").forEach((el) => {
    const key = el.getAttribute("data-i18n");
    const translated = t(key);
    if (translated !== key) {
      el.textContent = translated;
    }
  });

  // Update placeholders
  document.querySelectorAll("[data-i18n-placeholder]").forEach((el) => {
    const key = el.getAttribute("data-i18n-placeholder");
    const translated = t(key);
    if (translated !== key) {
      el.placeholder = translated;
    }
  });

  // Update language selector
  const langSelect = document.getElementById("language-select");
  if (langSelect) {
    langSelect.value = window.currentLang;
  }
}

async function setLanguage(lang) {
  window.currentLang = lang;
  localStorage.setItem("language", lang);
  await loadTranslations(lang);
  updatePageLanguage();

  // Reload dynamic content to reflect language change
  loadLibraries();
  loadJobs();
}

// Export functions
window.initI18n = initI18n;
window.detectLanguage = detectLanguage;
window.loadTranslations = loadTranslations;
window.t = t;
window.updatePageLanguage = updatePageLanguage;
window.setLanguage = setLanguage;
