const STORAGE_KEY = 'thalamus-console-config';

export interface AppConfig {
  baseUrl: string;
  authHeader?: string;
  observabilityUrl?: string;
  observabilityToken?: string;
  memoryUrl?: string;
  maestroUrl?: string;
}

function load(): AppConfig {
  if (typeof window === 'undefined') return { baseUrl: '' };
  const stored = sessionStorage.getItem(STORAGE_KEY);
  if (stored) {
    try {
      return JSON.parse(stored);
    } catch {
      /* corrupted — reset */
    }
  }
  return { baseUrl: '' };
}

function save(cfg: AppConfig) {
  if (typeof window === 'undefined') return;
  sessionStorage.setItem(STORAGE_KEY, JSON.stringify(cfg));
}

let config = $state<AppConfig>(load());

$effect(() => {
  save(config);
});

export function getConfig(): AppConfig {
  return config;
}

export function setConfig(cfg: AppConfig) {
  config = cfg;
}

export function resetConfig() {
  config = { baseUrl: '' };
  sessionStorage.removeItem(STORAGE_KEY);
}
