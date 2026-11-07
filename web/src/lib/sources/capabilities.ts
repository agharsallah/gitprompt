// What each connector can feed into the dashboard (spec `capabilities`, sources/spec.py), and the one
// helper pages call to say "not available for <tool>" instead of showing zeros. A tool has the
// capabilities of its active layers: connected, or with data already received. Pure, tested in
// capabilities.test.ts.
import type { Capability, Source } from '#lib/api/types.ts';
import { layerName, tools, type Tool } from '#lib/sources/state.ts';

/** Every capability, in the order cards list them, with the words pages use for it. */
export const CAPABILITIES: Record<Capability, { label: string; what: string }> = {
	prompt: { label: 'Prompts', what: 'prompt text' },
	cwd: { label: 'Folder', what: 'the working folder' },
	model: { label: 'Model', what: 'models' },
	usage: { label: 'Tokens', what: 'token counts' },
	cost: { label: 'Cost', what: 'costs' },
	turn_end: { label: 'Turn end', what: 'when a turn ends' },
	answer: { label: 'Answer', what: 'the final answer' },
	tools: { label: 'Tools', what: 'tool calls' },
	subagents: { label: 'Subagents', what: 'subagents' },
	origin_markers: { label: 'Origin', what: 'who wrote a prompt' },
	transcript_path: { label: 'Transcript path', what: 'the transcript path' },
	session_lifecycle: { label: 'Sessions', what: 'session start and end' },
	session_metrics: {
		label: 'Session metrics',
		what: 'lines of code, commits and slash commands per session'
	}
};

export const CAPABILITY_IDS = Object.keys(CAPABILITIES) as Capability[];

/** A layer counts once it is connected or has sent something, and is not turned off. */
const active = (s: Source) => s.enabled && (s.connected || s.total + s.events > 0);

/** What a tool yields now (its active layers), and what its other layers would add. */
export function toolCapabilities(tool: Tool): { has: Capability[]; offered: Capability[] } {
	const has = new Set(tool.layers.filter(active).flatMap((l) => l.capabilities));
	const offered = new Set(tool.layers.flatMap((l) => l.capabilities).filter((c) => !has.has(c)));
	return {
		has: CAPABILITY_IDS.filter((c) => has.has(c)),
		offered: CAPABILITY_IDS.filter((c) => offered.has(c))
	};
}

export type Availability = {
	capability: Capability;
	/** Tools that provide it now. */
	available: string[];
	/** Tools whose optional layer would provide it once connected. */
	connectable: { tool: string; toolId: string; layer: string }[];
	/** Tools whose newer version would provide it (spec `capability_since`). */
	updatable: { tool: string; version: string }[];
	/** Tools that can't provide it at all. */
	unavailable: string[];
};

/**
 * Who can provide `capability`, among the tools in use (a layer active), or among every tool when
 * none is in use yet (first run). Panels show what `note` says next to their numbers.
 */
export function availability(sources: Source[], capability: Capability): Availability {
	const all = tools(sources);
	const inUse = all.filter((t) => t.layers.some(active));
	const out: Availability = {
		capability,
		available: [],
		connectable: [],
		updatable: [],
		unavailable: []
	};
	for (const tool of inUse.length ? inUse : all) {
		const { has } = toolCapabilities(tool);
		const layer = tool.layers.find((l) => l.enabled && l.capabilities.includes(capability));
		const since = tool.layers.map((l) => l.needs_version?.[capability]).find(Boolean);
		if (has.includes(capability)) out.available.push(tool.name);
		else if (layer)
			out.connectable.push({ tool: tool.name, toolId: tool.id, layer: layerName(layer) });
		else if (since) out.updatable.push({ tool: tool.name, version: since });
		else out.unavailable.push(tool.name);
	}
	return out;
}

/** The tool a connector (a prompt's `source`) belongs to, with all its layers. */
export const toolOf = (sources: Source[], source: string): Tool | undefined =>
	tools(sources).find((t) => t.layers.some((l) => l.id === source));

/** Who answers in a tool, e.g. "Claude" (its main connector's `assistant`), else "Agent". */
export const assistantOf = (tool: Tool | undefined) => tool?.layers[0].assistant || 'Agent';

/** "A", "A and B", "A, B and C". */
export const names = (list: string[]) =>
	list.length < 2 ? (list[0] ?? '') : `${list.slice(0, -1).join(', ')} and ${list.at(-1)}`;

/** "Needs OpenAI Codex CLI 0.164.0 or later." for the tools an update would give it. */
export const updateNote = (a: Availability) =>
	a.updatable.map((u) => `Needs ${u.tool} ${u.version} or later.`).join(' ');

/** One line for a panel whose numbers can't include every tool, or null when nothing is missing. */
export function note(a: Availability): string | null {
	const text = [
		a.unavailable.length ? `Not available for ${names(a.unavailable)}.` : '',
		updateNote(a)
	]
		.filter(Boolean)
		.join(' ');
	return text || null;
}
