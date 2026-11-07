/** Loading a story: a story the daemon does not know is a 404 page, not a crash. */
import { error } from '@sveltejs/kit';

export async function orNotFound<T>(request: Promise<T>, what: string): Promise<T> {
	try {
		return await request;
	} catch (e) {
		const message = e instanceof Error ? e.message : String(e);
		if (message.startsWith('404')) error(404, `No ${what} with this id was captured.`);
		throw e;
	}
}
