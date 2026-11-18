import { render } from 'svelte/server';
import { describe, expect, it, vi } from 'vitest';
import { PreferenceEditor } from '#lib/settings/editor.svelte.ts';
import LeakAllowRow from '#lib/settings/LeakAllowRow.svelte';
import { DEFAULTS } from '#lib/settings/preferences.ts';
import { settingsState } from '#lib/testing/builders.ts';

vi.mock('$app/navigation', () => ({ refreshAll: () => Promise.resolve() }));

const row = (leak_allow: string[], locked = false) => {
	const prefs = settingsState({ ...DEFAULTS, leak_allow });
	prefs.settings.leak_allow.locked = locked;
	return render(LeakAllowRow, { props: { prefs, editor: new PreferenceEditor(() => prefs) } }).body;
};

describe('the values never flagged as a leak, in Settings', () => {
	it('says what they are and that your git email is never flagged', () => {
		const out = row([]);
		expect(out).toContain('Never a leak');
		expect(out).toContain('your other addresses, a shared test account');
		expect(out).toContain('Your git email is never flagged.');
	});

	it('edits the stored list as one comma-separated field', () => {
		expect(row(['me@work.example', 'test@example.com'])).toContain(
			'value="me@work.example, test@example.com"'
		);
	});

	it('shows the list as text when an environment variable sets it', () => {
		const out = row(['me@work.example'], true);
		expect(out).toContain('me@work.example');
		expect(out).not.toContain('<input');
	});
});
