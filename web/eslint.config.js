import { fileURLToPath } from 'node:url';
import { includeIgnoreFile } from '@eslint/compat';
import comments from '@eslint-community/eslint-plugin-eslint-comments/configs';
import js from '@eslint/js';
import prettier from 'eslint-config-prettier';
import sonarjs from 'eslint-plugin-sonarjs';
import svelte from 'eslint-plugin-svelte';
import { defineConfig } from 'eslint/config';
import globals from 'globals';
import ts from 'typescript-eslint';

// Formatting belongs to Prettier (`pnpm format`); ESLint only reports code problems.
const gitignore = fileURLToPath(new URL('./.gitignore', import.meta.url));

export default defineConfig(
	includeIgnoreFile(gitignore),
	{ ignores: ['coverage/', 'static/', 'src/lib/api/generated.ts'] },
	js.configs.recommended,
	ts.configs.recommendedTypeChecked,
	svelte.configs.recommended,
	prettier,
	svelte.configs.prettier,
	comments.recommended,
	{
		// No escape hatches: a finding is fixed in the code, never silenced in place. A rule that is
		// wrong for this project is turned off here, once, with the reason.
		linterOptions: { reportUnusedDisableDirectives: 'error' },
		rules: {
			'@eslint-community/eslint-comments/no-use': ['error', { allow: [] }],
			// `<!-- eslint-disable -->` in markup is honoured by this rule alone: off, so it silences nothing.
			'svelte/comment-directive': 'off',
			'@typescript-eslint/no-explicit-any': 'error',
			'@typescript-eslint/ban-ts-comment': [
				'error',
				{ 'ts-expect-error': true, 'ts-ignore': true, 'ts-nocheck': true, 'ts-check': false }
			]
		}
	},
	{
		// Duplication, compared on the syntax tree: a copied function body, branches that do the same
		// thing, a condition or operand written twice. Unlike a token copy-paste scan, renamed or
		// reformatted copies still match and boilerplate (imports, headers) never does.
		// no-duplicate-string stays off: repeated labels and class names are not duplicated logic.
		plugins: { sonarjs },
		rules: {
			'sonarjs/no-identical-functions': 'error',
			'sonarjs/no-duplicated-branches': 'error',
			'sonarjs/no-all-duplicated-branches': 'error',
			'sonarjs/no-identical-conditions': 'error',
			'sonarjs/no-identical-expressions': 'error'
		}
	},
	{
		// Complexity, the same limits as the backend (clippy.toml): a function over one does more than one
		// thing, so split it by step or concept. Cognitive complexity weighs nesting and broken flow
		// (sonarjs); cyclomatic counts paths. File length in non-blank, non-comment lines.
		rules: {
			'sonarjs/cognitive-complexity': ['error', 15],
			complexity: ['error', 15],
			'max-depth': ['error', 4],
			'max-params': ['error', 5],
			'max-nested-callbacks': ['error', 4],
			'max-lines-per-function': ['error', { max: 80, skipBlankLines: true, skipComments: true }],
			'max-lines': ['error', { max: 400, skipBlankLines: true, skipComments: true }]
		}
	},
	{
		// A test file is one `describe` callback holding every case: its length is the file's, which
		// max-lines already limits.
		files: ['**/*.test.ts', '**/*.svelte.test.ts'],
		rules: { 'max-lines-per-function': 'off' }
	},
	{
		languageOptions: {
			globals: { ...globals.browser, ...globals.node },
			parserOptions: {
				projectService: true,
				tsconfigRootDir: import.meta.dirname,
				extraFileExtensions: ['.svelte']
			}
		},
		rules: {
			// TypeScript's compiler already reports undefined names, with full type knowledge.
			'no-undef': 'off',
			eqeqeq: ['error', 'smart'],
			'no-console': ['warn', { allow: ['warn', 'error'] }],
			'@typescript-eslint/consistent-type-imports': ['error', { fixStyle: 'inline-type-imports' }],
			'@typescript-eslint/no-unused-vars': [
				'error',
				{ argsIgnorePattern: '^_', varsIgnorePattern: '^_', caughtErrorsIgnorePattern: '^_' }
			],
			'@typescript-eslint/restrict-template-expressions': ['error', { allowNumber: true }],
			'svelte/no-at-html-tags': 'error',
			'svelte/require-each-key': 'error',
			'svelte/button-has-type': 'error',
			// Rune-aware: `let { a } = $props()` stays `let`, as the Svelte docs write it.
			'prefer-const': 'off',
			'svelte/prefer-const': 'error',
			// Typed linting cannot see types through `.svelte` imports or generic snippet parameters, so
			// these misfire there; svelte-check (`pnpm check`) owns type safety.
			'@typescript-eslint/no-unsafe-argument': 'off',
			'@typescript-eslint/no-unsafe-assignment': 'off',
			'@typescript-eslint/no-unsafe-call': 'off',
			'@typescript-eslint/no-unsafe-member-access': 'off',
			'@typescript-eslint/no-unsafe-return': 'off',
			'@typescript-eslint/no-unnecessary-type-assertion': 'off',
			// Flags every local Map/Set/URLSearchParams, even throwaway ones built inside a function or
			// $derived; state that must react is declared with $state and reviewed as such.
			'svelte/prefer-svelte-reactivity': 'off',
			// The app is served from the root (no `paths.base`), so resolve() would only add noise.
			'svelte/no-navigation-without-resolve': 'off'
		}
	},
	{
		files: ['**/*.svelte', '**/*.svelte.ts', '**/*.svelte.js'],
		languageOptions: {
			parserOptions: { parser: ts.parser }
		}
	},
	{
		// Plain JS config files are outside the TypeScript project: lint them without type info.
		files: ['**/*.js'],
		extends: [ts.configs.disableTypeChecked]
	}
);
