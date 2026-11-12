// The building blocks of a report page (tools/quality/lib/spec.mjs makes them): kpis, bars, table,
// checks, embed, links, note. Classic script (a page opened from disk cannot load modules): it
// defines window.Q, which app.js uses.
'use strict';

(function () {
	/** h('div', { class: 'x', onclick }, child, [children], 'text') */
	function h(tag, attrs, ...children) {
		const node = document.createElement(tag);
		for (const [key, value] of Object.entries(attrs || {})) {
			if (value === undefined || value === null || value === false) continue;
			if (key.startsWith('on')) node.addEventListener(key.slice(2), value);
			else if (key === 'class') node.className = value;
			else node.setAttribute(key, value === true ? '' : value);
		}
		for (const child of children.flat(Infinity)) {
			if (child === undefined || child === null || child === false) continue;
			node.append(child instanceof Node ? child : document.createTextNode(String(child)));
		}
		return node;
	}

	/** Text with `code` spans. */
	function rich(text) {
		return String(text)
			.split(/(`[^`]+`)/)
			.map((part) => (part.startsWith('`') && part.endsWith('`') ? h('code', {}, part.slice(1, -1)) : part));
	}

	function ago(timestamp) {
		const seconds = Math.max(0, Math.round((Date.now() - timestamp) / 1000));
		if (seconds < 45) return 'just now';
		const minutes = Math.round(seconds / 60);
		if (minutes < 60) return `${minutes} min ago`;
		const hours = Math.round(minutes / 60);
		if (hours < 36) return `${hours} h ago`;
		return `${Math.round(hours / 24)} days ago`;
	}

	function duration(ms) {
		const seconds = Math.round(ms / 1000);
		if (seconds < 60) return `${seconds}s`;
		return `${Math.floor(seconds / 60)}m ${String(seconds % 60).padStart(2, '0')}s`;
	}

	const cellText = (cell) => (cell !== null && typeof cell === 'object' ? cell.text : cell);
	const cellSort = (cell) => {
		if (cell !== null && typeof cell === 'object') return cell.sort !== undefined ? cell.sort : cell.text;
		return cell;
	};

	function cell(value, column) {
		const isObject = value !== null && typeof value === 'object';
		const text = isObject ? value.text : value;
		const content = isObject && value.href ? h('a', { href: value.href, target: value.href.startsWith('http') ? '_blank' : undefined, rel: 'noopener' }, String(text ?? '')) : String(text ?? '');
		const classes = [column.align === 'right' ? 'num' : '', column.mono ? 'mono' : '', isObject && value.tone ? `t-${value.tone}` : ''].filter(Boolean).join(' ');
		return h('td', { class: classes || undefined, title: isObject ? value.title : undefined }, content);
	}

	function section(title, note, ...body) {
		return h('section', { class: 'block' }, title ? h('h2', {}, rich(title)) : null, body, note ? h('p', { class: 'block-note' }, rich(note)) : null);
	}

	const blocks = {
		kpis(block) {
			return h(
				'div',
				{ class: 'kpis' },
				block.items.map((item) =>
					h('div', { class: `kpi${item.tone ? ` k-${item.tone}` : ''}`, title: item.hint }, h('span', { class: 'kpi-label' }, item.label), h('b', { class: 'kpi-value' }, String(item.value)), item.hint ? h('span', { class: 'kpi-hint' }, item.hint) : null)
				)
			);
		},

		bars(block) {
			return section(
				block.title,
				block.note,
				h(
					'div',
					{ class: 'bars' },
					block.items.map((item) =>
						h(
							'div',
							{ class: 'bar-row' },
							h('span', { class: 'bar-label mono', title: item.label }, item.label),
							h('span', { class: 'bar-track' }, h('i', { class: `bar-fill b-${item.tone || 'accent'}`, style: `width:${Math.max(0, Math.min(1, item.value)) * 100}%` })),
							h('span', { class: 'bar-text mono' }, item.text ?? '')
						)
					)
				)
			);
		},

		table(block) {
			const pageSize = 25;
			const state = { key: null, desc: false, filter: '', limit: pageSize };
			const body = h('tbody');
			const footer = h('div', { class: 'table-foot' });
			const heads = block.columns.map((column) => h('th', { class: column.align === 'right' ? 'num' : undefined, onclick: () => sortBy(column.key), tabindex: 0 }, column.label, h('span', { class: 'sort-mark' })));
			const sortBy = (key) => {
				state.desc = state.key === key ? !state.desc : true;
				state.key = key;
				draw();
			};
			const draw = () => {
				let rows = block.rows;
				if (state.filter) {
					const needle = state.filter.toLowerCase();
					rows = rows.filter((row) => block.columns.some((c) => String(cellText(row[c.key]) ?? '').toLowerCase().includes(needle)));
				}
				if (state.key) {
					rows = [...rows].sort((a, b) => {
						const [x, y] = [cellSort(a[state.key]), cellSort(b[state.key])];
						const order = typeof x === 'number' && typeof y === 'number' ? x - y : String(x ?? '').localeCompare(String(y ?? ''), 'en', { numeric: true });
						return state.desc ? -order : order;
					});
				}
				body.replaceChildren(...rows.slice(0, state.limit).map((row) => h('tr', {}, block.columns.map((column) => cell(row[column.key], column)))));
				heads.forEach((head, i) => {
					head.lastChild.textContent = block.columns[i].key === state.key ? (state.desc ? ' ↓' : ' ↑') : '';
				});
				footer.replaceChildren(
					rows.length > state.limit
						? h('button', { class: 'link-button', type: 'button', onclick: () => ((state.limit = Infinity), draw()) }, `Show all ${rows.length} rows`)
						: rows.length === 0
							? h('span', {}, state.filter ? 'Nothing matches.' : 'Nothing to list.')
							: h('span', {}, `${rows.length} ${rows.length === 1 ? 'row' : 'rows'}`)
				);
			};
			const filter =
				block.rows.length > 12
					? h('input', {
							class: 'filter',
							type: 'search',
							name: 'filter',
							placeholder: 'Filter…',
							'aria-label': `Filter ${block.title || 'rows'}`,
							oninput: (event) => {
								state.filter = event.target.value;
								state.limit = pageSize;
								draw();
							}
						})
					: null;
			draw();
			return section(block.title, block.note, filter, h('div', { class: 'table-wrap' }, h('table', {}, h('thead', {}, h('tr', {}, heads)), body)), footer);
		},

		checks(block) {
			return section(
				block.title,
				block.note,
				h(
					'ul',
					{ class: 'checks' },
					block.items.map((item) =>
						h(
							'li',
							{ class: item.ok === true ? 'ok' : item.ok === false ? 'ko' : 'na' },
							h('span', { class: 'check-mark', 'aria-hidden': 'true' }, item.ok === true ? '✓' : item.ok === false ? '✕' : '–'),
							h('div', {}, h('span', {}, item.label), item.detail ? h('pre', { class: 'check-detail' }, item.detail) : null)
						)
					)
				)
			);
		},

		embed(block) {
			return section(
				block.title,
				block.note,
				h('div', { class: 'embed-bar' }, h('a', { href: block.src, target: '_blank', rel: 'noopener' }, 'Open on its own ↗')),
				h('iframe', { class: 'embed', src: block.src, title: block.title, loading: 'lazy' })
			);
		},

		links(block) {
			return section(
				block.title,
				null,
				h(
					'ul',
					{ class: 'links' },
					block.items.map((item) => h('li', {}, h('a', { href: item.href, target: '_blank', rel: 'noopener' }, item.label, ' ↗'), item.note ? h('span', { class: 'muted' }, ` ${item.note}`) : null))
				)
			);
		},

		note(block) {
			return h('p', { class: `callout c-${block.tone || 'neutral'}` }, rich(block.text));
		}
	};

	function block(spec) {
		const draw = blocks[spec.type];
		return draw ? draw(spec) : h('p', { class: 'callout c-warn' }, `Unknown block type "${spec.type}".`);
	}

	window.Q = { h, rich, ago, duration, block };
})();
