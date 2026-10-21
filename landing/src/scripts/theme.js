(function () {
  var root = document.documentElement;
  var saved = null;
  var reduceMotion = window.matchMedia
    ? window.matchMedia("(prefers-reduced-motion: reduce)")
    : null;
  var resizeTimer = null;
  var SVG_NS = "http://www.w3.org/2000/svg";

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

  function buildPixelGrid() {
    if (!document.body) return;

    var svg = document.getElementById("theme-pixel-grid-svg");
    if (!svg) {
      svg = document.createElementNS(SVG_NS, "svg");
      svg.setAttribute("id", "theme-pixel-grid-svg");
      svg.setAttribute("class", "theme-pixel-grid");
      svg.setAttribute("aria-hidden", "true");
      svg.setAttribute("focusable", "false");
      document.body.appendChild(svg);
    }

    var clip = document.getElementById("theme-pixel-grid");
    if (!clip) {
      clip = document.createElementNS(SVG_NS, "clipPath");
      clip.setAttribute("id", "theme-pixel-grid");
      clip.setAttribute("clipPathUnits", "objectBoundingBox");
      svg.appendChild(clip);
    }

    while (clip.firstChild) clip.removeChild(clip.firstChild);

    var cell = 56;
    var cols = Math.ceil(window.innerWidth / cell);
    var rows = Math.ceil(window.innerHeight / cell);
    var maxDelay = 220;

    for (var row = 0; row < rows; row += 1) {
      for (var col = 0; col < cols; col += 1) {
        var rect = document.createElementNS(SVG_NS, "rect");
        rect.setAttribute("x", String(col / cols));
        rect.setAttribute("y", String(row / rows));
        rect.setAttribute("width", String(1 / cols));
        rect.setAttribute("height", String(1 / rows));
        rect.style.setProperty(
          "--pixel-delay",
          (((cols - 1 - col) + row) / (cols + rows)) * maxDelay + "ms",
        );
        clip.appendChild(rect);
      }
    }
  }

  function toggleTheme() {
    var next = root.dataset.theme === "dark" ? "light" : "dark";

    if (
      !document.startViewTransition ||
      (reduceMotion && reduceMotion.matches) ||
      root.classList.contains("theme-pixel-wiping")
    ) {
      applyTheme(next);
      return;
    }

    document.startViewTransition(function () {
      applyTheme(next);
      root.classList.add("theme-pixel-wiping");
    });

    window.setTimeout(function () {
      root.classList.remove("theme-pixel-wiping");
    }, 720);
  }

  root.dataset.theme = theme;
  syncThemeColor(theme);

  document.addEventListener("DOMContentLoaded", function () {
    buildPixelGrid();

    window.addEventListener("resize", function () {
      window.clearTimeout(resizeTimer);
      resizeTimer = window.setTimeout(buildPixelGrid, 180);
    });

    var buttons = document.querySelectorAll("[data-theme-toggle]");
    for (var i = 0; i < buttons.length; i += 1) {
      buttons[i].setAttribute("aria-pressed", String(theme === "dark"));
      buttons[i].addEventListener("click", toggleTheme);
    }
  });
})();
