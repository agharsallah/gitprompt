/**
 * One chart as a file, made in the browser from the SVG the daemon draws (format=svg): saved as is,
 * or drawn on a canvas and saved as PNG. Nothing is uploaded; the browser saves a local blob.
 */

/** Save a blob under a file name (the browser's download, from memory). */
export function save(blob: Blob, name: string): void {
	const url = URL.createObjectURL(blob);
	const a = Object.assign(document.createElement('a'), { href: url, download: name });
	document.body.append(a);
	a.click();
	a.remove();
	setTimeout(() => URL.revokeObjectURL(url), 1000);
}

/** The width and height an SVG file declares (the daemon's standalone charts always do). */
export function svgSize(svg: string): { width: number; height: number } {
	const tag = svg.match(/<svg[^>]*>/)?.[0] ?? '';
	const read = (name: string) => Number(tag.match(new RegExp(`\\s${name}="([\\d.]+)"`))?.[1]) || 0;
	return { width: read('width') || 640, height: read('height') || 320 };
}

/** The SVG drawn at `scale` times its size, as a PNG. */
export async function png(svg: string, scale = 2): Promise<Blob> {
	const { width, height } = svgSize(svg);
	const url = URL.createObjectURL(new Blob([svg], { type: 'image/svg+xml' }));
	try {
		const image = new Image();
		image.src = url;
		await image.decode();
		const canvas = Object.assign(document.createElement('canvas'), {
			width: Math.round(width * scale),
			height: Math.round(height * scale)
		});
		canvas.getContext('2d')?.drawImage(image, 0, 0, canvas.width, canvas.height);
		return await new Promise<Blob>((resolve, reject) =>
			canvas.toBlob(
				(b) => (b ? resolve(b) : reject(new Error('could not draw the chart'))),
				'image/png'
			)
		);
	} finally {
		URL.revokeObjectURL(url);
	}
}
