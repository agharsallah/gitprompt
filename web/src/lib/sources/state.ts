// The state of a connector, derived from the raw facts GET /api/sources reports (api/routes/sources.py),
// and how every page shows it. One state -> one icon, label, tone, primary action and sentence, used by
// the Connectors page, the Settings summary, first-run steps and the Design system page. Pure, tested
// in state.test.ts against the fixtures in fixtures.ts.
import {
	Activity,
	CircleDashed,
	CircleFadingArrowUp,
	FileQuestionMark,
	Hourglass,
	Moon,
	Plug,
	PowerOff,
	ShieldBan,
	TriangleAlert,
	type LucideIcon
} from '@lucide/svelte';
import type { Source } from '#lib/api/types.ts';
import type { Tone } from '#lib/insights/outcome.ts';
import { plural, relative, relativePath } from '#lib/shared/format.ts';

/**
 * Every state a connector can be in. Exactly one applies, decided in this order:
 *
 *   either         disabled           turned off in GitPrompt: what it sends is discarded
 *   not connected  not_installed      the tool isn't on this machine
 *                  blocked_by_policy  a setting GitPrompt won't override turns it off
 *                  warning            it can connect, but a setting may stop its entries from running
 *                  available          ready to connect
 *   connected      blocked_by_policy  (as above: the entries can't run)
 *                  outdated           the spec now installs more or other entries
 *                  drift              prompts arrived without text: the tool's payload changed
 *                  waiting            nothing received yet
 *                  quiet              nothing received for QUIET_AFTER_MS
 *                  receiving          events are arriving
 *
 * A connected connector is never `not_installed` (its entries are in a file the tool reads) and
 * never `warning`: once connected, events arriving (or not) say more than the warning, which is
 * still listed under the state.
 */
export type ConnectorStateId =
	| 'disabled'
	| 'not_installed'
	| 'available'
	| 'warning'
	| 'blocked_by_policy'
	| 'outdated'
	| 'drift'
	| 'waiting'
	| 'quiet'
	| 'receiving';

export const STATE_IDS: readonly ConnectorStateId[] = [
	'disabled',
	'not_installed',
	'available',
	'warning',
	'blocked_by_policy',
	'outdated',
	'drift',
	'waiting',
	'quiet',
	'receiving'
];

/**
 * No event for this long = quiet. A day, the heartbeat's window: gaps of hours are ordinary use
 * (meetings, a night), so a shorter threshold would cry wolf every morning, while a whole day of
 * silence from a tool you use is worth a look. Shorter gaps show in the heartbeat itself.
 */
export const QUIET_AFTER_MS = 24 * 60 * 60 * 1000;

/** The one primary action of a state (Disconnect stays available as a secondary one when connected). */
export type ConnectorAction = 'enable' | 'recheck' | 'connect' | 'update' | 'remap' | 'view';

export type StateInfo = {
	label: string;
	icon: LucideIcon;
	tone: Tone;
	action: ConnectorAction;
	actionLabel: string;
};

export const STATES: Record<ConnectorStateId, StateInfo> = {
	disabled: {
		label: 'Turned off',
		icon: PowerOff,
		tone: 'neutral',
		action: 'enable',
		actionLabel: 'Turn on'
	},
	not_installed: {
		label: 'Not installed',
		icon: CircleDashed,
		tone: 'neutral',
		action: 'recheck',
		actionLabel: 'Check again'
	},
	available: {
		label: 'Ready to connect',
		icon: Plug,
		tone: 'accent',
		action: 'connect',
		actionLabel: 'Review and connect'
	},
	warning: {
		label: 'Check before connecting',
		icon: TriangleAlert,
		tone: 'warning',
		action: 'connect',
		actionLabel: 'Review and connect'
	},
	blocked_by_policy: {
		label: 'Blocked by policy',
		icon: ShieldBan,
		tone: 'danger',
		action: 'recheck',
		actionLabel: 'Check again'
	},
	outdated: {
		label: 'Update available',
		icon: CircleFadingArrowUp,
		tone: 'info',
		action: 'update',
		actionLabel: 'Update connection'
	},
	drift: {
		label: 'Payload drift',
		icon: FileQuestionMark,
		tone: 'warning',
		action: 'remap',
		actionLabel: 'Re-map stored events'
	},
	waiting: {
		label: 'Waiting for the first event',
		icon: Hourglass,
		tone: 'info',
		action: 'recheck',
		actionLabel: 'Check again'
	},
	quiet: {
		label: 'Quiet',
		icon: Moon,
		tone: 'neutral',
		action: 'view',
		actionLabel: 'View prompts'
	},
	receiving: {
		label: 'Receiving',
		icon: Activity,
		tone: 'success',
		action: 'view',
		actionLabel: 'View prompts'
	}
};

