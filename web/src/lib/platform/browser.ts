/** The browser's answers: a tab is not ours to hide, a file is a download, the app never calls. */
import type { Platform, SavedFile, Unsubscribe } from '#lib/platform/types.ts';

/** The parts of the page the answers use, so a test can lend its own. */
export type BrowserPage = {
	document: Pick<Document, 'createElement' | 'visibilityState'> & {
		body: Pick<HTMLElement, 'append'>;
		addEventListener(type: 'visibilitychange', listener: () => void): void;
		removeEventListener(type: 'visibilitychange', listener: () => void): void;
	};
	url: Pick<typeof URL, 'createObjectURL' | 'revokeObjectURL'>;
};

const NOTHING: Unsubscribe = () => {};

/** A download through a link nobody sees, then gone. */
export function download(page: BrowserPage, file: SavedFile): void {
	const blob = file.data instanceof Blob ? file.data : new Blob([file.data], { type: file.type });
	const href = page.url.createObjectURL(blob);
	const link = page.document.createElement('a');
	link.href = href;
	link.download = file.name;
	page.document.body.append(link);
	link.click();
	link.remove();
	page.url.revokeObjectURL(href);
}

export function browserPlatform(page: BrowserPage): Platform {
	return {
		desktop: false,
		hideWindow: () => Promise.resolve(),
		saveFile: (file) => Promise.resolve(download(page, file)),
		onNavigate: () => NOTHING,
		onWindowVisible: (handler) => {
			const changed = () => handler(page.document.visibilityState === 'visible');
			page.document.addEventListener('visibilitychange', changed);
			return () => page.document.removeEventListener('visibilitychange', changed);
		},
		recall: {
			problem: () => Promise.resolve(null),
			register: () => Promise.resolve({ kind: 'unsupported' })
		}
	};
}
