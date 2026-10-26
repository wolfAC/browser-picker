import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { api, Snapshot, Target, targetsOf } from "./api";
import Avatar from "./Avatar";

export default function Picker() {
  const [snap, setSnap] = useState<Snapshot | null>(null);
  // Bumped for every incoming link so the window re-measures and re-presents.
  const [openCount, setOpenCount] = useState(0);
  const [selected, setSelected] = useState(0);
  const [remember, setRemember] = useState(false);
  const [copied, setCopied] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const root = useRef<HTMLDivElement>(null);

  const load = useCallback(async () => {
    const next = await api.snapshot();
    if (!next.url) return;
    setSnap(next);
    setSelected(0);
    setRemember(false);
    setCopied(false);
    setError(null);
    setOpenCount((n) => n + 1);
  }, []);

  useEffect(() => {
    load();
    const unlisten = listen<string>("open-url", () => load());
    return () => {
      unlisten.then((f) => f());
    };
  }, [load]);

  const groups = useMemo(() => {
    if (!snap) return [];
    const hidden = new Set(snap.config.hidden);
    return snap.browsers
      .filter((b) => !hidden.has(b.id))
      .map((b) => ({ browser: b, targets: targetsOf(b).filter((t) => !hidden.has(t.id)) }))
      .filter((g) => g.targets.length > 0);
  }, [snap]);
  const targets = useMemo(() => groups.flatMap((g) => g.targets), [groups]);

  useLayoutEffect(() => {
    if (openCount === 0 || !root.current) return;
    api.presentPicker(Math.ceil(root.current.getBoundingClientRect().height));
  }, [openCount, error]);

  const choose = useCallback(
    async (target: Target, rememberChoice = remember) => {
      if (!snap?.url) return;
      try {
        await api.openIn(target.id, snap.url, rememberChoice);
        setSnap(null);
      } catch (e) {
        setError(String(e));
      }
    },
    [snap, remember],
  );

  const copy = useCallback(async () => {
    if (!snap?.url) return;
    await navigator.clipboard.writeText(snap.url);
    setCopied(true);
  }, [snap]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (!snap) return;
      const mod = e.metaKey || e.ctrlKey;
      if (e.key === "Escape") {
        api.dismiss();
      } else if (e.key === "ArrowDown" || (e.key === "Tab" && !e.shiftKey)) {
        setSelected((i) => (i + 1) % Math.max(targets.length, 1));
      } else if (e.key === "ArrowUp" || (e.key === "Tab" && e.shiftKey)) {
        setSelected((i) => (i - 1 + targets.length) % Math.max(targets.length, 1));
      } else if (e.key === "Enter" && targets[selected]) {
        choose(targets[selected], remember || e.altKey);
      } else if (/^[1-9]$/.test(e.key) && !mod && targets[Number(e.key) - 1]) {
        choose(targets[Number(e.key) - 1], remember || e.altKey);
      } else if (mod && e.key.toLowerCase() === "c" && !window.getSelection()?.toString()) {
        copy();
      } else {
        return;
      }
      e.preventDefault();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [snap, targets, selected, remember, choose, copy]);

  if (!snap?.url) return <div ref={root} />;

  let index = 0;
  return (
    <div ref={root} className="picker">
      <header className="picker-head">
        <div className="picker-url" title={snap.url}>
          <strong>{snap.host ?? "Local file"}</strong>
          <span>{snap.url}</span>
        </div>
        <button className="icon-btn" onClick={copy} title="Copy link (⌘/Ctrl+C)">
          {copied ? "✓" : "⧉"}
        </button>
        <button className="icon-btn" onClick={() => api.openSettings()} title="Settings">
          ⚙
        </button>
      </header>

      {error && <p className="picker-error">{error}</p>}

      <div className="picker-list">
        {targets.length === 0 && <p className="empty">No browsers to show. Check Settings.</p>}
        {groups.map(({ browser, targets: rows }) => {
          const grouped = browser.profiles.length > 1;
          return (
            <section key={browser.id} className="group">
              {grouped && (
                <div className="group-head">
                  <Avatar image={browser.icon} label={browser.name} size={16} />
                  {browser.name}
                </div>
              )}
              {rows.map((t) => {
                const i = index++;
                return (
                  <button
                    key={t.id}
                    className={`row${i === selected ? " selected" : ""}${grouped ? " indented" : ""}`}
                    onMouseEnter={() => setSelected(i)}
                    onClick={(e) => choose(t, remember || e.altKey)}
                  >
                    <Avatar image={t.image} label={t.title} color={t.color} round={grouped} />
                    <span className="row-text">
                      <span className="row-title">{t.title}</span>
                      {t.subtitle && <span className="row-sub">{t.subtitle}</span>}
                    </span>
                    {i < 9 && <kbd>{i + 1}</kbd>}
                  </button>
                );
              })}
            </section>
          );
        })}
      </div>

      {snap.host && (
        <label className="picker-foot">
          <input type="checkbox" checked={remember} onChange={(e) => setRemember(e.target.checked)} />
          Always open <strong>{snap.host}</strong> with this choice
          <span className="hint">or hold Alt</span>
        </label>
      )}
    </div>
  );
}
