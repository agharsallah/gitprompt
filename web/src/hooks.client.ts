import type { HandleClientError } from '@sveltejs/kit/hooks';

// Local-only dashboard: show the real error instead of SvelteKit's generic "Internal Error"
// (lib/errors/describe.ts turns it into a title and a hint). Only unexpected errors are overridden;
// 404s and error() calls keep their own status and message.
export const handleError: HandleClientError = ({ error, kind }) => {
	console.error(error);
	if (kind !== 'unknown') return;
	return { message: error instanceof Error ? error.message : String(error) };
};
