import { h } from 'preact';
import { useEffect, useRef, useState } from 'preact/hooks';
import { Button } from '../ui/Button';
import type { ThemeDefinition, ThemeID } from '../../types';

interface Props {
  activeThemeID: ThemeID;
  themes: ThemeDefinition[];
  onSelectTheme: (themeId: ThemeID) => void;
}

export function ThemeCarousel({ activeThemeID, themes, onSelectTheme }: Props) {
  const trackRef = useRef<HTMLDivElement | null>(null);
  const [canScrollPrev, setCanScrollPrev] = useState(false);
  const [canScrollNext, setCanScrollNext] = useState(false);

  useEffect(() => {
    const track = trackRef.current;
    if (!track) {
      return undefined;
    }

    const updateScrollState = () => {
      const maxScrollLeft = track.scrollWidth - track.clientWidth;
      setCanScrollPrev(track.scrollLeft > 4);
      setCanScrollNext(track.scrollLeft < maxScrollLeft - 4);
    };

    updateScrollState();
    track.addEventListener('scroll', updateScrollState, { passive: true });
    window.addEventListener('resize', updateScrollState);

    return () => {
      track.removeEventListener('scroll', updateScrollState);
      window.removeEventListener('resize', updateScrollState);
    };
  }, [themes]);

  useEffect(() => {
    const track = trackRef.current;
    if (!track) {
      return;
    }

    const activeItem = track.querySelector(`[data-theme-id="${activeThemeID}"]`) as HTMLElement | null;
    activeItem?.scrollIntoView({
      behavior: 'smooth',
      block: 'nearest',
      inline: 'nearest',
    });
  }, [activeThemeID]);

  function scrollByDirection(direction: -1 | 1) {
    const track = trackRef.current;
    if (!track) {
      return;
    }

    const amount = Math.max(260, Math.floor(track.clientWidth * 0.78));
    track.scrollBy({
      left: direction * amount,
      behavior: 'smooth',
    });
  }

  return (
    <div className="theme-carousel">
      <div className="theme-carousel-controls" aria-label="Theme carousel controls">
        <Button
          aria-label="Scroll themes left"
          className="theme-carousel-button"
          disabled={!canScrollPrev}
          onClick={() => scrollByDirection(-1)}
          variant="ghost"
        >
          <i className="fa-solid fa-chevron-left" aria-hidden="true" />
        </Button>
        <Button
          aria-label="Scroll themes right"
          className="theme-carousel-button"
          disabled={!canScrollNext}
          onClick={() => scrollByDirection(1)}
          variant="ghost"
        >
          <i className="fa-solid fa-chevron-right" aria-hidden="true" />
        </Button>
      </div>

      <div className="theme-carousel-track" ref={trackRef}>
        {themes.map((theme) => (
          <button
            key={theme.id}
            data-theme-id={theme.id}
            className={`theme-option ${activeThemeID === theme.id ? 'is-active' : ''}`}
            onClick={() => onSelectTheme(theme.id)}
            type="button"
          >
            <div className={`theme-preview ${theme.previewClassName}`} aria-hidden="true" />
            <div className="theme-option-copy">
              <strong>{theme.name}</strong>
            </div>
          </button>
        ))}
      </div>
    </div>
  );
}
