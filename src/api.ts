import { invoke } from "@tauri-apps/api/core";

export interface Profile {
  id: string;
  name: string;
  detail: string | null;
  avatar: string | null;
  color: string | null;
}

export interface Browser {
  id: string;
  name: string;
  icon: string | null;
  profiles: Profile[];
}

export interface Rule {
  host: string;
  target: string;
}

export interface Config {
  hidden: string[];
  rules: Rule[];
}

export interface Snapshot {
  url: string | null;
  host: string | null;
  browsers: Browser[];
  config: Config;
  isDefault: boolean;
}

/** One row in the picker: a browser, or one profile of a browser. */
export interface Target {
  id: string;
  title: string;
  subtitle: string | null;
  image: string | null;
  color: string | null;
  browser: Browser;
}

export const api = {
  snapshot: () => invoke<Snapshot>("snapshot"),
  presentPicker: (height: number) => invoke("present_picker", { height }),
  openIn: (target: string, url: string, remember: boolean) => invoke("open_in", { target, url, remember }),
  dismiss: () => invoke("dismiss"),
  setHidden: (id: string, hidden: boolean) => invoke("set_hidden", { id, hidden }),
  removeRule: (host: string) => invoke("remove_rule", { host }),
  refreshBrowsers: () => invoke("refresh_browsers"),
  makeDefault: () => invoke("make_default"),
  openSettings: () => invoke("open_settings"),
};

/** Browsers with several profiles show one row per profile; the rest show one row. */
export function targetsOf(browser: Browser): Target[] {
  if (browser.profiles.length < 2) {
    return [{ id: browser.id, title: browser.name, subtitle: null, image: browser.icon, color: null, browser }];
  }
  return browser.profiles.map((p) => ({
    id: p.id,
    title: p.name,
    subtitle: p.detail,
    image: p.avatar,
    color: p.color,
    browser,
  }));
}

export function targetLabel(browsers: Browser[], id: string): string {
  for (const b of browsers) {
    if (b.id === id) return b.name;
    const p = b.profiles.find((p) => p.id === id);
    if (p) return `${b.name} — ${p.name}`;
  }
  return `${id} (not installed)`;
}
