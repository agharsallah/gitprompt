/**
 * What the Overview says when nothing needs you: the period in one line, from the key figures
 * already loaded ("Last 7 days: 278 prompts · $453 · 181 commits"). Pure.
 */
import type { StatsBucket } from '#lib/api/types.ts';
import type { Range } from '#lib/insights/range.ts';
import { money, plural } from '#lib/shared/format.ts';

/** "Last 7 days", "Last 24 hours", "All time". */
export const periodName = (range: Range) =>
	range.days == null ? range.label : `Last ${range.label}`;

export function calmLine(range: Range, t: StatsBucket): string {
	const parts = [
		t.prompts ? plural(t.prompts, 'prompt') : 'no prompts',
		t.cost_usd > 0 ? money(t.cost_usd) : '',
		t.git_commits > 0 ? plural(t.git_commits, 'commit') : ''
	];
	return `${periodName(range)}: ${parts.filter(Boolean).join(' · ')}`;
}
