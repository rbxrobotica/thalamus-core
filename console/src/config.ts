export interface AppConfig {
  baseUrl: string;
  authHeader?: string;
}

const STORAGE_KEY = "thalamus-console-config";

export function loadConfig(): AppConfig {
  try {
    const stored = localStorage.getItem(STORAGE_KEY);
    if (stored) return JSON.parse(stored);
  } catch {}
  return { baseUrl: "http://localhost:3000" };
}

export function saveConfig(config: AppConfig): void {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(config));
}
