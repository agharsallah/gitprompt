/**
 * What a panel says when no connected tool reports its numbers (spec `capabilities`): which tool
 * could and how, or that none can, rather than showing zeros. The words live here, next to the
 * availability they explain; the panel only draws the notice (lib/ui/PanelState.svelte).
 */
import { sourcesHref } from '#lib/shared/links.ts';
import { names, note, updateNote, type Availability } from '#lib/sources/capabilities.ts';
import type { Notice } from '#lib/ui/panelState.ts';

const capital = (text: string) => text.charAt(0).toUpperCase() + text.slice(1);

/** The first sentence: why there is nothing, from the most hopeful case to the least. */
function reason(what: string, a: Availability): string {
	const What = capital(what);
	if (a.available.length) return `${names(a.available)} reported no ${what} for these prompts yet.`;
	if (a.connectable.length) {
		const from = a.connectable.map((c) => `${c.tool}’s ${c.layer.toLowerCase()}`).join(' or ');
		return `${What} come from ${from}.`;
	}
	if (a.updatable.length) return `${What} need a newer version: ${updateNote(a)}`;
	const which = names(a.unavailable) || 'the connected tools';
	const verb = a.unavailable.length > 1 ? 'their connectors don’t' : 'its connector doesn’t';
	return `Not available for ${which}: ${verb} report ${what}.`;
}

/** What else is missing, said once: the first sentence already names it in the last two cases. */
function rest(a: Availability): string | null {
	if (a.available.length || a.connectable.length) return note(a);
	if (a.updatable.length && a.unavailable.length)
		return `Not available for ${names(a.unavailable)}.`;
	return null;
}

/** The notice for `what` ("costs", "slash commands"), with "Connect" when a tool could provide it. */
export function missingNotice(what: string, a: Availability): Notice {
	const tool = a.available.length ? undefined : a.connectable[0];
	return {
		title: a.available.length ? `No ${what} yet` : `${capital(what)} not available`,
		message: [reason(what, a), rest(a)].filter(Boolean).join(' '),
		next: tool && {
			label: `Connect ${tool.tool}`,
			href: sourcesHref({ tool: tool.toolId }),
			title: `Connectors › ${tool.tool}`
		}
	};
}

/**
 * For a figure no tool in use reports (a tile shows "Not reported", never 0): which tool could,
 * and how, or that none of them can.
 */
export function whoCouldReport(what: string, a: Availability): string {
	const layer = a.connectable[0];
	if (layer) return `${layer.tool}’s ${layer.layer.toLowerCase()} would report ${what}`;
	if (a.updatable.length) return updateNote(a);
	return a.unavailable.length
		? `${names(a.unavailable)} can’t report ${what}`
		: `No tool reports ${what}`;
}
