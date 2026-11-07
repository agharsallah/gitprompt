// Query strings for GET /api/prompts and GET /api/stats: both take the same filters
// (api/filters.py). Lists repeat the parameter (`origin=typed&origin=slash_command`); an empty
// list or value is left out, which the daemon reads as "any".
import type { GroupBy, PromptQuery, StatsOrder } from '#lib/api/types.ts';

export type StatsQuery = PromptQuery & {
	group_by?: GroupBy;
	split_by?: GroupBy;
	order?: StatsOrder;
	limit?: number;
};

export function queryParams(query: StatsQuery): URLSearchParams {
	const params = new URLSearchParams();
	for (const [key, value] of Object.entries(query)) {
		for (const v of Array.isArray(value) ? value : [value]) {
			if (v !== undefined && v !== null && v !== '') params.append(key, String(v));
		}
	}
	return params;
}
