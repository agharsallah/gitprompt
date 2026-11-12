import { describe, expect, it } from 'vitest';
import { modelName, parseModelId, SCHEMES } from '#lib/shared/model-name.ts';

// Models and versions that don't exist yet: the name comes from the id, never from a list.
// Each row: the id, its name, and whether the id carries a snapshot date.
const FAMILIES = ['opus', 'sonnet', 'haiku', 'fable', 'nova'];
const VERSIONS: [suffix: string, shown: string, dated: boolean][] = [
	['6', '6', false],
	['6-1', '6.1', false],
	['7-12', '7.12', false],
	['10', '10', false],
	['10-3-20271231', '10.3', true]
];
const capital = (word: string) => word.charAt(0).toUpperCase() + word.slice(1);
const FUTURE: [id: string, name: string, dated: boolean][] = FAMILIES.flatMap((family) =>
	VERSIONS.map(([suffix, shown, dated]): [string, string, boolean] => [
		`claude-${family}-${suffix}`,
		`${capital(family)} ${shown}`,
		dated
	])
);

describe('modelName', () => {
	describe('Claude', () => {
		it.each([
			['claude-opus-5-5', 'Opus 5.5'],
			['claude-haiku-5-5', 'Haiku 5.5'],
			['claude-sonnet-4-5-20250929', 'Sonnet 4.5'],
			['claude-opus-4-20250514', 'Opus 4'],
			['claude-sonnet-5', 'Sonnet 5'],
			['Claude-Opus-5-5', 'Opus 5.5'],
			['  claude-opus-5-5  ', 'Opus 5.5'],
			// A family without a version, and no family at all.
			['claude-haiku', 'Haiku'],
			['claude', 'Claude'],
			// Variants follow the version.
			['claude-sonnet-4-5-thinking', 'Sonnet 4.5 Thinking'],
			['claude-3-7-sonnet-latest', 'Sonnet 3.7']
		])('%s -> %s', (id, name) => expect(modelName(id)).toBe(name));

		it.each([
			['claude-3-5-sonnet-20241022', 'Sonnet 3.5'],
			['claude-3-opus-20240229', 'Opus 3'],
			['claude-3-5-haiku-latest', 'Haiku 3.5'],
			['claude-2', 'Claude 2'],
			['claude-2.1', 'Claude 2.1'],
			['claude-instant-1-2', 'Instant 1.2']
		])('reads the older version-first id %s -> %s', (id, name) => expect(modelName(id)).toBe(name));

		it.each([
			// Copilot and Cursor write the version with a dot, and Cursor puts it first.
			['claude-sonnet-4.5', 'Sonnet 4.5'],
			['claude-haiku-4.5', 'Haiku 4.5'],
			['claude-4-sonnet', 'Sonnet 4'],
			['claude-4.5-sonnet-thinking', 'Sonnet 4.5 Thinking'],
			// Claude Code tags the context window.
			['claude-opus-5-5[1m]', 'Opus 5.5'],
			// Bedrock: region, provider and revision.
			['us.anthropic.claude-opus-5-5-v1:0', 'Opus 5.5'],
			['global.anthropic.claude-sonnet-4-5-20250929-v1:0', 'Sonnet 4.5'],
			['anthropic.claude-sonnet-4-5-20250929-v1:0', 'Sonnet 4.5'],
			['anthropic.claude-3-5-sonnet-20241022-v2:0', 'Sonnet 3.5'],
			['anthropic.claude-v2:1', 'Claude 2'],
			['anthropic.claude-instant-v1', 'Instant 1'],
			// Vertex.
			['claude-opus-4-1@20250805', 'Opus 4.1'],
			['claude-3-5-sonnet-v2@20241022', 'Sonnet 3.5'],
			// Routers.
			['anthropic/claude-sonnet-4.5', 'Sonnet 4.5'],
			['openrouter/anthropic/claude-opus-5-5', 'Opus 5.5']
		])('unwraps %s -> %s', (id, name) => expect(modelName(id)).toBe(name));

		it.each(FUTURE)('%s names its own family and version', (id, name) =>
			expect(modelName(id)).toBe(name)
		);

		it('never names two different models the same', () => {
			const ids = FUTURE.filter(([, , dated]) => !dated).map(([id]) => id);
			expect(new Set(ids.map(modelName)).size).toBe(ids.length);
		});
	});

	describe('GPT', () => {
		it.each([
			['gpt-5', 'GPT-5'],
			['gpt-6', 'GPT-6'],
			['gpt-6-luna', 'GPT-6 Luna'],
			['gpt-6-mini', 'GPT-6 Mini'],
			['gpt-5-codex', 'GPT-5 Codex'],
			['gpt-5.1-codex-mini', 'GPT-5.1 Codex Mini'],
			['gpt-4o', 'GPT-4o'],
			['gpt-4o-mini', 'GPT-4o Mini'],
			['gpt-4.1-mini', 'GPT-4.1 Mini'],
			// Snapshots: ISO dates, MMDD, "latest".
			['gpt-4o-2024-08-06', 'GPT-4o'],
			['gpt-4o-mini-2024-07-18', 'GPT-4o Mini'],
			['gpt-5-2025-08-07', 'GPT-5'],
			['gpt-4-0613', 'GPT-4'],
			['gpt-3.5-turbo-0125', 'GPT-3.5 Turbo'],
			// A size is a word, not a version part or a date.
			['gpt-4-32k', 'GPT-4 32k'],
			['gpt-3.5-turbo-16k', 'GPT-3.5 Turbo 16k'],
			['openai/gpt-5', 'GPT-5']
		])('%s -> %s', (id, name) => expect(modelName(id)).toBe(name));

		it.each(['gpt-oss-120b', 'gpt-image-1', 'gpt-a', 'gpt'])(
			'passes %s through: no version straight after gpt',
			(id) => expect(modelName(id)).toBe(id)
		);
	});

	describe('Gemini', () => {
		it.each([
			['gemini-2.5-pro', 'Gemini 2.5 Pro'],
			['gemini-2.5-flash-lite', 'Gemini 2.5 Flash Lite'],
			['gemini-3-pro-preview', 'Gemini 3 Pro Preview'],
			['gemini-1.5-pro-002', 'Gemini 1.5 Pro'],
			['gemini-2.5-flash-preview-05-20', 'Gemini 2.5 Flash Preview'],
			['models/gemini-2.5-pro', 'Gemini 2.5 Pro'],
			['google/gemini-2.5-flash', 'Gemini 2.5 Flash']
		])('%s -> %s', (id, name) => expect(modelName(id)).toBe(name));

		it('passes an id without a leading version through', () =>
			expect(modelName('gemini-exp-1206')).toBe('gemini-exp-1206'));
	});

	describe('anything else passes through unchanged', () => {
		it.each([
			'o3',
			'o4-mini',
			'opus',
			'sonnet[1m]',
			'some-local-model',
			'llama3.1:8b',
			'mlx-community/Qwen3-8B',
			'meta.llama3-70b-instruct-v1:0',
			'Claude Opus 5.5',
			// Tokens a scheme can't read.
			'claude-opus_5',
			'claude-opus-5-5!',
			// Not vendors, even though plain objects have these keys.
			'constructor-1',
			'toString',
			'__proto__-2',
			'',
			'-'
		])('%j', (id) => expect(modelName(id)).toBe(id));
	});
});

