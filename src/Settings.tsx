import { useCallback, useEffect, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { api, Snapshot, targetLabel } from "./api";
import Avatar from "./Avatar";

export default function Settings() {
  const [snap, setSnap] = useState<Snapshot | null>(null);
  const [message, setMessage] = useState<string | null>(null);

  const load = useCallback(() => api.snapshot().then(setSnap), []);

  useEffect(() => {
    load();
    // Pick up rules saved from the picker and default-browser changes made in the OS.
    const unlisten = getCurrentWindow().onFocusChanged(({ payload: focused }) => focused && load());
    return () => {
      unlisten.then((f) => f());
    };
  }, [load]);

  const run = async (action: () => Promise<unknown>, done?: string) => {
    try {
      await action();
      setMessage(done ?? null);
    } catch (e) {
      setMessage(String(e));
    }
    await load();
  };

  if (!snap) return null;
  const hidden = new Set(snap.config.hidden);
  const toggle = (id: string) => run(() => api.setHidden(id, !hidden.has(id)));

  return (
    <main className="settings">
      <section className="card default-card">
        <div>
          <h2>{snap.isDefault ? "Browser Picker is your default browser" : "Not your default browser yet"}</h2>
          <p>
            {snap.isDefault
              ? "Links you click in other apps open the picker."
              : "Make it the default so links from other apps show the picker."}
          </p>
        </div>
        {!snap.isDefault && (
          <button className="primary" onClick={() => run(api.makeDefault, "Confirm the change in the system prompt or settings page.")}>
            Make default
          </button>
        )}
      </section>
      {message && <p className="message">{message}</p>}

      <section className="card">
        <div className="card-head">
          <h2>Browsers</h2>
          <button onClick={() => run(api.refreshBrowsers, "Browser list refreshed.")}>Refresh</button>
        </div>
        <p className="muted">Untick anything you don't want in the picker.</p>
        {snap.browsers.length === 0 && <p className="empty">No browsers found.</p>}
        <ul className="browser-list">
          {snap.browsers.map((b) => (
            <li key={b.id}>
              <label className="check-row">
                <input type="checkbox" checked={!hidden.has(b.id)} onChange={() => toggle(b.id)} />
                <Avatar image={b.icon} label={b.name} size={24} />
                <span className="row-title">{b.name}</span>
              </label>
              {b.profiles.length > 1 && !hidden.has(b.id) && (
                <ul className="profile-list">
                  {b.profiles.map((p) => (
                    <li key={p.id}>
                      <label className="check-row">
                        <input type="checkbox" checked={!hidden.has(p.id)} onChange={() => toggle(p.id)} />
                        <Avatar image={p.avatar} label={p.name} color={p.color} size={22} round />
                        <span className="row-title">{p.name}</span>
                        {p.detail && <span className="row-sub">{p.detail}</span>}
                      </label>
                    </li>
                  ))}
                </ul>
              )}
            </li>
          ))}
        </ul>
      </section>

      <section className="card">
        <h2>Always open with</h2>
        {snap.config.rules.length === 0 ? (
          <p className="muted">
            No saved sites. Tick “Always open … with this choice” in the picker, or hold Alt when you choose.
          </p>
        ) : (
          <ul className="rules">
            {snap.config.rules.map((r) => (
              <li key={r.host}>
                <span className="rule-host">{r.host}</span>
                <span className="muted">{targetLabel(snap.browsers, r.target)}</span>
                <button onClick={() => run(() => api.removeRule(r.host))}>Remove</button>
              </li>
            ))}
          </ul>
        )}
      </section>
    </main>
  );
}
