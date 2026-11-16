const toggle = document.querySelector(".theme-toggle");

// A quiet HL7 stream sits behind the page content and adapts to the chosen theme.
const reducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)");
if (!reducedMotion.matches) {
  const canvas = document.createElement("canvas");
  canvas.className = "data-flow";
  canvas.setAttribute("aria-hidden", "true");
  Object.assign(canvas.style, {
    position: "fixed", inset: "0", width: "100%", height: "100%",
    pointerEvents: "none", zIndex: "0", opacity: "0.52"
  });
  document.body.prepend(canvas);
  document.querySelectorAll(".page").forEach((page) => {
    page.style.position = "relative";
    page.style.zIndex = "1";
  });

  const context = canvas.getContext("2d");
  const particles = Array.from({ length: 76 }, (_, index) => ({
    progress: Math.random(), lane: index % 5 - 2,
    speed: 0.000035 + Math.random() * 0.000035,
    size: index % 5 === 0 ? 11 : 2 + Math.random() * 1.5,
    label: ["MSH", "PID", "PV1", "OBX", "|", "^", "~"][index % 7],
    phase: Math.random() * Math.PI * 2
  }));
  let width = 0;
  let height = 0;
  let frame = 0;

  function resize() {
    const ratio = Math.min(window.devicePixelRatio || 1, 1.5);
    width = window.innerWidth;
    height = window.innerHeight;
    canvas.width = Math.round(width * ratio);
    canvas.height = Math.round(height * ratio);
    context.setTransform(ratio, 0, 0, ratio, 0, 0);
  }

  function draw(time) {
    const dark = document.documentElement.dataset.theme === "dark";
    const color = dark ? "119, 231, 151" : "19, 139, 76";
    context.clearRect(0, 0, width, height);
    // The faint curved lanes suggest messages flowing into an interface endpoint.
    for (let lane = -2; lane <= 2; lane++) {
      context.beginPath();
      context.moveTo(-20, height * (0.29 + lane * 0.09));
      context.bezierCurveTo(width * 0.32, height * (0.17 + lane * 0.13), width * 0.52, height * (0.82 + lane * 0.08), width * 0.82, height * 0.52);
      context.strokeStyle = `rgba(${color}, ${dark ? 0.10 : 0.075})`;
      context.lineWidth = 1;
      context.stroke();
    }
    // Small green message fields drift along the lanes, then converge at the hub.
    for (const particle of particles) {
      particle.progress += particle.speed * 16;
      if (particle.progress > 1) particle.progress -= 1;
      const t = particle.progress;
      // Use the exact same Bezier curve as the faint lane drawn above.
      const x0 = -20;
      const x1 = width * 0.32;
      const x2 = width * 0.52;
      const x3 = width * 0.82;
      const y0 = height * (0.29 + particle.lane * 0.09);
      const y1 = height * (0.17 + particle.lane * 0.13);
      const y2 = height * (0.82 + particle.lane * 0.08);
      const y3 = height * 0.52;
      const inverse = 1 - t;
      const x = inverse ** 3 * x0 + 3 * inverse ** 2 * t * x1 + 3 * inverse * t ** 2 * x2 + t ** 3 * x3;
      const y = inverse ** 3 * y0 + 3 * inverse ** 2 * t * y1 + 3 * inverse * t ** 2 * y2 + t ** 3 * y3;
      context.fillStyle = `rgba(${color}, ${0.25 + 0.48 * (1 - t)})`;
      if (particle.size > 5) {
        context.font = `600 ${particle.size}px ui-monospace, monospace`;
        context.fillText(particle.label, x, y);
      } else {
        context.beginPath();
        context.arc(x, y, particle.size, 0, Math.PI * 2);
        context.fill();
      }
    }
    // Quiet endpoint node at the shared end point of each curve.
    const hubX = width * 0.82;
    const hubY = height * 0.52;
    context.beginPath();
    context.arc(hubX, hubY, 3, 0, Math.PI * 2);
    context.fillStyle = `rgba(${color}, ${dark ? 0.5 : 0.32})`;
    context.fill();
    frame = window.requestAnimationFrame(draw);
  }

  resize();
  window.addEventListener("resize", resize, { passive: true });
  document.addEventListener("visibilitychange", () => {
    if (document.hidden) window.cancelAnimationFrame(frame);
    else frame = window.requestAnimationFrame(draw);
  });
  frame = window.requestAnimationFrame(draw);
}

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
