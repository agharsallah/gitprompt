// The names you give sessions (and later chapters): set and forgotten from a card or a story header.
import { ApiError, write } from '#lib/api/client.ts';
import type { NameOut } from '#lib/api/types.ts';

const sessionUrl = (id: string) => `/api/names/session/${encodeURIComponent(id)}`;

type Fetch = typeof fetch;

export const namesApi = {
	/** Call a session `name`; answers the name as stored (trimmed, white space collapsed). */
	setSession: (id: string, name: string, fetcher?: Fetch) =>
		write<NameOut>(sessionUrl(id), 'PUT', { name }, fetcher),

	/** Forget a session's name: it shows its default label again. */
	clearSession: async (id: string, fetcher: Fetch = fetch) => {
		const response = await fetcher(sessionUrl(id), {
			method: 'DELETE',
			headers: { 'X-GitPrompt': '1' }
		});
		if (!response.ok) throw new ApiError(response.status, sessionUrl(id), response.statusText);
	}
};
