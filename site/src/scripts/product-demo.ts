document.querySelectorAll<HTMLElement>("[data-product-demo]").forEach((root) => {
  const tabs = [...root.querySelectorAll<HTMLButtonElement>("[data-demo-step]")];
  const panels = [...root.querySelectorAll<HTMLElement>("[data-demo-block]")];
  const viewport = root.querySelector<HTMLElement>("[data-demo-viewport]")!;
  const captions = [...root.querySelectorAll<HTMLElement>("[data-demo-caption]")];
  const toggle = root.querySelector<HTMLButtonElement>("[data-demo-toggle]")!;
  const reducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)");
  let scene = 0;
  let playing = !reducedMotion.matches;
  let visible = false;
  let timer: number | undefined;
  let frame: number | undefined;
  let progress: Animation | undefined;
  const durations = [2400, 4200, 4200, 7000];
  const scrollToLatest = () => { viewport.scrollTop = viewport.scrollHeight; };

  const finishTyping = () => {
    window.cancelAnimationFrame(frame ?? 0);
    root.dataset.typing = "false";
    panels.forEach((panel) => {
      const command = panel.querySelector<HTMLElement>("[data-demo-command]")!;
      command.textContent = command.dataset.source ?? "";
      command.removeAttribute("data-typing");
      panel.dataset.complete = "true";
    });
  };

  function schedule() {
    window.clearTimeout(timer);
    progress?.cancel();
    const active = playing && visible && !document.hidden;
    root.dataset.playing = String(active);
    toggle.setAttribute("aria-label", (playing ? root.dataset.pause : root.dataset.play) ?? "");
    toggle.title = toggle.getAttribute("aria-label") ?? "";
    root.querySelector<SVGElement>("[data-play-icon]")!.style.display = playing ? "none" : "";
    root.querySelector<SVGElement>("[data-pause-icon]")!.style.display = playing ? "" : "none";
    root.querySelector("[data-pause-icon]")!.removeAttribute("hidden");
    if (!active) return;
    const duration = durations[scene];
    if (!reducedMotion.matches)
      progress = tabs[scene]
        .querySelector<HTMLElement>(".step-progress")!
        .animate([{ transform: "scaleX(0)" }, { transform: "scaleX(1)" }], {
          duration,
          fill: "forwards",
        });
    timer = window.setTimeout(() => show((scene + 1) % panels.length), duration);
  }

  function show(index: number, animate = true) {
    finishTyping();
    scene = index;
    root.dataset.scene = String(index);
    tabs.forEach((tab, i) => {
      if (i === index) tab.setAttribute("aria-current", "step");
      else tab.removeAttribute("aria-current");
      tab.tabIndex = i === index ? 0 : -1;
    });
    panels.forEach((panel, i) => {
      panel.hidden = i > index;
    });
    captions.forEach((caption, i) => {
      caption.hidden = i !== index;
    });
    const panel = panels[index];
    if (animate && !reducedMotion.matches) {
      const command = panel.querySelector<HTMLElement>("[data-demo-command]")!;
      const source = command.dataset.source ?? "";
      const started = performance.now();
      const typingDuration = Math.min(1100, source.length * 14);
      command.textContent = "";
      command.dataset.typing = "true";
      root.dataset.typing = "true";
      panel.dataset.complete = "false";
      const type = (now: number) => {
        const count = Math.min(
          source.length,
          Math.floor(((now - started) / typingDuration) * source.length),
        );
        command.textContent = source.slice(0, count);
        if (count < source.length) frame = window.requestAnimationFrame(type);
        else {
          command.removeAttribute("data-typing");
          panel.dataset.complete = "true";
          root.dataset.typing = "false";
        }
        scrollToLatest();
      };
      frame = window.requestAnimationFrame(type);
    }
    scrollToLatest();
    schedule();
  }

  tabs.forEach((tab, index) => {
    const select = (next: number) => {
      playing = false;
      show(next);
      tabs[next].focus({ preventScroll: true });
    };
    tab.addEventListener("click", () => select(index));
    tab.addEventListener("keydown", (event) => {
      let next: number | undefined;
      if (event.key === "ArrowRight") next = (index + 1) % tabs.length;
      if (event.key === "ArrowLeft") next = (index + tabs.length - 1) % tabs.length;
      if (event.key === "Home") next = 0;
      if (event.key === "End") next = tabs.length - 1;
      if (next !== undefined) {
        event.preventDefault();
        select(next);
      }
    });
  });
  toggle.addEventListener("click", () => {
    playing = !playing;
    if (!playing) { finishTyping(); scrollToLatest(); }
    schedule();
  });
  viewport.addEventListener("pointerdown", () => {
    playing = false;
    finishTyping();
    schedule();
  });
  root.querySelector<HTMLButtonElement>("[data-demo-replay]")!.addEventListener("click", () => {
    playing = true;
    show(0);
  });
  reducedMotion.addEventListener("change", () => {
    playing = !reducedMotion.matches;
    finishTyping();
    schedule();
  });
  document.addEventListener("visibilitychange", () => {
    if (document.hidden) finishTyping();
    schedule();
  });
  const observer = new IntersectionObserver(
    ([entry]) => {
      visible = entry.isIntersecting;
      schedule();
    },
    { threshold: 0.2 },
  );
  observer.observe(root);
  window.addEventListener("pagehide", () => {
    window.clearTimeout(timer);
    progress?.cancel();
    finishTyping();
    observer.disconnect();
  });
  schedule();
});
