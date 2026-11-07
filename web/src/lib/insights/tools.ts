import {
	Bot,
	FilePen,
	FileSearch,
	Globe,
	Plug,
	SquareTerminal,
	Wrench,
	type LucideIcon
} from '@lucide/svelte';

/**
 * What a tool call does, in the order the story of a turn usually goes: look around, change
 * files, run things, look things up, use integrations, delegate. Each kind owns one chart slot
 * (color follows the kind, never its rank), and stacked bars draw kinds in this order, which is
 * the validated adjacency order of the chart palette.
 */
export const TOOL_KINDS = ['read', 'write', 'shell', 'web', 'mcp', 'agent', 'other'] as const;
export type ToolKind = (typeof TOOL_KINDS)[number];

export const KIND_INFO: Record<ToolKind, { label: string; color: string; icon: LucideIcon }> = {
	read: { label: 'Read & search', color: 'var(--chart-1)', icon: FileSearch },
	write: { label: 'Edit files', color: 'var(--chart-2)', icon: FilePen },
	shell: { label: 'Shell', color: 'var(--chart-3)', icon: SquareTerminal },
	web: { label: 'Web', color: 'var(--chart-4)', icon: Globe },
	mcp: { label: 'MCP tools', color: 'var(--chart-5)', icon: Plug },
	agent: { label: 'Agents & tasks', color: 'var(--chart-6)', icon: Bot },
	other: { label: 'Other', color: 'var(--chart-other)', icon: Wrench }
};

const isKind = (value: unknown): value is ToolKind => TOOL_KINDS.includes(value as ToolKind);

/**
 * The kind of a tool, as the daemon classified it (derivations.tool_kind, sent as
 * `summary.tool_kind_of` on a prompt and as `detail` of a `tool` stats group). A kind this dashboard
 * does not know draws as "other".
 */
export function kindOf(name: string, kinds?: Record<string, string> | null): ToolKind {
	const kind = kinds?.[name];
	return isKind(kind) ? kind : 'other';
}

/** A kind from the daemon (a `tool_kind` group key), or "other". */
export const asKind = (kind: string | null | undefined): ToolKind =>
	isKind(kind) ? kind : 'other';

/** "mcp__claude_ai_Figma__get_screenshot" -> { server: "claude_ai_Figma", tool: "get_screenshot" }. */
export function mcpParts(name: string): { server: string; tool: string } | null {
	const m = name.match(/^mcp__(.+?)__(.+)$/);
	return m ? { server: m[1], tool: m[2] } : null;
}

/** A tool name short enough for a chip: MCP tools lose their prefix. */
export const toolLabel = (name: string) => mcpParts(name)?.tool ?? name;

/** Tool calls by kind (`summary.tool_kinds`), in kind order, zeros dropped. */
export function byKind(
	kinds: Record<string, number> | undefined
): { kind: ToolKind; count: number }[] {
	const totals = new Map<ToolKind, number>();
	for (const [kind, n] of Object.entries(kinds ?? {})) {
		totals.set(asKind(kind), (totals.get(asKind(kind)) ?? 0) + n);
	}
	return TOOL_KINDS.filter((k) => totals.get(k)).map((kind) => ({
		kind,
		count: totals.get(kind)!
	}));
}
