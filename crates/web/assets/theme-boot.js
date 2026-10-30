// Applies the saved theme before the first paint. Reads the settings the app writes (localStorage "nolabs.settings").
(function () {
  try {
    var s = JSON.parse(localStorage.getItem("nolabs.settings") || "{}");
    var theme = s.theme === "paper" || s.theme === "system" ? s.theme : "ink";
    document.documentElement.setAttribute("data-theme", theme);
    var light = theme === "paper" || (theme === "system" && window.matchMedia("(prefers-color-scheme: light)").matches);
    var meta = document.querySelector('meta[name="theme-color"]');
    if (meta) meta.setAttribute("content", light ? "#F6F2EA" : "#0B0B0F");
  } catch (e) {
    /* storage unavailable: keep Ink */
  }
})();
