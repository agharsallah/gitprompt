import { describe, expect, it } from 'vitest';
import {
	describe as described,
	forgetAll,
	isRemembering,
	REMEMBER_KEY,
	rememberView,
	SCOPES,
	setRemembering,
	type Scope
} from '#lib/settings/remembered.ts';

function memoryStorage() {
	const items = new Map<string, string>();
	let writes = 0;
	return {
		items,
		writes: () => writes,
		getItem: (k: string) => items.get(k) ?? null,
		setItem: (k: string, v: string) => {
			writes++;
			items.set(k, v);
		},
		removeItem: (k: string) => {
			writes++;
			items.delete(k);
		}
	};
}

/** Load a page URL through the given scopes, as a page's load does. */
function visit(storage: ReturnType<typeof memoryStorage>, query: string, scopes: Scope[]) {
	return rememberView(new URLSearchParams(query), scopes, storage).toString();
}

describe('remembered filters', () => {
	it('restores the last choice when a page is opened without it', () => {
		const s = memoryStorage();
		visit(s, 'range=30d&model=claude-opus-5-5&source=', ['range', 'overview']);
		expect(visit(s, '', ['range', 'overview'])).toBe('range=30d&model=claude-opus-5-5');
	});

	it('shares the time range between pages, but not their own filters', () => {
		const s = memoryStorage();
		visit(s, 'range=24h&model=m', ['range', 'overview']);
		expect(visit(s, '', ['range', 'sessions'])).toBe('range=24h');
		visit(s, 'range=all&sort=longest', ['range', 'sessions']);
		expect(visit(s, '', ['range', 'overview'])).toBe('range=all&model=m');
	});

	it('a value in the URL wins over the remembered one and replaces it', () => {
		const s = memoryStorage();
		visit(s, 'range=30d', ['range']);
		expect(visit(s, 'range=24h', ['range'])).toBe('range=24h');
		expect(visit(s, '', ['range'])).toBe('range=24h');
	});

	it('an empty value clears a remembered filter (the "every …" choice)', () => {
		const s = memoryStorage();
		visit(s, 'model=m&repo=/r', ['overview']);
		expect(visit(s, 'model=&repo=/r', ['overview'])).toBe('model=&repo=%2Fr');
		expect(visit(s, '', ['overview'])).toBe('repo=%2Fr');
	});

	it('keeps parameters it does not remember untouched', () => {
		const s = memoryStorage();
		visit(s, 'project=p', ['sessions']);
		expect(visit(s, 'q=fix&limit=40', ['sessions'])).toBe('q=fix&limit=40&project=p');
	});

	it('writes only when something changed', () => {
		const s = memoryStorage();
		visit(s, 'range=30d&model=m', ['range', 'overview']);
		const after = s.writes();
		for (let i = 0; i < 20; i++) {
			visit(s, '', ['range', 'overview']);
			visit(s, 'range=30d&model=m', ['range', 'overview']);
		}
		expect(s.writes()).toBe(after);
	});

	it('ignores values it does not know and broken storage', () => {
		const s = memoryStorage();
		s.items.set(SCOPES.overview.key, '{"model": 3, "evil": "x", "repo": "/r"}');
		s.items.set(SCOPES.range.key, 'not json');
		expect(visit(s, '', ['range', 'overview'])).toBe('repo=%2Fr');
		const blocked = {
			getItem: () => {
				throw new Error('blocked');
			},
			setItem: () => {
				throw new Error('blocked');
			},
			removeItem: () => {
				throw new Error('blocked');
			}
		};
		expect(rememberView(new URLSearchParams('range=7d'), ['range'], blocked).toString()).toBe(
			'range=7d'
		);
	});

	it('can be turned off: nothing is restored or stored, and what was kept is forgotten', () => {
		const s = memoryStorage();
		visit(s, 'range=30d&project=p', ['range', 'sessions']);
		setRemembering(false, s);
		expect(isRemembering(s)).toBe(false);
		expect(s.items.get(REMEMBER_KEY)).toBe('off');
		expect(visit(s, '', ['range', 'sessions'])).toBe('');
		visit(s, 'range=24h', ['range']);
		expect([...s.items.keys()]).toEqual([REMEMBER_KEY]);
		setRemembering(true, s);
		expect(s.items.size).toBe(0);
		expect(isRemembering(s)).toBe(true);
	});

	it('forgets every scope at once', () => {
		const s = memoryStorage();
		visit(s, 'range=30d&model=m&sort=longest', ['range', 'overview', 'sessions']);
		forgetAll(s);
		expect(visit(s, '', ['range', 'overview', 'sessions'])).toBe('');
	});

	it('describes what is remembered the way the filters name it', () => {
		const s = memoryStorage();
		expect(described('overview', s)).toBe('Nothing remembered');
		visit(s, 'range=30d&model=claude-opus-5-5&repo=/w/app&sort=longest&project=/w/api', [
			'range',
			'overview',
			'sessions'
		]);
		expect(described('range', s)).toBe('30 days');
		expect(described('overview', s)).toBe('Project: app, Model: Opus 5.5');
		expect(described('sessions', s)).toBe('Project: api, Sort: Most turns');
	});

	it('names every key under the gitprompt- prefix, once', () => {
		const keys = [REMEMBER_KEY, ...Object.values(SCOPES).map((s) => s.key)];
		expect(new Set(keys).size).toBe(keys.length);
		expect(keys.every((k) => k.startsWith('gitprompt-'))).toBe(true);
	});
});
