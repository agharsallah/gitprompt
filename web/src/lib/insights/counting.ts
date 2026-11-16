/**
 * What the Overview counts (lib/shared/usage.ts), wired here so the shared Panel stays generic:
 * `{@attach countPanel(name)}` marks a panel and counts it once it stays in view, and
 * `use:clickThrough` on the page counts a click on a link to the prompts under the panel it is in.
 */
import type { Action } from 'svelte/action';
import type { Attachment } from 'svelte/attachments';
import { pageHref } from '#lib/shared/links.ts';
import { seen } from '#lib/shared/seen.ts';
import { track } from '#lib/shared/usage.ts';
import {
	PANELS,
	panelAction,
	type PanelName,
	type UsageAction
} from '#lib/shared/usage-actions.ts';

/** Marks the element as the Overview panel `name` and counts it as seen. */
export const countPanel =
	(name: PanelName): Attachment<HTMLElement> =>
	(node) => {
		node.dataset.panel = name;
		const watch = seen(node, panelAction('seen', name));
		return () => watch.destroy();
	};

const pathOf = (href: string) => new URL(href, 'http://localhost').pathname.replace(/\/$/, '');

/**
 * What a click on a link to `href`, inside the panel named `panel`, counts (if anything). The
 * prompts' path is the one links.ts builds, so it holds under a base path too.
 */
export function clickAction(
	href: string,
	panel: string | undefined,
	promptsPath: string = pathOf(pageHref('/prompts'))
): UsageAction | undefined {
	const known = PANELS.find((p) => p === panel);
	return known && pathOf(href) === promptsPath ? panelAction('click', known) : undefined;
}

export const clickThrough: Action<HTMLElement> = (node) => {
	const onclick = (event: MouseEvent) => {
		const link = event.target instanceof Element ? event.target.closest('a[href]') : null;
		if (!link) return;
		const panel = link.closest<HTMLElement>('[data-panel]')?.dataset.panel;
		const action = clickAction(link.getAttribute('href') ?? '', panel);
		if (action) track(action);
	};
	node.addEventListener('click', onclick);
	return { destroy: () => node.removeEventListener('click', onclick) };
};
