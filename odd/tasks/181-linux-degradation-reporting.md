# #181 — Degradaciones informadas como éxito o con indicaciones que no resuelven (atajos, bandeja)

> Issue: https://github.com/egdev6/quorb/issues/181 — `type:bug`, `status:approved`.
> Assignee: `SrStamm` (verificado). Scope aprobado en triaje: **puntos 1 y 2**.
> El punto 3 (auto-hide sin efecto) va en el ciclo de #180 y queda fuera.
> Alimenta el cierre por criterios de #107 (criterio 4).
> Plan publicado y **aprobado con ajustes** por el mantenedor:
> https://github.com/egdev6/quorb/issues/181#issuecomment-6400158626 (plan)
> y la respuesta del mantenedor (aprobación con ajustes 1 y 2).

## Objetivo

Que los fallos de degradación en Linux/GNOME Wayland se informen como tales
(con causa real y acción posible), nunca como éxito.

## Tareas

### T1 — Atajos: diferenciar «registrado» de «funcional»
- [x] T1.1 Lab/probe: `global-hotkey` 0.8.0 solo trae backend X11 en Linux
      (`XGrabKey` por XWayland); bajo Wayland `register()` devuelve `Ok` sin
      efecto global. El portal `GlobalShortcuts` existe en la sesión pero el
      plugin no lo usa.
- [x] T1.2 En `apply_shortcuts` (`src-tauri/src/lib.rs`): estado «limitado»
      con mensaje accionable por atajo registrado en Wayland; sin afirmar
      activación; registro best-effort conservado; errores reales intactos.
      Decisión extraída a `compute_shortcuts` (pura) para cubrir la matriz
      de tests del ajuste del mantenedor.
- [x] T1.3 `shortcuts.enabled` conserva la intención del usuario; el estado
      real se refleja en la UI (canal `warnings`, badge del nav, aviso
      amarillo `role="status"` ≠ error rojo `role="alert"`).
- [x] T1.4 Tests deterministas de la matriz completa (punto anterior).

### T2 — Bandeja: estado por host StatusNotifier (diseño ajustado)
- [x] T2.1 Sondear `org.kde.StatusNotifierWatcher` en `ListNames` (D-Bus) y la
      extensión AppIndicator antes de emitir el consejo (`wm_compat.rs`).
- [x] T2.2 Estado primario del host (presente/ausente/desconocido);
      extensión complementaria (habilitada/deshabilitada/ausente/desconocida);
      sin host nunca se recomienda reinstalar lo que existe; con host solo se
      acredita disponibilidad, no visibilidad.
- [x] T2.3 Sondas con timeout (750 ms), fuera del hilo de UI, degradan a
      «desconocido»; un error de sondeo nunca es «extensión ausente».
      Matriz de tests deterministas completa (parseo puro + ramas de nota).
- [x] T2.4 Sin dependencias nuevas (`gdbus` + `gnome-extensions`, patrón puro
      + wrapper real como `detect_wm_info_from_env`).

### T3 — Verificación y entrega
- [x] T3.1 `npm test` (676 vitest + 225 node, verde), `npm run typecheck`
      (verde), `cargo test --locked` (280 ok, 2 ignored: smokes reales).
- [x] T3.2 ~~Plan publicado en la issue~~ (publicado y aprobado con ajustes).
- [x] T3.3 Comprobación en la sesión GNOME Wayland real: smoke `#[ignore]`
      `real_session_tray_diagnostics_smoke` ejecutado sobre la sesión del
      reporte (Ubuntu 26.04 + GNOME Wayland) → host ausente + extensión
      instalada pero deshabilitada → nota con causa real y consejo de
      habilitar (no reinstalar). La verificación visual de la UI de Ajustes
      queda como paso previo a la PR.
- [x] T3.4 PR abierta con plantilla del repo, aceptación y evidencia:
      https://github.com/egdev6/quorb/pull/183 (head `SrStamm:fix/181-...`,
      fork creado según la convención del repo). La autoasignación de la PR
      no fue posible (requiere permisos de escritura); la validación y el
      merge son del mantenedor.

## Evidencia de commits (rama `fix/181-wayland-degradation-reporting`)

- `f580a98` fix(linux): informar atajos registrados sin efecto global bajo Wayland (#181)
- `1cbb338` fix(linux): estado de bandeja por host StatusNotifier con sondeo acotado (#181)
- `6593954` test(linux): smoke real del diagnóstico de bandeja con sondas acotadas (#181)
- `e6535fd` docs(i18n): regenerar inventario i18n tras #181
