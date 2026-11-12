// Settings page endpoints: preferences (/api/settings), the database (/api/data) and the receiver
// token (/api/security). Kept apart from client.ts/types.ts so the settings domain stays in one place.
import { get, write } from '#lib/api/client.ts';
import type {
	Backup,
	CleanupReport,
	DataInfo,
	DeleteIn,
	Preferences,
	Removal,
	Rotation,
	Security,
	SettingOut,
	SettingsState as SettingsWire,
	ValueSource
} from '#lib/api/types.ts';

// The wire types are generated from Rust (types.ts); re-exported here for the settings pages.
export type { Backup, CleanupReport, DataInfo, Preferences, Removal, Rotation, Security };

type Fetch = typeof fetch;

export type PreferenceKey = keyof Preferences;

export type SettingSource = ValueSource;

/** One setting, its value typed by its key (the wire's `SettingOut` carries any JSON value). */
export type Setting<K extends PreferenceKey = PreferenceKey> = Omit<
	SettingOut,
	'value' | 'default'
> & {
	value: Preferences[K];
	default: Preferences[K];
};

/** GET /api/settings, each setting typed by its key. */
export type SettingsState = Omit<SettingsWire, 'settings'> & {
	settings: { [K in PreferenceKey]: Setting<K> };
};

export type SettingsSaved = SettingsState & { changed: PreferenceKey[]; restart_required: boolean };

export type DeleteSelection = DeleteIn;

const run = (dryRun: boolean) => (dryRun ? '' : '?dry_run=false');

export const settingsApi = {
	get: (fetcher?: Fetch) => get<SettingsState>('/api/settings', fetcher),
	/** A null value resets a setting to its default. */
	save: (changes: Partial<{ [K in PreferenceKey]: Preferences[K] | null }>) =>
		write<SettingsSaved>('/api/settings', 'PUT', changes),

	data: (fetcher?: Fetch) => get<DataInfo>('/api/data', fetcher),
	/** Destructive actions preview by default; `dryRun: false` applies. */
	compact: (olderThanDays: number | null, dryRun = true) =>
		write<CleanupReport>(`/api/data/compact${run(dryRun)}`, 'POST', {
			older_than_days: olderThanDays
		}),
	deleteEvents: (selection: DeleteSelection, dryRun = true) =>
		write<CleanupReport>(`/api/data/delete-events${run(dryRun)}`, 'POST', selection),

	security: (fetcher?: Fetch) => get<Security>('/api/security', fetcher),
	rotateToken: (dryRun = true) =>
		write<Rotation>(`/api/security/rotate-token${run(dryRun)}`, 'POST')
};
