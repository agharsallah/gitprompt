/**
 * A notification from the daemon, shown as a toast while the dashboard is open: its title, its
 * body, and a link to where it points. The daemon decided it deserves one; nothing is filtered here.
 */
import type { Notification } from '#lib/api/types.ts';
import { targetHref } from '#lib/shared/links.ts';
import type { Toasts } from '#lib/ui/toast.svelte.ts';

/** A notification stays longer than a confirmation: it was not caused by a click. */
export const NOTIFICATION_MS = 10_000;

export function showNotification(toasts: Toasts, notification: Notification) {
	toasts.show(notification.title, {
		detail: notification.body,
		href: targetHref(notification.target),
		ms: NOTIFICATION_MS
	});
}
