// Settings page endpoints: preferences (/api/settings), the database (/api/data) and the receiver
// token (/api/security). Kept apart from client.ts/types.ts so the settings domain stays in one place.
import { get, write } from '#lib/api/client.ts';
import type { Change } from '#lib/api/types.ts';
import type { RangeId } from '#lib/insights/range.ts';
import type { WeekStart } from '#lib/insights/week.ts';

type Fetch = typeof fetch;

/** Mirrors core/preferences.py `Preferences`. */
export type Preferences = {
	default_range: RangeId;
	week_start: WeekStart;
	event_retention_days: number | null;
	hide_secrets_on_ingest: boolean;
};
export type PreferenceKey = keyof Preferences;

export type SettingSource = 'env' | 'file' | 'default';

export type Setting<K extends PreferenceKey = PreferenceKey> = {
	value: Preferences[K];
	default: Preferences[K];
	/** Where the value comes from: env > file > default. */
	source: SettingSource;
	/** The environment variable that sets (and locks) it. */
	env: string;
	locked: boolean;
	applies: 'now' | 'restart';
};

export type SettingsState = {
	file: string;
	settings: { [K in PreferenceKey]: Setting<K> };
};

export type SettingsSaved = SettingsState & { changed: PreferenceKey[]; restart_required: boolean };

export type Backup = { name: string; path: string; bytes: number; modified: string };

export type DataInfo = {
	home: string;
	db_path: string;
	db_bytes: number;
	receiver: string;
	events: { source: string; rows: number; bytes: number }[];
	backups: Backup[];
};

export type Removal = {
	source: string;
	event: string | null;
	rows: number;
	bytes: number;
	why: 'dropped by spec' | 'expired' | 'selected';
	before: string | null;
};

/** What a clean-up removes (dry run) or removed (applied). */
export type CleanupReport = {
	removals: Removal[];
	rows: number;
	bytes: number;
	size_before: number;
	size_after: number | null;
	applied: boolean;
};

export type DeleteSelection = { source: string | null; before: string | null };

export type Security = { token_file: string; token_changed: string; connected: string[] };

export type Rotation = {
	reconnects: { source: string; name: string; change: Change }[];
	blocked: string[];
	applied: boolean;
};

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
