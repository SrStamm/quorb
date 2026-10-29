/** Mirror of the Rust DisplayBackend (wm_compat.rs, serde lowercase). */
export type DisplayBackend = 'x11' | 'wayland';

/**
 * 'native': the window positions itself (X11/XWayland).
 * 'compositor': xdg-shell gesture (startDragging); the compositor moves it.
 */
export type DragMode = 'native' | 'compositor';

export interface DragDecision {
  mode: DragMode;
  allowSnap: boolean;
  allowPersist: boolean;
}

/**
 * Single source of truth for #180 drag strategy, decided by the EFFECTIVE GDK
 * display backend (exposed as `displayBackend` in the get_wm_info response),
 * never by session type: `is_wayland_session_from_env` ignores
 * GDK_BACKEND=x11, so deciding by session would lose snap and persistence
 * exactly under XWayland, where the issue thread shows they work.
 *
 * X11 (including XWayland): gtk_window_move works, so snap on release and
 * persisting dockEdge/dockOffset/dockMonitor are both allowed.
 *
 * Wayland: under xdg-shell only the compositor controls position, the client
 * has no absolute position to consult, and a release edge computed from
 * unreliable coordinates could corrupt the shared config.json that X11
 * sessions also read — so neither snap nor persistence runs there.
 */
export function dragMode(backend: DisplayBackend): DragDecision {
  if (backend === 'x11') {
    return { mode: 'native', allowSnap: true, allowPersist: true };
  }
  return { mode: 'compositor', allowSnap: false, allowPersist: false };
}
