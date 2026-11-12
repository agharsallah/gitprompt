// The dashboard shell: sidebar (areas and their reports), overview (live build progress and every
// report's card), and one page per report drawn from its data. Reads status.js again every second
// while a build runs, so pages fill in as reports finish. Classic script; uses window.Q (render.js).
'use strict';

(function () {
	const { h, rich, ago, duration, block } = window.Q;
	const view = { status: null, data: {}, requested: {}, drawn: { route: null, version: null } };
	const ICONS = { queued: '○', running: '', done: '✓', failed: '✕', skipped: '–' };

	window.qualityStatus = (status) => {
		view.status = status;
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
		setTimeout(poll, view.status && !view.status.build?.running ? 8000 : 1000);
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
								r.state === 'done' && r.headline ? h('span', { class: `nav-value t-${r.headline.tone}` }, String(r.headline.value)) : null
							)
						)
				)
			)
		);
		document.getElementById('build-pill').replaceChildren(buildPill(build));
	}

	function buildPill(build) {
		if (!build) return h('span', { class: 'pill' }, 'never built');
		const done = build.steps.filter((s) => !['queued', 'running'].includes(s.state)).length;
		if (build.running) return h('span', { class: 'pill p-running' }, h('span', { class: 'spinner small' }), `building ${done}/${build.steps.length}`);
		const failed = build.steps.filter((s) => s.state === 'failed').length;
		return h('span', { class: `pill ${failed ? 'p-failed' : 'p-done'}` }, failed ? `${failed} failed` : 'up to date', ' · ', ago(build.finished_at));
	}

	function overview() {
		const { build, areas, reports } = view.status;
		return h(
			'div',
			{ class: 'page' },
			h('header', { class: 'page-head' }, h('div', {}, h('h1', {}, 'Code quality'), h('p', { class: 'lead' }, rich('Every report on the backend and the web dashboard. `just quality` rebuilds them all and this page fills in as they finish; `just quality <area or report>` rebuilds some.')))),
			build ? buildPanel(build) : h('p', { class: 'callout c-neutral' }, rich('Nothing built yet: run `just quality`.')),
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

	function buildPanel(build) {
		const total = build.steps.length;
		const finished = build.steps.filter((s) => !['queued', 'running'].includes(s.state)).length;
		const failed = build.steps.filter((s) => s.state === 'failed').length;
		const rows = build.steps.map((step) => {
			const report = reportById(step.id);
			const took = step.finished_at ? duration(step.finished_at - step.started_at) : null;
			return h(
				'li',
				{ class: `step st-${step.state}` },
				step.state === 'running' ? h('span', { class: 'spinner' }) : h('span', { class: `state-icon s-${step.state}` }, ICONS[step.state]),
				h('a', { class: 'step-name', href: `#${step.id}` }, h('span', { class: 'muted' }, `${areaOf(report?.area)?.title || ''} · `), report?.title || step.id),
				h('span', { class: 'step-detail' }, step.state === 'running' ? step.detail || '' : step.state === 'done' ? `${report?.headline?.value ?? ''} ${report?.headline?.label ?? ''}` : (step.message || '').split('\n')[0]),
				h('span', { class: 'step-time mono', 'data-since': step.state === 'running' ? step.started_at : undefined }, step.state === 'running' ? duration(Date.now() - step.started_at) : took || '')
			);
		});
		const head = build.running
			? [h('b', {}, `Building ${finished} of ${total}`), h('span', { class: 'muted mono', 'data-since': build.started_at }, duration(Date.now() - build.started_at))]
			: [h('b', {}, failed ? `Last build: ${failed} of ${total} failed` : `Last build: ${total} ${total === 1 ? 'report' : 'reports'} built`), h('span', { class: 'muted' }, `${ago(build.finished_at)} · took ${duration(build.finished_at - build.started_at)}`)];
		const panel = h(
			'details',
			{ class: `build ${build.running ? 'is-running' : failed ? 'is-failed' : 'is-done'}`, open: build.running || failed ? true : undefined },
			h('summary', {}, h('span', { class: 'build-head' }, head)),
			h('div', { class: 'progress', role: 'progressbar', 'aria-valuemin': 0, 'aria-valuemax': total, 'aria-valuenow': finished }, h('i', { style: `width:${(finished / total) * 100}%` })),
			h('ol', { class: 'steps' }, rows)
		);
		return panel;
	}

	function card(report) {
		const state = stateOf(report);
		const headline = report.headline;
		const footer =
			state === 'running'
				? h('span', { class: 't-accent' }, stepOf(report.id)?.detail || 'running…')
				: state === 'queued'
					? h('span', { class: 'muted' }, 'queued')
					: state === 'failed'
						? h('span', { class: 't-bad' }, 'last run failed', report.generated_at ? ' · showing the previous result' : '')
						: state === 'skipped'
							? h('span', { class: 'muted' }, 'skipped: ', report.message)
							: state === 'missing'
								? h('span', { class: 'muted' }, rich(`not built yet · \`${report.command}\``))
								: h('span', { class: 'muted' }, `${ago(report.generated_at)} · took ${duration(report.duration_ms)}`);
		return h(
			'a',
			{ class: `card cs-${state}${headline ? ` ct-${headline.tone}` : ''}`, href: `#${report.id}` },
			h('div', { class: 'card-top' }, marker(report), h('h3', {}, report.title)),
			headline
				? h('div', { class: 'card-headline' }, h('b', { class: `t-${headline.tone}` }, String(headline.value)), h('span', {}, headline.label))
				: h('div', { class: 'card-pending' }, state === 'running' ? 'Building the first result…' : 'No result yet'),
			h('p', { class: 'card-summary' }, report.summary || report.description),
			h('div', { class: 'card-foot' }, footer)
		);
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
				report.headline ? h('div', { class: `headline-tile ht-${report.headline.tone}` }, h('b', {}, String(report.headline.value)), h('span', {}, report.headline.label)) : null
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
		if (state === 'running') head.push(h('p', { class: 'callout c-accent' }, h('span', { class: 'spinner small' }), ' Rebuilding: ', step?.detail || '…', ' ', h('span', { class: 'mono', 'data-since': step?.started_at }, duration(Date.now() - (step?.started_at || Date.now())))));
		if (state === 'queued') head.push(h('p', { class: 'callout c-neutral' }, 'Queued in the current build.'));
		if (state === 'failed') head.push(h('div', { class: 'callout c-bad' }, h('b', {}, 'The last run failed'), page ? ' (below: the previous result).' : '.', h('pre', {}, report.message || ''), h('a', { href: report.log, target: '_blank', rel: 'noopener' }, 'Full log ↗')));
		if (state === 'skipped') head.push(h('p', { class: 'callout c-warn' }, 'Skipped: ', report.message));
		return head;
	}

	function copy(target, text) {
		navigator.clipboard?.writeText(text).then(() => {
			target.dataset.copied = 'true';
			setTimeout(() => delete target.dataset.copied, 1200);
		});
	}

	function tick() {
		for (const node of document.querySelectorAll('[data-since]')) node.textContent = duration(Date.now() - Number(node.dataset.since));
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
	setInterval(tick, 1000);
	poll();
})();
