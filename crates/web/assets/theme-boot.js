// Applies the saved theme before the first paint. Reads the settings the app writes (localStorage "nolabs.settings").
// The colours are filled in from brand/dist/tokens/tokens.json at build time (tools/postbuild.py).
(function () {
  var BG = { ink: "__BG_INK__", paper: "__BG_PAPER__", night: "__BG_NIGHT__" };
  try {
    var s = JSON.parse(localStorage.getItem("nolabs.settings") || "{}");
    var theme = s.theme === "paper" || s.theme === "night" || s.theme === "system" ? s.theme : "ink";
    if (theme === "system") {
      theme = window.matchMedia("(prefers-color-scheme: light)").matches ? "paper" : "ink";
    }
    document.documentElement.setAttribute("data-theme", theme);
    var meta = document.querySelector('meta[name="theme-color"]');
    if (meta) meta.setAttribute("content", BG[theme]);
    var lang = localStorage.getItem("nolabs.lang");
    if (lang === "ar") document.documentElement.setAttribute("dir", "rtl");
  } catch (e) {
    /* storage unavailable: keep Ink */
  }
})();
