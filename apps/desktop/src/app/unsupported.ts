// The "system webview too old" page. It uses plain inline CSS with literal values on purpose:
// the engine that shows it may lack custom properties, @layer and color-mix() (design brief SS-05).
// This is one of the two files allowed to contain colour literals, next to styles/tokens.css.

/** Replace the page with a static notice naming what is missing. */
export function renderUnsupported(root: HTMLElement, missing: string[]): void {
  root.innerHTML = '';
  const box = document.createElement('div');
  box.setAttribute(
    'style',
    'max-width:560px;margin:15vh auto;padding:24px;font:15px/1.5 system-ui,sans-serif;color:#e8f2ff;background:#0d2440;border:1px solid #3a6ea5',
  );
  const title = document.createElement('h1');
  title.setAttribute('style', 'margin:0 0 12px;font-size:20px');
  title.textContent = 'RimStudio needs a newer system webview';
  const body = document.createElement('p');
  body.textContent = `This webview does not support: ${missing.join(', ')}. Update your system webview or browser and start RimStudio again.`;
  box.append(title, body);
  root.append(box);
  document.body.setAttribute('style', 'background:#081a31;margin:0');
}
