/** An in-memory localStorage for tests (`vi.stubGlobal('localStorage', memoryStorage())`). */
export function memoryStorage(entries: Record<string, string> = {}) {
	const items = new Map(Object.entries(entries));
	let writes = 0;
	return {
		items,
		writes: () => writes,
		getItem: (key: string) => items.get(key) ?? null,
		setItem: (key: string, value: string) => {
			writes++;
			items.set(key, value);
		},
		removeItem: (key: string) => {
			writes++;
			items.delete(key);
		}
	};
}

/** A localStorage that throws on every call, like a private window that blocks it. */
export function blockedStorage() {
	const blocked = () => {
		throw new Error('storage blocked');
	};
	return { getItem: blocked, setItem: blocked, removeItem: blocked };
}
