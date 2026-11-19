import { render } from 'svelte/server';
import { describe, expect, it, vi } from 'vitest';
import { PreferenceEditor } from '#lib/settings/editor.svelte.ts';
import PrivateTermsRow from '#lib/settings/PrivateTermsRow.svelte';
import { DEFAULTS } from '#lib/settings/preferences.ts';
import { settingsState } from '#lib/testing/builders.ts';

vi.mock('$app/navigation', () => ({ refreshAll: () => Promise.resolve() }));

const row = (private_terms: string[], locked = false) => {
	const prefs = settingsState({ ...DEFAULTS, private_terms });
	prefs.settings.private_terms.locked = locked;
	return render(PrivateTermsRow, { props: { prefs, editor: new PreferenceEditor(() => prefs) } })
		.body;
};

describe('your own private words, in Settings', () => {
	it('says what they are and that a change checks the stored prompts again', () => {
		const out = row([]);
		expect(out).toContain('Your own private words');
		expect(out).toContain('as whole words in any case');
		expect(out).toContain('checks every stored prompt again');
	});

	it('edits the stored list as one comma-separated field', () => {
		expect(row(['Project Falcon', 'acme-internal'])).toContain(
			'value="Project Falcon, acme-internal"'
		);
	});

	it('shows the list as text when an environment variable sets it', () => {
		const out = row(['Project Falcon'], true);
		expect(out).toContain('Project Falcon');
		expect(out).not.toContain('<input');
	});
});
