<script lang="ts">
	import Panel from '#lib/ui/Panel.svelte';
	import Collects from '#lib/sources/Collects.svelte';
	import ConsentReview from '#lib/sources/ConsentReview.svelte';
	import { NOW, SPEC_PROBLEM_FIXTURES, STATE_FIXTURES } from '#lib/sources/fixtures.ts';
	import Heartbeat from '#lib/sources/Heartbeat.svelte';
	import SpecProblems from '#lib/sources/SpecProblems.svelte';
	import { STATE_IDS, STATES } from '#lib/sources/state.ts';
	import StateRow from '#lib/sources/StateRow.svelte';

	/** Every state a connector can be in, and the pieces of a connector's page, from the fixtures. */

	const SAMPLE_CHANGE = {
		file: '/Users/me/.claude/settings.json',
		diff: '--- ~/.claude/settings.json\n+++ ~/.claude/settings.json\n@@ -1,3 +1,8 @@\n {\n-  "theme": "dark"\n+  "theme": "dark",\n+  "hooks": {\n+    "UserPromptSubmit": [ … ],\n+    "Stop": [ … ]\n+  }\n }',
		applied: false,
		backup: null
	};
</script>

<Panel
	title="Connector states"
	description="Every state a connector can be in, drawn from the fixtures the state tests use (lib/sources/fixtures.ts). Each has one icon, label, tone, primary action and sentence."
>
	<ul class="connector-states">
		{#each STATE_IDS as id (id)}
			<li>
				<StateRow source={STATE_FIXTURES[id]} now={NOW} />
				<p class="muted state-meta">
					<code>{id}</code> · primary action: {STATES[id].actionLabel}
				</p>
			</li>
		{/each}
	</ul>
	<div class="grid two">
		<div class="stack">
			<Heartbeat source={STATE_FIXTURES.receiving} now={NOW} />
			<Collects source={STATE_FIXTURES.available} />
			<SpecProblems problems={SPEC_PROBLEM_FIXTURES} />
		</div>
		<ConsentReview
			action="connect"
			change={SAMPLE_CHANGE}
			onconfirm={() => {}}
			oncancel={() => {}}
			autofocus={false}
		/>
	</div>
</Panel>

<style>
	.connector-states {
		display: grid;
		gap: var(--space-4);
		margin-bottom: var(--space-5);
	}

	.state-meta {
		margin-top: var(--space-1);
		font-size: var(--text-xs);
	}
</style>
