import { describe, expect, it } from 'vitest';
import { ApiError } from '#lib/api/client.ts';
import { cleaned, Rename, type RenameOptions } from '#lib/ui/inlineRename.svelte.ts';

/** A rename over a value the "owner" holds, whose saves the test settles itself. */
function setup(overrides: Partial<RenameOptions> = {}, start = 'Old name') {
	const owner = { value: start };
	const saved: (string | null)[] = [];
	let settle: { resolve: (stored?: string | null) => void; reject: (e: unknown) => void } = {
		resolve: () => {},
		reject: () => {}
	};
	const rename = new Rename({
		value: () => owner.value,
		fallback: () => 'Default label',
		save: (next) => {
			saved.push(next);
			return new Promise((resolve, reject) => (settle = { resolve, reject }));
		},
		what: () => 'the name',
		...overrides
	});
	return { rename, owner, saved, settle: () => settle };
}

const type = (rename: Rename, text: string) => {
	rename.start();
	rename.draft = text;
};

describe('cleaned', () => {
	it('trims and collapses white space', () => expect(cleaned('  a \n b\t c ')).toBe('a b c'));
});

describe('starting and cancelling', () => {
	it('starts with the shown text in the field', () => {
		const { rename } = setup();
		rename.start();
		expect([rename.editing, rename.draft]).toEqual([true, 'Old name']);
	});

	it('drops what was typed on cancel and saves nothing', async () => {
		const { rename, saved } = setup();
		type(rename, 'Something else');
		rename.cancel();
		await rename.commit();
		expect([rename.editing, rename.shown, saved]).toEqual([false, 'Old name', []]);
	});

	it('does not start while a save runs', async () => {
		const { rename, settle } = setup();
		type(rename, 'New');
		const saving = rename.commit();
		rename.start();
		expect(rename.editing).toBe(false);
		settle().resolve();
		await saving;
	});
});

describe('saving', () => {
	it('shows the new text at once, before the daemon answers', async () => {
		const { rename, saved, settle } = setup();
		type(rename, '  New   name ');
		const saving = rename.commit();
		expect([rename.editing, rename.saving, rename.shown, saved]).toEqual([
			false,
			true,
			'New name',
			['New name']
		]);
		settle().resolve();
		await saving;
		expect([rename.saving, rename.shown, rename.error]).toEqual([false, 'New name', '']);
	});

	it('keeps the text as the daemon stored it', async () => {
		const { rename, settle } = setup();
		type(rename, 'new   name');
		const saving = rename.commit();
		settle().resolve('New name');
		await saving;
		expect(rename.shown).toBe('New name');
	});

	it('goes back to the old text, with the reason, when the save fails', async () => {
		const { rename, settle } = setup();
		type(rename, 'New');
		const saving = rename.commit();
		expect(rename.shown).toBe('New');
		settle().reject(new ApiError(422, '/api/names/session/s1', 'name: at most 80 characters'));
		await saving;
		expect([rename.saving, rename.shown]).toEqual([false, 'Old name']);
		expect(rename.error).toBe(
			'Could not save the name: 422 /api/names/session/s1: name: at most 80 characters'
		);
	});

	it('rolls back to the earlier saved text, not to the owner’s', async () => {
		const { rename, settle } = setup();
		type(rename, 'First');
		const first = rename.commit();
		settle().resolve();
		await first;
		type(rename, 'Second');
		const second = rename.commit();
		settle().reject(new Error('down'));
		await second;
		expect(rename.shown).toBe('First');
	});

	it('clears the error when editing starts again', async () => {
		const { rename, settle } = setup();
		type(rename, 'New');
		const saving = rename.commit();
		settle().reject(new Error('down'));
		await saving;
		rename.start();
		expect(rename.error).toBe('');
	});

	it('saves nothing when the text did not change', async () => {
		const { rename, saved } = setup();
		type(rename, '  Old   name ');
		await rename.commit();
		expect([rename.editing, saved]).toEqual([false, []]);
	});

	it('commits once: a second commit after the field closed does nothing', async () => {
		const { rename, saved, settle } = setup();
		type(rename, 'New');
		const saving = rename.commit();
		await rename.commit();
		settle().resolve();
		await saving;
		expect(saved).toEqual(['New']);
	});
});

describe('emptying the text', () => {
	it('goes back to the default and asks the daemon to forget the name', async () => {
		const { rename, saved, settle } = setup();
		type(rename, '   ');
		const saving = rename.commit();
		expect([rename.shown, saved]).toEqual(['Default label', [null]]);
		settle().resolve(null);
		await saving;
		expect(rename.shown).toBe('Default label');
	});

	it('cancels when there is no default to go back to', async () => {
		const { rename, saved } = setup({ fallback: undefined });
		type(rename, '');
		await rename.commit();
		expect([rename.shown, saved]).toEqual(['Old name', []]);
	});

	it('saves nothing when it already shows the default', async () => {
		const { rename, saved } = setup({}, 'Default label');
		type(rename, '');
		await rename.commit();
		expect(saved).toEqual([]);
	});
});

describe('the owner’s value', () => {
	it('shows what the owner shows until something is saved', () => {
		const { rename, owner } = setup();
		owner.value = 'Changed elsewhere';
		expect(rename.shown).toBe('Changed elsewhere');
	});

	it('takes over again once the owner has a new value', async () => {
		const { rename, owner, settle } = setup();
		type(rename, 'New');
		const saving = rename.commit();
		settle().resolve();
		await saving;
		owner.value = 'New';
		rename.settle();
		owner.value = 'Renamed on another tab';
		expect(rename.shown).toBe('Renamed on another tab');
	});

	it('does not drop the optimistic text while a save runs', async () => {
		const { rename, owner, settle } = setup();
		type(rename, 'New');
		const saving = rename.commit();
		owner.value = 'Refreshed';
		rename.settle();
		expect(rename.shown).toBe('New');
		settle().resolve();
		await saving;
	});
});
