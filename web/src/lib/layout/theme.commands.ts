// "Toggle theme": the layout's own command for the color theme.
import { nextTheme, theme } from '#lib/layout/theme.svelte.ts';
import { registerCommand } from '#lib/palette/commands.ts';

registerCommand({
	id: 'theme.toggle',
	title: 'Toggle theme',
	keywords: ['dark', 'light', 'color', 'appearance'],
	run: () => {
		const dark = matchMedia('(prefers-color-scheme: dark)').matches;
		theme.value = nextTheme(theme.value, dark);
	}
});