export function connectorState(s: Source, now: number): ConnectorStateId {
	if (!s.enabled) return 'disabled';
	if (!s.connected) {
		if (!s.detected) return 'not_installed';
		if (s.blocks.length) return 'blocked_by_policy';
		return s.warnings.length ? 'warning' : 'available';
	}
	if (s.blocks.length) return 'blocked_by_policy';
	if (s.outdated) return 'outdated';
	if (s.unmapped > 0) return 'drift';
	if (!s.last_at) return 'waiting';
	return now - Date.parse(s.last_at) > QUIET_AFTER_MS ? 'quiet' : 'receiving';
}

/** One sentence: why the connector is in this state and what the primary action does. */
export function explain(id: ConnectorStateId, s: Source, now: number): string {
	const file = relativePath(s.config_file);
	switch (id) {
		case 'disabled':
			return s.connected
				? `Turned off in GitPrompt: what it sends is discarded. Its entries stay in ${file} until you disconnect.`
				: 'Turned off in GitPrompt: it can’t be connected, and anything it sends is discarded.';
		case 'not_installed':
			return 'GitPrompt can’t find this tool on this machine: install it, then check again.';
		case 'available':
			return `Review the exact change to ${file}, then confirm; a backup is kept and Disconnect undoes it.`;
		case 'warning':
			return 'You can connect, but another setting may stop GitPrompt’s entries from running: read it before you do.';
		case 'blocked_by_policy':
			return s.connected
				? 'A setting GitPrompt won’t override now turns this off, so its entries don’t run.'
				: 'A setting GitPrompt won’t override turns this off, so it can’t connect until that changes.';
		case 'outdated':
			return `GitPrompt now captures more than the entries in ${file}: review the update to add the rest.`;
		case 'drift':
			return `${plural(s.unmapped, 'prompt')} arrived without text, so the tool may have changed its payload: fix the spec, then re-map.`;
		case 'waiting':
			return s.after_connect
				? `Connected. ${s.after_connect} Then events arrive once you start a new session and send a prompt.`
				: 'Connected: events arrive once you start a new session of the tool and send a prompt.';
		case 'quiet':
			return `Nothing for over a day (last event ${relative(s.last_at, now)}): normal if you haven’t used the tool, otherwise check it reads ${file}.${s.after_connect ? ` ${s.after_connect}` : ''}`;
		case 'receiving':
			return `Events are arriving; the last one came in ${relative(s.last_at, now)}.`;
	}
}

export type ConnectorPresentation = StateInfo & { id: ConnectorStateId; explanation: string };

export function present(s: Source, now: number): ConnectorPresentation {
	const id = connectorState(s, now);
	return { id, ...STATES[id], explanation: explain(id, s, now) };
}

// --- tools: a main connector and its optional layers ------------------------------------------

/** One tool as the user knows it (Claude Code), with its connectors as layers, the main one first. */
export type Tool = { id: string; name: string; layers: Source[] };

export function tools(sources: Source[]): Tool[] {
	const ids = new Set(sources.map((s) => s.id));
	const main = (s: Source) => (s.part_of && ids.has(s.part_of) ? s.part_of : s.id);
	const out = new Map<string, Tool>();
	for (const s of sources.filter((s) => main(s) === s.id)) {
		out.set(s.id, { id: s.id, name: s.name, layers: [s] });
	}
	for (const s of sources.filter((s) => main(s) !== s.id)) out.get(main(s))!.layers.push(s);
	return [...out.values()];
}

/** A layer's name within its tool: its `layer`, else its own name. */
export const layerName = (s: Source) => s.layer || s.name;

/** Where "View prompts" goes: this connector's prompts, or all of them for a layer without any. */
export const promptsQuery = (s: Source) =>
	s.total > 0 ? `?source=${encodeURIComponent(s.id)}` : '';