describe('parseModelId', () => {
	it('reads vendor, version, its position and words', () => {
		expect(parseModelId('claude-3-5-sonnet-20241022')).toEqual({
			vendor: 'claude',
			version: '3.5',
			versionFirst: true,
			words: ['sonnet']
		});
		expect(parseModelId('claude-sonnet-4-5-thinking')).toEqual({
			vendor: 'claude',
			version: '4.5',
			versionFirst: false,
			words: ['sonnet', 'thinking']
		});
	});

	it('reads a Bedrock revision as the version only when there is no other', () => {
		expect(parseModelId('claude-v2')).toMatchObject({ version: '2', versionFirst: true });
		expect(parseModelId('claude-instant-v1')).toMatchObject({ version: '1', versionFirst: false });
		expect(parseModelId('claude-opus-5-v3')).toMatchObject({ version: '5', words: ['opus'] });
	});

	it('takes the first run of numbers as the version and drops later ones', () =>
		expect(parseModelId('claude-3-opus-4')).toMatchObject({ version: '3', words: ['opus'] }));

	it('has no version, and so no version position, when the id has no version', () =>
		expect(parseModelId('claude-haiku')).toEqual({
			vendor: 'claude',
			version: '',
			versionFirst: false,
			words: ['haiku']
		}));

	it('ends a version at a lettered piece and keeps words on both sides', () =>
		expect(parseModelId('gpt-4o-mini-2024-07-18')).toEqual({
			vendor: 'gpt',
			version: '4o',
			versionFirst: true,
			words: ['mini']
		}));

	it('drops a date before the version without moving it off the front', () =>
		expect(parseModelId('gemini-0613-2.5-pro')).toMatchObject({
			version: '2.5',
			versionFirst: true,
			words: ['pro']
		}));

	it.each(['claude-opus-5.x', 'claude-3-opus-4.5', 'claude-opus_5-4'])(
		'is null when a token is unreadable: %s',
		(id) => expect(parseModelId(id)).toBeNull()
	);
});

describe('SCHEMES', () => {
	it('has one scheme per vendor', () =>
		expect([...SCHEMES.keys()]).toEqual(['claude', 'gpt', 'gemini']));
});
