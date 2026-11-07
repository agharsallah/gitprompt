// One connector per state, at a fixed time: what state.test.ts checks and the Design system page
// draws, so every state is reachable and shown from the same data.
import type { Source, SpecProblem } from '#lib/api/types.ts';
import type { ConnectorStateId } from '#lib/sources/state.ts';
import { after, source } from '#lib/testing/builders.ts';

export const NOW_ISO = '2026-11-07T15:40:00.000Z';
export const NOW = Date.parse(NOW_ISO);

const ago = (hours: number) => after(NOW_ISO, -hours * 3600);
/** A day of activity: busy in the afternoon, a quiet night. */
const DAY = [0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 6, 14, 9, 3, 0, 1, 12, 20, 16, 8, 11, 7, 4, 5];
const received = { total: 412, events: 9_870, last_at: ago(0.1), hourly: DAY };

export const STATE_FIXTURES: Record<ConnectorStateId, Source> = {
	disabled: source({
		id: 'copilot-cli',
		name: 'GitHub Copilot CLI',
		enabled: false,
		version: '1.0.107',
		config_file: '/Users/me/.copilot/hooks/gitprompt.json',
		join: 'session'
	}),
	not_installed: source({ detected: false, version: null }),
	available: source(),
	warning: source({
		id: 'claude-code-otel',
		name: 'Claude Code telemetry',
		part_of: 'claude-code',
		layer: 'Telemetry',
		warnings: [
			'OTEL_EXPORTER_OTLP_ENDPOINT is set in this shell’s environment: where it is also set, it wins over the settings file'
		]
	}),
	blocked_by_policy: source({
		blocks: [
			'disableAllHooks is set in /Library/Application Support/ClaudeCode/managed-settings.json'
		]
	}),
	outdated: source({ connected: true, outdated: true, ...received }),
	drift: source({ connected: true, ...received, unmapped: 3 }),
	waiting: source({ connected: true }),
	quiet: source({ connected: true, total: 88, events: 1_204, last_at: ago(30) }),
	receiving: source({ connected: true, ...received })
};

/** Spec files that can't be used, as GET /api/sources/problems reports them (Design system page). */
export const SPEC_PROBLEM_FIXTURES: SpecProblem[] = [
	{
		file: {
			name: 'claude-code.yaml',
			path: '/Users/me/.gitprompt/sources/claude-code.yaml',
			origin: 'user',
			overrides: true
		},
		error: 'source spec claude-code.yaml is not valid YAML: mapping values are not allowed here',
		fallback: 'claude-code'
	},
	{
		file: {
			name: 'mytool.yaml',
			path: '/Users/me/.gitprompt/sources/mytool.yaml',
			origin: 'user',
			overrides: false
		},
		error:
			'source spec mytool.yaml is invalid: 2 validation errors for Spec\ndetect\n  Field required\ninstall\n  Field required',
		fallback: null
	}
];
