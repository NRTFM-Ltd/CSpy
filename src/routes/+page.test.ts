import { render, screen, waitFor, fireEvent } from '@testing-library/svelte';
import { describe, it, expect, vi, beforeEach } from 'vitest';

const listeners: Record<string, (event: { payload: unknown }) => void> = {};
const mockInvoke = vi.fn();

vi.mock('@tauri-apps/api/core', () => ({
	invoke: (...args: unknown[]) => mockInvoke(...args),
}));

vi.mock('@tauri-apps/api/event', () => ({
	listen: vi.fn((eventName: string, callback: (event: { payload: unknown }) => void) => {
		listeners[eventName] = callback as (event: { payload: unknown }) => void;
		return Promise.resolve(() => {});
	}),
}));

vi.mock('@tauri-apps/api/app', () => ({
	getVersion: vi.fn(() => Promise.resolve('0.4.0')),
}));

import Page from './+page.svelte';

const futureReset = new Date(Date.now() + 3 * 3600_000).toISOString();

const mockUsage = {
	five_hour: { utilisation: 0.42, resets_at: futureReset },
	seven_day: null,
	fetched_at: new Date().toISOString(),
};

describe('+page.svelte', () => {
	beforeEach(() => {
		vi.clearAllMocks();
		Object.keys(listeners).forEach(k => delete listeners[k]);
		mockInvoke.mockRejectedValue('No cached data yet');
	});

	it('shows loading state initially', async () => {
		render(Page);
		await waitFor(() => {
			expect(screen.getByText(/Reading Keychain/)).toBeTruthy();
		});
	});

	it('renders usage data when usage-updated fires', async () => {
		render(Page);
		await waitFor(() => expect(listeners['usage-updated']).toBeDefined());
		listeners['usage-updated']({ payload: mockUsage });
		await waitFor(() => {
			expect(screen.getByText('42%')).toBeTruthy();
		});
	});

	it('renders error box when usage-error fires with no data', async () => {
		render(Page);
		await waitFor(() => expect(listeners['usage-error']).toBeDefined());
		listeners['usage-error']({ payload: 'Token expired' });
		await waitFor(() => {
			expect(screen.getByText('Token expired')).toBeTruthy();
		});
	});

	it('renders stale warning when error occurs but data exists', async () => {
		render(Page);
		await waitFor(() => expect(listeners['usage-updated']).toBeDefined());
		listeners['usage-updated']({ payload: mockUsage });
		await waitFor(() => expect(screen.getByText('42%')).toBeTruthy());
		listeners['usage-error']({ payload: 'Network error' });
		await waitFor(() => {
			expect(screen.getByText(/showing cached data/i)).toBeTruthy();
		});
	});

	it('calls refresh_usage when refresh button is clicked', async () => {
		mockInvoke.mockImplementation((cmd: string) => {
			if (cmd === 'get_usage') return Promise.reject('no data');
			if (cmd === 'refresh_usage') return Promise.resolve(mockUsage);
			return Promise.reject('unknown command');
		});

		render(Page);
		await waitFor(() => expect(listeners['usage-updated']).toBeDefined());
		listeners['usage-updated']({ payload: mockUsage });
		await waitFor(() => expect(screen.getByText('42%')).toBeTruthy());

		const refreshBtn = screen.getByTitle('Refresh now');
		await fireEvent.click(refreshBtn);

		expect(mockInvoke).toHaveBeenCalledWith('refresh_usage');
	});
});
