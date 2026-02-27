import { load } from '@tauri-apps/plugin-store';
import type { AppConfig } from './config';
import { DEFAULT_CONFIG } from './config';

let storeInstance: Awaited<ReturnType<typeof load>> | null = null;

async function getStore() {
  if (!storeInstance) {
    storeInstance = await load('config.json', {
      autoSave: true,
      defaults: { config: DEFAULT_CONFIG },
    });
  }
  return storeInstance;
}

export async function loadConfig(): Promise<AppConfig> {
  const store = await getStore();
  const config = await store.get<AppConfig>('config');
  return config ?? DEFAULT_CONFIG;
}

export async function saveConfig(config: AppConfig): Promise<void> {
  const store = await getStore();
  await store.set('config', config);
}
