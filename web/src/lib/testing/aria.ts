/**
 * Structural accessibility checks over rendered HTML, for components tested with svelte/server (no
 * DOM, so no axe): the rules a markup mistake breaks and a screen reader hits first.
 */

const REFERENCES = [
	'aria-controls',
	'aria-activedescendant',
	'aria-labelledby',
	'aria-describedby'
];

/** Every id in the markup. */
export const idsIn = (html: string): Set<string> =>
	new Set([...html.matchAll(/\bid="([^"]*)"/g)].map((m) => m[1]));

/** The ids an `aria-*` reference names that no element has. */
export function danglingReferences(html: string): string[] {
	const ids = idsIn(html);
	const missing: string[] = [];
	for (const attribute of REFERENCES) {
		for (const match of html.matchAll(new RegExp(`${attribute}="([^"]*)"`, 'g'))) {
			for (const id of match[1].split(/\s+/).filter(Boolean)) {
				if (!ids.has(id)) missing.push(`${attribute} → ${id}`);
			}
		}
	}
	return missing;
}

/** An opening tag, whose quoted attribute values may hold a `>`. */
const OPENING_TAG = /<[a-z][\w-]*(?:\s+[^\s=>"]+(?:="[^"]*")?)*\s*\/?>/g;

/** The opening tags of the elements with this role. */
export const tagsWithRole = (html: string, role: string): string[] =>
	[...html.matchAll(OPENING_TAG)].map((m) => m[0]).filter((tag) => tag.includes(` role="${role}"`));

/** Two elements with the same id (a reference then reaches only the first). */
export function duplicateIds(html: string): string[] {
	const seen = new Set<string>();
	const twice = new Set<string>();
	for (const m of html.matchAll(/\bid="([^"]*)"/g)) {
		if (seen.has(m[1])) twice.add(m[1]);
		seen.add(m[1]);
	}
	return [...twice];
}
