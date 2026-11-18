/**
 * A notification the daemon decided on (`event: notification` on the live stream): checked
 * before use, since the stream is text. The daemon decides what deserves one
 * (`gitprompt-insights/src/notify`); the dashboard only shows it.
 */
import type { AttentionTarget, Notification, NotificationKind } from '#lib/api/types.ts';

const KINDS: readonly NotificationKind[] = ['long_turn', 'possible_leak', 'quiet_connector'];

const isRecord = (value: unknown): value is Record<string, unknown> =>
	typeof value === 'object' && value !== null;

const isKind = (value: unknown): value is NotificationKind => KINDS.some((kind) => kind === value);

/** A target the dashboard can link to; a page it does not know leads to the Overview. */
function target(value: unknown): AttentionTarget {
	if (!isRecord(value)) return { page: 'overview' };
	const { page, id, q } = value;
	if (page === 'prompt' && typeof id === 'number') return { page, id };
	if ((page === 'session' || page === 'source') && typeof id === 'string') return { page, id };
	if (page === 'prompts' && typeof q === 'string') return { page, q };
	if (page === 'leaks') return { page };
	return { page: 'overview' };
}

/** The notification in an event's `data`, or null when it is not one. */
export function parseNotification(data: string): Notification | null {
	let value: unknown;
	try {
		value = JSON.parse(data);
	} catch {
		return null;
	}
	if (!isRecord(value)) return null;
	const { kind, title, body } = value;
	if (!isKind(kind) || typeof title !== 'string' || typeof body !== 'string') return null;
	return { kind, title, body, target: target(value.target) };
}
