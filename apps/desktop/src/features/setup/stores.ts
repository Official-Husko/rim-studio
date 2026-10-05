import { resetCe } from './ceStore';
import { resetDetect } from './detectStore';
import { resetScan } from './scanStore';
import { resetSettings } from './settingsStore';
import { resetSources } from './sourcesStore';

/** Reset every store of the setup feature (tests and session changes). */
export function resetSetupStores(): void {
  resetDetect();
  resetSources();
  resetScan();
  resetCe();
  resetSettings();
}
