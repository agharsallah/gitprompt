// The dashboard shell: sidebar (areas and their reports), overview (live build progress and every
// report's card), and one page per report drawn from its data. Reads status.js again every second
// while a build runs, so pages fill in as reports finish. Classic script; uses window.Q (render.js)
// and window.QB (build.js: the build in progress).
'use strict';

(function () {
	const { h, rich, ago, duration, block } = window.Q;
	const { ICONS } = window.QB;
	const view = { status: null, data: {}, requested: {}, drawn: { route: null, version: null } };

	window.qualityStatus = (status) => {
		view.status = { ...status, build: window.QB.settle(status.build) };
		for (const report of status.reports) {
			if (report.version && view.requested[report.id] !== report.version) {
				view.requested[report.id] = report.version;
				load(`${report.data}?v=${report.version}`);
			}
		}
		draw();
	};
	window.registerReport = (page) => {
		view.data[page.id] = page;
		draw();
	};

	function load(src) {
		const script = document.createElement('script');
		script.src = src;
		script.onload = script.onerror = () => script.remove();
		document.head.append(script);
	}

	function poll() {
		load(`status.js?t=${Date.now()}`);
		// Between builds too, often enough that a build started from the terminal shows up at once.
		setTimeout(poll, view.status && !view.status.build?.running ? 2000 : 1000);
	}

	const route = () => decodeURIComponent(location.hash.slice(1)) || 'overview';
	const reportById = (id) => view.status?.reports.find((r) => r.id === id);
	const stepOf = (id) => view.status?.build?.steps.find((s) => s.id === id);
	const areaOf = (id) => view.status?.areas.find((a) => a.id === id);

	/** queued | running | done | failed | skipped | missing, from the build first, then the last run. */
	function stateOf(report) {
		const step = view.status?.build?.running ? stepOf(report.id) : null;
		if (step && (step.state === 'queued' || step.state === 'running')) return step.state;
		return report.state || 'missing';
	}

	function marker(report) {
		const state = stateOf(report);
		if (state === 'running') return h('span', { class: 'spinner', 'aria-label': 'running' });
		if (state === 'done') return h('span', { class: `dot d-${report.headline?.tone || 'neutral'}`, 'aria-hidden': 'true' });
		return h('span', { class: `state-icon s-${state}`, 'aria-label': state }, ICONS[state] || '·');
	}

	function draw() {
		if (!view.status) return;
		drawSidebar();
		const id = route();
		const main = document.getElementById('main');
		if (id === 'overview' || !reportById(id)) {
			view.drawn = { route: 'overview', version: null };
			main.replaceChildren(overview());
			document.title = 'GitPrompt Quality';
		} else {
			drawReport(main, reportById(id));
		}
	}

	function drawSidebar() {
		const { build, areas, reports } = view.status;
		const current = route();
		const nav = document.getElementById('nav');
		nav.replaceChildren(
			h('a', { class: 'nav-item', href: '#overview', 'aria-current': current === 'overview' ? 'page' : undefined }, h('span', { class: 'nav-icon', 'aria-hidden': 'true' }, '◫'), h('span', { class: 'nav-title' }, 'Overview')),
			...areas.map((area) =>
				h(
					'div',
					{ class: 'nav-group' },
					h('div', { class: 'nav-group-title' }, area.title),
					reports
						.filter((r) => r.area === area.id)
						.map((r) =>
							h(
								'a',
								{ class: 'nav-item', href: `#${r.id}`, 'aria-current': current === r.id ? 'page' : undefined },
								marker(r),
								h('span', { class: 'nav-title' }, r.title),
								r.state === 'done' && r.headline ? h('span', { class: `nav-value t-${r.headline.tone}${pending(r) ? ' is-old' : ''}` }, String(r.headline.value)) : null
							)
						)
				)
			)
		);
		document.getElementById('build-pill').replaceChildren(window.QB.pill(build));
	}

	function overview() {
		const { build, areas, reports } = view.status;
		return h(
			'div',
			{ class: 'page' },
			h('header', { class: 'page-head' }, h('div', {}, h('h1', {}, 'Code quality'), h('p', { class: 'lead' }, rich('Every report on the backend and the web dashboard. `just quality` runs the checks whose code changed and this page fills in as they finish; `just quality measure` runs the measurements (timings, minutes each, alone on the machine); `just quality <area or report>` runs some.')))),
			build ? window.QB.panel(build, { reportById, areaOf }) : h('p', { class: 'callout c-neutral' }, rich('Nothing built yet: run `just quality`.')),
			areas.map((area) =>
				h(
					'section',
					{ class: 'area' },
					h('div', { class: 'area-head' }, h('h2', {}, area.title), h('p', { class: 'muted' }, area.description)),
					h(
						'div',
						{ class: 'cards' },
						reports.filter((r) => r.area === area.id).map(card)
					)
				)
			)
		);
	}

	/** Queued or running in the build now: what it shows is the previous result. */
	const pending = (report) => ['queued', 'running'].includes(stateOf(report));

	/** The headline of a card: the last result, dimmed and labelled while a new one is on its way. */
	function cardHeadline(report, state) {
		const headline = report.headline;
		if (!headline) return h('div', { class: 'card-pending' }, state === 'running' ? 'Building the first result…' : 'No result yet');
		const old = pending(report) ? h('span', { class: 'card-old' }, `last result, ${ago(report.generated_at)}`) : null;
		return h('div', { class: `card-headline${old ? ' is-old' : ''}` }, h('b', { class: `t-${headline.tone}` }, String(headline.value)), h('span', {}, headline.label), old);
	}

	function card(report) {
		const state = stateOf(report);
		const headline = pending(report) ? null : report.headline;
		const footer =
			state === 'running'
				? runningFoot(stepOf(report.id))
				: state === 'queued'
					? h('span', { class: 'muted' }, 'queued, not started yet', stepOf(report.id)?.waiting ? ` · ${stepOf(report.id).waiting}` : '')
					: state === 'failed'
						? h('span', { class: 't-bad' }, `last run failed${report.failed_at ? ` ${ago(report.failed_at)}` : ''}`, report.generated_at ? ` · showing the result from ${ago(report.generated_at)}` : '')
						: state === 'skipped'
							? h('span', { class: 'muted' }, 'skipped: ', report.message)
							: state === 'missing'
								? h('span', { class: 'muted' }, rich(`not built yet · \`${report.command}\``))
								: doneFoot(report);
		return h(
			'a',
			{ class: `card cs-${state}${headline ? ` ct-${headline.tone}` : ''}`, href: `#${report.id}` },
			h('div', { class: 'card-top' }, marker(report), h('h3', {}, report.title)),
			cardHeadline(report, state),
			h('p', { class: 'card-summary' }, report.summary || report.description),
			h('div', { class: 'card-foot' }, footer)
		);
	}

	/** A finished report's card: when, how long, and whether the code changed since (it was not in the last build). */
	function doneFoot(report) {
		const when = h('span', { class: 'muted' }, `${ago(report.generated_at)} · took ${duration(report.duration_ms)}`);
		if (!report.changed) return when;
		const command = report.measure ? 'just quality measure' : report.command;
		return h('span', { class: 'card-live' }, when, h('span', { class: 't-warn' }, rich(`code changed since · \`${command}\``)));
	}

	/** A running report's card: what it waits for, or its step and latest activity. */
	function runningFoot(step) {
		if (step?.waiting) return h('span', { class: 't-warn' }, '⧗ ', step.waiting);
		return h('span', { class: 'card-live' }, h('span', { class: 't-accent' }, step?.detail || 'running…'), step?.activity ? h('span', { class: 'mono muted' }, step.activity) : null);
	}

	function drawReport(main, report) {
		const page = view.data[report.id];
		const version = page?.version ?? null;
		if (view.drawn.route !== report.id || view.drawn.version !== version) {
			view.drawn = { route: report.id, version };
			main.replaceChildren(h('div', { class: 'page' }, h('div', { id: 'report-head' }), h('div', { id: 'report-body' }, page ? page.sections.map(block) : emptyBody(report))));
			main.scrollTop = 0;
		}
		document.getElementById('report-head').replaceChildren(...reportHead(report, page));
		document.title = `${report.title} · ${areaOf(report.area)?.title} · GitPrompt Quality`;
	}

	function emptyBody(report) {
		const state = stateOf(report);
		if (state === 'running' || state === 'queued') return h('div', { class: 'empty' }, h('span', { class: 'spinner large' }), h('p', {}, 'Building this report…'));
		return h('div', { class: 'empty' }, h('p', {}, rich(`No result yet. Run \`${report.command}\`.`)));
	}

	function reportHead(report, page) {
		const state = stateOf(report);
		const step = stepOf(report.id);
		const area = areaOf(report.area);
		const head = [
			h('nav', { class: 'crumbs' }, h('a', { href: '#overview' }, 'Overview'), ' / ', area?.title, ' / ', report.title),
			h(
				'header',
				{ class: 'report-head' },
				h('div', {}, h('h1', {}, report.title), h('p', { class: 'lead' }, report.description)),
				report.headline ? h('div', { class: `headline-tile ht-${report.headline.tone}${pending(report) ? ' is-old' : ''}`, title: pending(report) ? 'the last result: a new one is on its way' : undefined }, h('b', {}, String(report.headline.value)), h('span', {}, report.headline.label)) : null
			),
			h(
				'div',
				{ class: 'meta-row' },
				page ? h('span', {}, `built ${ago(page.generated_at)} · took ${duration(page.duration_ms)}`) : null,
				h('span', {}, 'rebuild: ', h('code', { class: 'copy', title: 'click to copy', onclick: (e) => copy(e.target, report.command) }, report.command)),
				h('a', { href: report.log, target: '_blank', rel: 'noopener' }, 'Log ↗')
			)
		];
		if (page?.summary) head.push(h('p', { class: 'summary-line' }, page.summary));
		if ((state === 'running' || state === 'queued') && step) head.push(window.QB.reportCallout(step));
		if (state === 'failed') head.push(failedCallout(report, page));
		if (state === 'skipped') head.push(h('p', { class: 'callout c-warn' }, 'Skipped: ', report.message));
		return head;
	}

	/** Why the last run failed, when, and what the page shows instead; with the command that reruns it. */
	function failedCallout(report, page) {
		const when = report.failed_at ? ` ${ago(report.failed_at)}` : '';
		const below = page ? ` Below: the previous result, from ${ago(page.generated_at)}.` : '';
		return h(
			'div',
			{ class: 'callout c-bad' },
			h('b', {}, `The last run failed${when}.`),
			below,
			h('pre', {}, report.message || ''),
			h('span', {}, rich(`Run it again: \`${report.command}\` · `)),
			h('a', { href: report.log, target: '_blank', rel: 'noopener' }, 'Full log ↗')
		);
	}

	function copy(target, text) {
		navigator.clipboard?.writeText(text).then(() => {
			target.dataset.copied = 'true';
			setTimeout(() => delete target.dataset.copied, 1200);
		});
	}

	function setupTheme() {
		const root = document.documentElement;
		try {
			const saved = localStorage.getItem('quality-theme');
			if (saved) root.dataset.theme = saved;
		} catch {
			/* storage unavailable: follow the system */
		}
		document.getElementById('theme').addEventListener('click', () => {
			const dark = root.dataset.theme ? root.dataset.theme === 'dark' : matchMedia('(prefers-color-scheme: dark)').matches;
			root.dataset.theme = dark ? 'light' : 'dark';
			try {
				localStorage.setItem('quality-theme', root.dataset.theme);
			} catch {
				/* storage unavailable */
			}
		});
	}

	setupTheme();
	window.addEventListener('hashchange', () => {
		view.drawn.route = null;
		draw();
		document.getElementById('main').scrollTop = 0;
	});
	setInterval(window.QB.tick, 1000);
	poll();
})();
