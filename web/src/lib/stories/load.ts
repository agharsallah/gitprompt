/** Loading a story: a story the daemon does not know is a 404 page, not a crash. */
import { error } from '@sveltejs/kit';
import { ApiError } from '#lib/api/client.ts';

export async function orNotFound<T>(request: Promise<T>, what: string): Promise<T> {
	try {
		return await request;
	} catch (e) {
		if (e instanceof ApiError && e.status === 404)
			error(404, `No ${what} with this id was captured.`);
		throw e;
	}
}
