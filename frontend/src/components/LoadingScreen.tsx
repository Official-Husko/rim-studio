import { h } from 'preact';
import logo from '../assets/images/logo-universal.png';

export function LoadingScreen() {
  return (
    <div className="app-shell loading-shell">
      <img className="hero-logo" src={logo} alt="RimStudio" />
      <p>Loading RimStudio workspace...</p>
    </div>
  );
}
