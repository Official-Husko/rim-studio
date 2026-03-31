import { h } from 'preact';
import logo from '../assets/images/logo-universal.png';

export function LoadingScreen() {
  return (
    <div className="app-shell loading-shell">
      <img className="hero-logo" src={logo} alt="Rim-Studio" />
      <p>Loading Rim-Studio workspace...</p>
    </div>
  );
}
