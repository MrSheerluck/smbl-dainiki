(function () {
  var root = document.documentElement;
  var saved = null;

  try {
    saved = localStorage.getItem("dainiki-theme");
  } catch (_) {}

  var systemDark = window.matchMedia && window.matchMedia("(prefers-color-scheme: dark)").matches;
  var theme = saved === "dark" || saved === "light" ? saved : systemDark ? "dark" : "light";
  root.dataset.theme = theme;

  function toggleTheme() {
    var next = root.dataset.theme === "dark" ? "light" : "dark";
    root.dataset.theme = next;
    try {
      localStorage.setItem("dainiki-theme", next);
    } catch (_) {}
  }

  document.addEventListener("DOMContentLoaded", function () {
    var buttons = document.querySelectorAll("[data-theme-toggle]");
    for (var i = 0; i < buttons.length; i += 1) {
      buttons[i].addEventListener("click", toggleTheme);
    }
  });
})();
