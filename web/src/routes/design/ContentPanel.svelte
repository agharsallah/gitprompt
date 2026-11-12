<script lang="ts">
	import CodeBlock from '#lib/ui/CodeBlock.svelte';
	import Disclosure from '#lib/ui/Disclosure.svelte';
	import EmptyState from '#lib/ui/EmptyState.svelte';
	import KeyValue from '#lib/ui/KeyValue.svelte';
	import Panel from '#lib/ui/Panel.svelte';
	import Tabs from '#lib/ui/Tabs.svelte';

	/** What holds content: tabs, key-value fields, code and diffs, disclosures and empty states. */

	type TabId = 'one' | 'two' | 'three';
	let tab = $state<TabId>('one');
	const TABS: { id: TabId; label: string; count?: number; disabled?: boolean }[] = [
		{ id: 'one', label: 'Activity', count: 12 },
		{ id: 'two', label: 'Usage' },
		{ id: 'three', label: 'Disabled', disabled: true }
	];
	const FIELDS = {
		git_branch: 'main',
		git_ahead: 2,
		files: ['src/app.ts', 'README.md'],
		cwd: '/Users/me/project',
		worktree: false
	};
	const DIFF =
		'@@ hooks @@\n   "hooks": {\n+    "UserPromptSubmit": [ ... ],\n-    "Stop": [ ... ]\n   }';
</script>

<Panel title="Content">
	<Tabs label="Sample tabs" bind:selected={tab} tabs={TABS}>
		{#snippet panel(id)}
			{#if id === 'one'}
				<KeyValue data={FIELDS} rawKeys />
			{:else}
				<p class="muted">Usage panel.</p>
			{/if}
		{/snippet}
	</Tabs>
	<div class="stack">
		<CodeBlock label="settings.json" diff code={DIFF} />
		<Disclosure summary="A disclosure">Hidden detail, opened on demand.</Disclosure>
		<EmptyState title="No prompts yet">Connect a source, then type a prompt in it.</EmptyState>
	</div>
</Panel>
