// Applied immediately (before paint) so there's no flash of the wrong theme.
(function () {
  var KEY = "vm-check-theme";

  function apply(pref) {
    if (pref === "light" || pref === "dark") {
      document.documentElement.setAttribute("data-theme", pref);
    } else {
      document.documentElement.removeAttribute("data-theme");
    }
  }

  function current() {
    var stored = localStorage.getItem(KEY);
    return stored === "light" || stored === "dark" ? stored : "system";
  }

  apply(current());

  window.__vmCheckTheme = { current: current, apply: apply, key: KEY };
})();

document.addEventListener("DOMContentLoaded", function () {
  // ---------- manual theme toggle ----------
  var themeApi = window.__vmCheckTheme;
  var toggle = document.getElementById("theme-toggle");
  if (toggle && themeApi) {
    var cycle = ["system", "light", "dark"];
    var labels = { system: "💻 System", light: "☀️ Light", dark: "🌙 Dark" };

    function render() {
      toggle.textContent = labels[themeApi.current()];
    }
    render();

    toggle.addEventListener("click", function () {
      var next = cycle[(cycle.indexOf(themeApi.current()) + 1) % cycle.length];
      if (next === "system") {
        localStorage.removeItem(themeApi.key);
      } else {
        localStorage.setItem(themeApi.key, next);
      }
      themeApi.apply(next);
      render();
    });
  }

  // ---------- dynamic latest-release version ----------
  var versionEls = document.querySelectorAll("[data-version]");
  if (versionEls.length) {
    fetch("https://api.github.com/repos/transfairs/vm-check/releases/latest", {
      headers: { Accept: "application/vnd.github+json" },
    })
      .then(function (res) {
        return res.ok ? res.json() : null;
      })
      .then(function (data) {
        if (!data || !data.tag_name) return;
        versionEls.forEach(function (el) {
          el.textContent = data.tag_name;
          el.hidden = false;
        });
      })
      .catch(function () {
        // Offline, rate-limited, or no release yet: leave the static copy as-is.
      });
  }
});
