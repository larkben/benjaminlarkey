const toggle = document.querySelector(".theme-toggle");

function updateThemeButton() {
  const isDark = document.documentElement.dataset.theme === "dark";
  toggle.textContent = isDark ? "☀ Light mode" : "☾ Dark mode";
  toggle.setAttribute("aria-label", isDark ? "Switch to light mode" : "Switch to dark mode");
  toggle.setAttribute("aria-pressed", String(isDark));
}

if (toggle) {
  updateThemeButton();
  toggle.addEventListener("click", () => {
    const nextTheme = document.documentElement.dataset.theme === "dark" ? "light" : "dark";
    document.documentElement.dataset.theme = nextTheme;
    try { localStorage.setItem("theme", nextTheme); } catch {}
    updateThemeButton();
  });
}
