<script lang="ts">
	import type { Doorway } from '#lib/insights/filters.ts';
	import type { Overview } from '#lib/insights/overview.ts';
	import { ratio, repeated } from '#lib/insights/series.ts';
	import { excerpt, percent, plural } from '#lib/shared/format.ts';
	import BarList from '#lib/ui/charts/BarList.svelte';
	import { countPanel } from '#lib/insights/counting.ts';
	import Panel from '#lib/ui/Panel.svelte';
	import { readyOr } from '#lib/ui/panelState.ts';

	/** How often you send a text you sent before (case and spacing aside), and which ones. */
	let { data, link }: { data: Overview; link: Doorway } = $props();

	const t = $derived(data.activity.totals);
	const again = $derived(repeated(t));
	const reused = $derived(data.texts.groups.filter((g) => g.prompts > 1 && g.detail));
	// The first words find the prompt (search matches every word anywhere in the typed text).
	const words = (text: string) => text.split(/\s+/).filter(Boolean).slice(0, 8).join(' ');
	const view = $derived(
		readyOr(reused.length, {
			kind: 'empty',
			title: 'Every prompt was new',
			message: 'No typed text was sent twice in this window.'
		})
	);
</script>

<Panel
	{@attach countPanel('reuse')}
	state={view}
	title="Which prompts do I send again?"
	description={t.prompts
		? `${plural(again, 'prompt')} (${percent(ratio(again, t.prompts))}) repeated a text sent before.`
		: undefined}
>
	<BarList
		label="Prompts sent more than once"
		max={6}
		items={reused.map((g) => ({
			key: g.key ?? '',
			label: excerpt(g.detail, 80),
			value: g.prompts,
			hint: plural(g.sessions, 'session'),
			href: link({ q: words(g.detail ?? '') })
		}))}
		format={(v) => `${v}×`}
	/>
</Panel>
