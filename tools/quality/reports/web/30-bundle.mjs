// What the browser downloads: web/build after `vite build`, every file raw and gzipped, by kind and
// largest first. The dashboard ships inside the binary, so this is also its share of the binary.
import { readdir, readFile } from 'node:fs/promises';
import { extname, join, relative } from 'node:path';
import { gzipSync } from 'node:zlib';

import { webBuild } from '../../lib/shared.mjs';
import { bars, kb, kpis, plural, table, toneBelow } from '../../lib/spec.mjs';

/** Gzipped JavaScript, all chunks together: good up to the first, acceptable up to the second. */
const JS_BUDGET = [250 * 1024, 500 * 1024];
const KINDS = { '.js': 'JavaScript', '.css': 'CSS', '.html': 'HTML', '.woff2': 'Fonts', '.woff': 'Fonts', '.svg': 'Images', '.png': 'Images', '.ico': 'Images', '.json': 'Data' };

export default {
	title: 'Bundle size',
	description: 'web/build after `vite build`: every file raw and gzipped, JavaScript against a budget.',
	needs: [{ command: 'pnpm', install: 'https://pnpm.io/installation' }],
	async run(ctx) {
		await webBuild(ctx);
		ctx.step('measuring web/build');
		const root = join(ctx.web, 'build');
		const files = await Promise.all(
			(await walk(root)).map(async (path) => {
				const bytes = await readFile(path);
				return { path: relative(root, path), kind: KINDS[extname(path)] || 'Other', raw: bytes.length, gzip: gzipSync(bytes).length };
			})
		);
		return page(files);
	}
};

async function walk(dir) {
	const entries = await readdir(dir, { withFileTypes: true });
	const nested = await Promise.all(entries.map((entry) => (entry.isDirectory() ? walk(join(dir, entry.name)) : [join(dir, entry.name)])));
	return nested.flat();
}

function page(files) {
	const sum = (list, key) => list.reduce((total, file) => total + file[key], 0);
	const js = files.filter((file) => file.kind === 'JavaScript');
	const jsGzip = sum(js, 'gzip');
	const largest = js.reduce((max, file) => (file.gzip > (max?.gzip ?? 0) ? file : max), null);
	const kinds = [...new Set(files.map((file) => file.kind))].map((kind) => {
		const mine = files.filter((file) => file.kind === kind);
		return { kind, files: mine.length, raw: sum(mine, 'raw'), gzip: sum(mine, 'gzip') };
	});
	const totalGzip = sum(files, 'gzip');
	const jsTone = toneBelow(jsGzip, ...JS_BUDGET);
	return {
		headline: { value: kb(jsGzip), label: 'JavaScript, gzipped', tone: jsTone },
		summary: `${plural(files.length, 'file')} · ${kb(totalGzip)} gzipped in all (${kb(sum(files, 'raw'))} raw) · largest chunk ${largest ? kb(largest.gzip) : '–'}`,
		sections: [
			kpis([
				{ label: 'JavaScript (gzip)', value: kb(jsGzip), tone: jsTone, hint: `budget ${kb(JS_BUDGET[0])}, at most ${kb(JS_BUDGET[1])}` },
				{ label: 'CSS (gzip)', value: kb(sum(files.filter((f) => f.kind === 'CSS'), 'gzip')) },
				{ label: 'Fonts', value: kb(sum(files.filter((f) => f.kind === 'Fonts'), 'raw')) },
				{ label: 'Largest JS chunk (gzip)', value: largest ? kb(largest.gzip) : '–', tone: largest ? toneBelow(largest.gzip, 80 * 1024, 150 * 1024) : 'neutral' },
				{ label: 'JavaScript files', value: js.length },
				{ label: 'All files', value: files.length }
			]),
			bars(
				'Share of the gzipped download, by kind',
				kinds.sort((a, b) => b.gzip - a.gzip).map((k) => ({ label: k.kind, value: k.gzip / totalGzip, text: `${kb(k.gzip)} · ${plural(k.files, 'file')}` })),
				'Pages load their own chunks: a page needs the shared entry chunks plus its own, not all of them.'
			),
			table(
				'Files, largest first',
				[
					{ key: 'path', label: 'File', mono: true },
					{ key: 'kind', label: 'Kind' },
					{ key: 'raw', label: 'Raw', align: 'right' },
					{ key: 'gzip', label: 'Gzip', align: 'right' }
				],
				[...files].sort((a, b) => b.gzip - a.gzip).map((file) => ({ path: file.path, kind: file.kind, raw: { text: kb(file.raw), sort: file.raw }, gzip: { text: kb(file.gzip), sort: file.gzip } }))
			)
		]
	};
}
