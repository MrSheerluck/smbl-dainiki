(function () {
  var root = document.documentElement;
  var saved = null;
  var reduceMotion = window.matchMedia
    ? window.matchMedia("(prefers-reduced-motion: reduce)")
    : null;

  try {
    saved = localStorage.getItem("dainiki-theme");
  } catch (_) {}

  var systemDark = window.matchMedia && window.matchMedia("(prefers-color-scheme: dark)").matches;
  var theme = saved === "dark" || saved === "light" ? saved : systemDark ? "dark" : "light";

  function syncThemeColor(next) {
    var meta = document.querySelector('meta[name="theme-color"]');
    if (meta) meta.setAttribute("content", next === "dark" ? "#151312" : "#fef8f6");
  }

  function applyTheme(next) {
    root.dataset.theme = next;
    syncThemeColor(next);

    try {
      localStorage.setItem("dainiki-theme", next);
    } catch (_) {}

    var buttons = document.querySelectorAll("[data-theme-toggle]");
    for (var i = 0; i < buttons.length; i += 1) {
      buttons[i].setAttribute("aria-pressed", String(next === "dark"));
    }
  }

  function toggleTheme() {
    var next = root.dataset.theme === "dark" ? "light" : "dark";

    if (!document.startViewTransition || (reduceMotion && reduceMotion.matches)) {
      applyTheme(next);
      return;
    }

    document.startViewTransition(function () {
      applyTheme(next);
    });
  }

  root.dataset.theme = theme;
  syncThemeColor(theme);

  document.addEventListener("DOMContentLoaded", function () {
    var buttons = document.querySelectorAll("[data-theme-toggle]");
    for (var i = 0; i < buttons.length; i += 1) {
      buttons[i].setAttribute("aria-pressed", String(theme === "dark"));
      buttons[i].addEventListener("click", toggleTheme);
    }
  });
})();
