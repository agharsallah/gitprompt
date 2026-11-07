/**
 * What an error means to the person looking at the dashboard: one of a few kinds, each with a
 * title, what to do about it, and which actions make sense. Every error the dashboard shows (the
 * error page, a section that failed to load, a panel that failed to render) goes through here, so
 * the same failure always reads the same way.
 */
import { isHttpError } from '@sveltejs/kit';
import { ApiError } from '#lib/api/client.ts';

export type ErrorKind = 'not-found' | 'offline' | 'outdated' | 'daemon' | 'unexpected';

export type Described = {
	kind: ErrorKind;
	/** HTTP status when there is one (a daemon answer or a SvelteKit error page). */
	status: number | null;
	title: string;
	/** What happened and what to do, in one or two sentences. */
	hint: string;
	/** The raw message, for the "Details" disclosure. Empty when it adds nothing. */
	detail: string;
	/** Trying again can help (not for a missing page or an out-of-date dashboard). */
	retry: boolean;
	/** Only a full reload fixes it (the dashboard's files changed under the open page). */
	reload: boolean;
};

/** An error as it reaches the page: a thrown value, or SvelteKit's `page.error` plus `page.status`. */
type Source = unknown;

// fetch() rejects with a TypeError when nothing answers; each engine words it differently.
const OFFLINE = /failed to fetch|networkerror|load failed|network request failed/i;
// A lazily imported chunk that no longer exists: the daemon was upgraded while the page was open.
const OUTDATED =
	/dynamically imported module|importing a module script failed|error loading dynamically|unable to preload css/i;
// SvelteKit's own message for a route that does not exist.
const KIT_NOT_FOUND = /^not found/i;

function statusOf(error: Source, fallback?: number): number | null {
	if (isHttpError(error)) return error.status;
	if (error instanceof ApiError) return error.status;
	return fallback ?? null;
}

export function messageOf(error: Source): string {
	if (isHttpError(error)) return error.body.message;
	if (error instanceof Error) return error.message;
	if (error && typeof error === 'object' && 'message' in error) return String(error.message);
	return error === undefined || error === null ? '' : String(error);
}

/**
 * Describe `error`. `status` is the page status when the error comes from `page.error`;
 * `what` names the thing that failed ("the overview", "this panel") for the generic titles.
 */
export function describeError(error: Source, status?: number, what?: string): Described {
	const code = statusOf(error, status);
	const message = messageOf(error);
	const base = { status: code, detail: message, retry: true, reload: false };

	if (code === 404) {
		const generic = !message || KIT_NOT_FOUND.test(message);
		return {
			...base,
			kind: 'not-found',
			title: generic ? 'This page does not exist' : message,
			hint: 'The link may be mistyped, or what it pointed to was removed by your retention setting.',
			detail: '',
			retry: false
		};
	}
	if (OUTDATED.test(message)) {
		return {
			...base,
			kind: 'outdated',
			title: 'The dashboard was updated',
			hint: 'GitPrompt changed since this page opened, and part of it is no longer there. Reload to get the new version.',
			retry: false,
			reload: true
		};
	}
	// A TypeError from fetch, or its message once SvelteKit turned it into `page.error`.
	const network = error instanceof TypeError || !(error instanceof Error);
	if (network && OFFLINE.test(message)) {
		return {
			...base,
			kind: 'offline',
			title: 'GitPrompt is not running',
			hint: 'The dashboard can’t reach its daemon. Start it again with just serve, then try again.'
		};
	}
	if (code !== null && code >= 500) {
		return {
			...base,
			kind: 'daemon',
			title: what ? `The daemon could not load ${what}` : 'The daemon could not answer',
			hint: 'If this appeared after an update, the daemon may be running older code than the dashboard: restart it with just serve.'
		};
	}
	return {
		...base,
		kind: 'unexpected',
		title: what ? `Couldn’t show ${what}` : 'Something went wrong on this page',
		hint: 'The rest of the dashboard still works. If trying again doesn’t help, the details are also in the browser console.'
	};
}
