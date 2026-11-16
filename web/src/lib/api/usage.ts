// The local usage counts: read and cleared here (Settings, Developer); reported by `track`
// (#lib/shared/usage.ts), which is the only writer of counts.
import { ApiError, get } from '#lib/api/client.ts';
import type { UsageOut } from '#lib/api/types.ts';
import { USAGE_URL } from '#lib/shared/usage.ts';

type Fetch = typeof fetch;

export const usageApi = {
	/** The counts of the last `days` days, today included. */
	get: (days: number, fetcher?: Fetch) => get<UsageOut>(`${USAGE_URL}?days=${days}`, fetcher),

	/** Forget every count. */
	clear: async (fetcher: Fetch = fetch) => {
		const response = await fetcher(USAGE_URL, {
			method: 'DELETE',
			headers: { 'X-GitPrompt': '1' }
		});
		if (!response.ok) throw new ApiError(response.status, USAGE_URL, response.statusText);
	}
};
