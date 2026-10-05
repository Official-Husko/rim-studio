import { render } from 'preact';
import './styles/index.css';
import { App } from './app/App';
import { installGlobalErrorToasts } from './app/globalErrors';
import { probeFeatures } from './app/probe';
import { applyTheme } from './app/theme';
import { renderUnsupported } from './app/unsupported';
import { connect } from './shared/ipc';

const root = document.getElementById('app');
if (root) {
  const missing = probeFeatures();
  if (missing.length > 0) {
    renderUnsupported(root, missing);
  } else {
    applyTheme();
    installGlobalErrorToasts();
    render(<App />, root);
    // The shell paints first; pages appear when a transport is installed.
    void connect({ forceMock: new URLSearchParams(location.search).has('mock') });
  }
}
