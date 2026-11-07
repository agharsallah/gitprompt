// Where a prompt was typed (the daemon's `facts.surface`, insights/surface.py): the spec names an
// icon, this draws it. Unknown icon names fall back to a generic app window. Pure, tested in
// surface.test.ts.
import {
	AppWindow,
	Braces,
	Code,
	Globe,
	MessageSquare,
	MousePointer2,
	SquareTerminal,
	type LucideIcon
} from '@lucide/svelte';
import type { Connector, Surface } from '#lib/api/types.ts';

const ICONS: Record<string, LucideIcon> = {
	terminal: SquareTerminal,
	code: Code,
	'app-window': AppWindow,
	'mouse-pointer': MousePointer2,
	braces: Braces,
	'message-square': MessageSquare,
	globe: Globe
};

export function surfaceIcon(surface: Pick<Surface, 'icon'>): LucideIcon {
	return ICONS[surface.icon] ?? AppWindow;
}

/** The tooltip of a prompt's origin badge: who sent it, and where it was typed when known. */
export function sentBy(connector: Connector, surface?: Surface | null): string {
	return surface
		? `Sent by ${connector.name}, typed in ${surfaceIn(surface)}`
		: `Sent by ${connector.name}`;
}

/** "the Desktop app", "VS Code", "the CLI": a surface's name inside a sentence. */
export function surfaceIn(surface: Pick<Surface, 'id' | 'name'>): string {
	return /app$|^CLI$|^SDK$/.test(surface.name) ? `the ${surface.name}` : surface.name;
}
