/** Saving a session's name from its card or its story header (InlineRename's `onsave`). */
import { namesApi } from '#lib/api/names.ts';

/**
 * `next` is the new name, or `null` to forget it (the default label shows again). Answers the
 * name as the daemon stored it, `null` once forgotten.
 */
export async function saveSessionName(
	id: string,
	next: string | null,
	fetcher?: typeof fetch
): Promise<string | null> {
	if (next === null) {
		await namesApi.clearSession(id, fetcher);
		return null;
	}
	return (await namesApi.setSession(id, next, fetcher)).name;
}
