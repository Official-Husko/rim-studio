import { fireEvent, screen } from '@testing-library/preact';
import { renderWithProviders } from 'rimstudio-testkit';
import { describe, expect, it, vi } from 'vitest';
import type { FitReportDto, PreviewDto } from 'rimstudio-ipc-types';
import { fixture } from '../../testSupport';
import { FitPanel } from './FitPanel';

const props = { calibrating: false, onCalibrate: () => undefined, onSetPredicted: () => undefined };

describe('FitPanel', () => {
  it('shows a meter per stat with the level in words, the typicality and the pool', () => {
    const report = fixture<FitReportDto>('designer_fit');
    renderWithProviders(<FitPanel report={report} suggestions={[]} {...props} />);
    expect(screen.getByRole('meter', { name: 'Damage' })).toBeTruthy();
    expect(screen.getAllByText('plausible').length).toBeGreaterThan(0);
    expect(screen.getByLabelText('Typicality').textContent).toContain('90');
    expect(screen.getByText(/Low means unusual, not wrong/)).toBeTruthy();
    expect(screen.getByText(/based on 12 items at Industrial/)).toBeTruthy();
  });

  it('says the bands are the defaults and rough, and offers calibration', () => {
    const report = fixture<FitReportDto>('designer_fit');
    const onCalibrate = vi.fn();
    renderWithProviders(
      <FitPanel report={report} suggestions={[]} {...props} onCalibrate={onCalibrate} />,
    );
    expect(screen.getByText(/bands are the defaults/)).toBeTruthy();
    expect(screen.getByText(/Bands are rough/)).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Calibrate bands' }));
    expect(onCalibrate).toHaveBeenCalled();
  });

  it('offers to set an unusual stat to its predicted value', () => {
    const report = fixture<FitReportDto>('designer_fit');
    const preview = fixture<PreviewDto>('designer_preview');
    const unusual: FitReportDto = {
      ...report,
      perStat: report.perStat.map((s) =>
        s.stat === 'damage' ? { ...s, level: 'unusual' as const } : s,
      ),
    };
    const onSet = vi.fn();
    renderWithProviders(
      <FitPanel
        report={unusual}
        suggestions={preview.suggestions}
        {...props}
        onSetPredicted={onSet}
      />,
    );
    expect(screen.getByText('Nearest reference value 25')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Set to 12' }));
    expect(onSet).toHaveBeenCalledWith('/ranged/damage', 12);
  });

  it('waits for the first fit', () => {
    renderWithProviders(<FitPanel report={undefined} suggestions={[]} {...props} />);
    expect(screen.getByText(/appears after the first preview/)).toBeTruthy();
  });
});
