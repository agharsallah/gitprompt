import { afterEach, describe, expect, it } from 'vitest';
import { commands, matchCommands, registerCommand, type Command } from '#lib/palette/commands.ts';

const made: (() => void)[] = [];
const add = (id: string, title: string, keywords: string[] = []): Command => {
	const command = { id, title, keywords, run: () => id };
	made.push(registerCommand(command));
	return command;
};
afterEach(() => {
	for (const remove of made.splice(0)) remove();
});

describe('the command registry', () => {
	it('lists what features registered, in order', () => {
		add('t.a', 'Alpha');
		add('t.b', 'Beta');
		expect(commands().map((c) => c.id)).toEqual(['t.a', 't.b']);
	});

	it('replaces a command registered again under the same id', () => {
		add('t.a', 'Alpha');
		add('t.a', 'Alpha two');
		expect(commands().map((c) => c.title)).toEqual(['Alpha two']);
	});

	it('takes a command back, and only the one that was registered', () => {
		const remove = registerCommand({ id: 't.x', title: 'X', keywords: [], run: () => 0 });
		remove();
		expect(commands()).toEqual([]);
		const stale = registerCommand({ id: 't.x', title: 'Old', keywords: [], run: () => 0 });
		add('t.x', 'New');
		stale();
		expect(commands().map((c) => c.title)).toEqual(['New']);
	});
});

describe('finding a command', () => {
	it('lists them all for no words', () => {
		const all = [add('t.a', 'Alpha'), add('t.b', 'Beta')];
		expect(matchCommands('').map((m) => m.command)).toEqual(all);
	});

	it('needs every word, in the title or the keywords, in any case', () => {
		add('t.theme', 'Toggle theme', ['dark', 'color']);
		add('t.export', 'Export this view', ['download']);
		expect(matchCommands('DARK toggle').map((m) => m.command.id)).toEqual(['t.theme']);
		expect(matchCommands('dark export')).toEqual([]);
		expect(matchCommands('download').map((m) => m.command.id)).toEqual(['t.export']);
	});

	it('puts a title that starts with the words first, then a title that has them, then keywords', () => {
		add('t.kw', 'Zebra', ['export']);
		add('t.in', 'Quick export', []);
		add('t.start', 'Export this view');
		expect(matchCommands('export').map((m) => m.command.id)).toEqual(['t.start', 't.in', 't.kw']);
	});

	it('marks the matched words in the title', () => {
		add('t.export', 'Export this view');
		expect(matchCommands('exp')[0].title).toEqual([
			{ text: 'Exp', hit: true },
			{ text: 'ort this view', hit: false }
		]);
	});
});
