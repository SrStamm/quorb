import { describe, expect, it } from 'vitest';
import { dragMode } from './drag-session';

describe('dragMode', () => {
  it('decides by effective GDK backend, not session type', () => {
    // The maintainer-required case for #180: a Wayland session forced to
    // GDK_BACKEND=x11 reports displayBackend 'x11' from get_wm_info, and the
    // decision layer must treat it as native (snap + persistence keep working
    // under XWayland). The session + override combination itself is covered by
    // the Rust integration test in src-tauri/src/wm_compat.rs.
    expect(dragMode('x11')).toEqual({
      mode: 'native',
      allowSnap: true,
      allowPersist: true,
    });
    // Native GDK Wayland display: the compositor moves the window, so neither
    // snap nor persistence is possible (see rationale in drag-session.ts).
    expect(dragMode('wayland')).toEqual({
      mode: 'compositor',
      allowSnap: false,
      allowPersist: false,
    });
  });

  it.each([
    ['x11', 'native', true, true],
    ['wayland', 'compositor', false, false],
  ] as const)('keeps decision invariants for %s', (backend, mode, snap, persist) => {
    const decision = dragMode(backend);
    expect(decision.mode).toBe(mode);
    expect(decision.allowSnap).toBe(decision.mode === 'native');
    expect(decision.allowPersist).toBe(snap);
    expect(decision.allowPersist).toBe(persist);
  });
});
