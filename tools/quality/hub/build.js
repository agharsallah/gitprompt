// The build in progress: the panel on the overview (every report of the build, what each one runs
// now, its latest output, its time against the last run), the sidebar pill, and the live view on a
// report's own page. Classic script after render.js; defines window.QB, which app.js uses.
'use strict';

(function () {
	const { h, ago, duration } = window.Q;
	const ICONS = { queued: '○', running: '', done: '✓', failed: '✕', skipped: '–', fresh: '=' };
	/** A running build rewrites status.js every few seconds (runner.mjs: HEARTBEAT_MS); silent this long, it is gone. */
	const STALE_MS = 15_000;

	/**
	 * The build as it really is: one still marked running whose process stopped writing (killed, its
	 * terminal closed) is stopped, and its unfinished reports did not finish. Every view reads this.
	 */
	function settle(build) {
		if (!build?.running) return build;
		const last = build.heartbeat_at ?? build.started_at;
		if (Date.now() - last < STALE_MS) return build;
		const unfinished = (step) => step.state === 'running' || step.state === 'queued';
		const steps = build.steps.map((step) => (unfinished(step) ? { ...step, state: 'skipped', message: 'not finished: the build stopped' } : step));
		return { ...build, running: false, stopped: true, finished_at: last, steps };
	}
	/**
	 * Folds the reader opened or closed, by key: the page redraws every second and they stay as the
	 * reader left them. (A fold created open fires `toggle` too: only a change from how it was drawn counts.)
	 */
	const folds = new Map();
	function fold(key, byDefault, attrs, ...children) {
		const open = folds.has(key) ? folds.get(key) : byDefault;
		const ontoggle = (event) => event.target.open !== open && folds.set(key, event.target.open);
		return h('details', { ...attrs, open: open || undefined, ontoggle }, ...children);
	}

	const finishedCount = (build) => build.steps.filter((s) => !['queued', 'running'].includes(s.state)).length;
	const runningCount = (build) => build.steps.filter((s) => s.state === 'running').length;

	/** `1m 12s / ~3m 27s`, kept current by tick(). */
	function clock(since, expected) {
		const text = duration(Date.now() - since) + (expected ? ` / ~${duration(expected)}` : '');
		return h('span', { class: 'step-time mono', 'data-since': since, 'data-expected': expected || undefined }, text);
	}

	/** A thin bar: time so far against the last run (never full while running). */
	function timeBar(step) {
		if (!step.expected_ms) return null;
		const share = Math.min(0.97, (Date.now() - step.started_at) / step.expected_ms);
		return h('div', { class: 'step-bar', title: 'time so far, against the last run' }, h('i', { style: `width:${share * 100}%`, 'data-bar-since': step.started_at, 'data-bar-expected': step.expected_ms }));
	}

	/** The latest lines a running report printed, folded unless the reader opened them. */
	function output(step, { open = false } = {}) {
		if (!step.tail?.length) return null;
		return fold(`output:${open ? 'page' : 'panel'}:${step.id}`, open, { class: 'step-output' }, h('summary', {}, `latest output (${step.tail.length} lines)`), h('pre', {}, step.tail.join('\n')));
	}

	/** What a running step shows under its name: its activity, the time bar, its output. */
	function live(step, options) {
		if (step.state !== 'running') return null;
		return h('div', { class: 'step-live' }, step.activity && !step.waiting ? h('div', { class: 'step-activity mono' }, step.activity) : null, timeBar(step), output(step, options));
	}

	function detailText(step, report) {
		if (step.state === 'running') return step.waiting || step.detail || '';
		if (step.state === 'queued') return step.waiting ? `queued · ${step.waiting}` : 'queued';
		if (step.state === 'done') return `${report?.headline?.value ?? ''} ${report?.headline?.label ?? ''}`;
		if (step.state === 'fresh') return `${report?.headline?.value ?? ''} ${report?.headline?.label ?? ''} · up to date, built ${ago(step.built_at)}`;
		return (step.message || '').split('\n')[0];
	}

	function stepIcon(step) {
		if (step.state === 'running' && step.waiting) return h('span', { class: 'state-icon s-waiting', title: 'waiting' }, '⧗');
		if (step.state === 'running') return h('span', { class: 'spinner' });
		return h('span', { class: `state-icon s-${step.state}` }, ICONS[step.state]);
	}

	function stepRow(step, { reportById, areaOf }) {
		const report = reportById(step.id);
		const time = step.state === 'running' ? clock(step.started_at, step.expected_ms) : h('span', { class: 'step-time mono' }, step.finished_at ? duration(step.finished_at - step.started_at) : '');
		return h(
			'li',
			{ class: `step st-${step.state}${step.waiting ? ' is-waiting' : ''}` },
			stepIcon(step),
			h('a', { class: 'step-name', href: `#${step.id}` }, h('span', { class: 'muted' }, `${areaOf(report?.area)?.title || ''} · `), report?.title || step.id),
			h('span', { class: 'step-detail', title: detailText(step, report) }, detailText(step, report)),
			time,
			live(step)
		);
	}

	/** `up to 4 long and 2 quick reports at once` */
	const places = (build) => `up to ${build.jobs || 1} long${build.quick_jobs ? ` and ${build.quick_jobs} quick` : ''} reports at once`;

	/** Wall time, and the reports' time added up: what running side by side saved. */
	function work(build) {
		return build.steps.reduce((sum, s) => sum + (s.finished_at && s.started_at ? s.finished_at - s.started_at : 0), 0);
	}

	function head(build) {
		const fresh = build.steps.filter((s) => s.state === 'fresh').length;
		const total = build.steps.length - fresh;
		const kept = fresh ? ` · ${fresh} up to date` : '';
		const failed = build.steps.filter((s) => s.state === 'failed').length;
		if (build.running) {
			const queued = build.steps.filter((s) => s.state === 'queued').length;
			return [h('b', {}, `Building ${finishedCount(build) - fresh} of ${total}`), h('span', { class: 'muted' }, `${runningCount(build)} running · ${queued} queued${kept} · ${places(build)} · `, clock(build.started_at))];
		}
		if (build.stopped) return [h('b', {}, 'Last build stopped before finishing'), h('span', { class: 'muted' }, `${ago(build.finished_at)} · its process ended (killed, or its terminal closed): run just quality again`)];
		if (total === 0) return [h('b', {}, 'Last build: every report up to date'), h('span', { class: 'muted' }, `${ago(build.finished_at)} · no input changed since their last build`)];
		const saved = build.jobs > 1 && total > 1 ? ` · ${duration(work(build))} of work, ${build.jobs} at a time` : '';
		const built = failed ? `Last build: ${failed} of ${total} failed` : `Last build: ${total} ${total === 1 ? 'report' : 'reports'} built`;
		return [h('b', {}, built + kept), h('span', { class: 'muted' }, `${ago(build.finished_at)} · took ${duration(build.finished_at - build.started_at)}${saved}`)];
	}

	/** lookup: { reportById, areaOf }. Running reports first, then queued, then finished. */
	function panel(build, lookup) {
		const total = build.steps.length;
		const finished = finishedCount(build);
		const failed = build.steps.filter((s) => s.state === 'failed').length;
		const order = { running: 0, queued: 1 };
		const steps = build.running ? [...build.steps].sort((a, b) => (order[a.state] ?? 2) - (order[b.state] ?? 2)) : build.steps;
		return fold(
			`build:${build.started_at}`,
			build.running || build.stopped || failed > 0,
			{ class: `build ${build.running ? 'is-running' : failed || build.stopped ? 'is-failed' : 'is-done'}` },
			h('summary', {}, h('span', { class: 'build-head' }, head(build))),
			h('div', { class: 'progress', role: 'progressbar', 'aria-valuemin': 0, 'aria-valuemax': total, 'aria-valuenow': finished }, h('i', { style: `width:${(finished / total) * 100}%` })),
			h('ol', { class: 'steps' }, steps.map((step) => stepRow(step, lookup)))
		);
	}

	function pill(build) {
		if (!build) return h('span', { class: 'pill' }, 'never built');
		if (build.running) return h('span', { class: 'pill p-running' }, h('span', { class: 'spinner small' }), `building ${finishedCount(build)}/${build.steps.length} · ${runningCount(build)} running`);
		const failed = build.steps.filter((s) => s.state === 'failed').length;
		const text = build.stopped ? 'stopped' : failed ? `${failed} failed` : 'up to date';
		return h('span', { class: `pill ${failed || build.stopped ? 'p-failed' : 'p-done'}` }, text, ' · ', ago(build.finished_at));
	}

	/** The callout on a report's page while it runs or waits in the build. */
	function reportCallout(step) {
		if (step.state === 'queued') return h('p', { class: 'callout c-neutral' }, 'Queued in the current build', step.waiting ? ` (${step.waiting})` : '', '.');
		const label = step.waiting ? h('span', {}, '⧗ ', step.waiting) : h('span', {}, h('span', { class: 'spinner small' }), ' Rebuilding: ', step.detail || '…');
		return h('div', { class: `callout live-callout ${step.waiting ? 'c-warn' : 'c-accent'}` }, h('div', { class: 'live-head' }, label, clock(step.started_at, step.expected_ms)), live(step, { open: true }));
	}

	/** Keeps clocks and time bars current between two reads of status.js. */
	function tick() {
		const now = Date.now();
		for (const node of document.querySelectorAll('[data-since]')) {
			const expected = Number(node.dataset.expected);
			node.textContent = duration(now - Number(node.dataset.since)) + (expected ? ` / ~${duration(expected)}` : '');
		}
		for (const node of document.querySelectorAll('[data-bar-since]')) {
			node.style.width = `${Math.min(0.97, (now - Number(node.dataset.barSince)) / Number(node.dataset.barExpected)) * 100}%`;
		}
	}

	window.QB = { ICONS, settle, panel, pill, reportCallout, tick };
})();
