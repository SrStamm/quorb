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
- [ ] T1.2 En `apply_shortcuts` (`src-tauri/src/lib.rs:84`): estado «limitado»
      con mensaje accionable por atajo registrado en Wayland; sin afirmar
      activación; registro best-effort conservado; errores reales intactos.
      **Ajuste del mantenedor:** extraer la decisión a una función pura para
      cubrir la matriz de tests: Wayland+Ok → aviso; X11+Ok → nada; disabled
      → nada; error real → error conservado junto al diagnóstico.
- [ ] T1.3 `shortcuts.enabled` conserva la intención del usuario; el estado
      real se refleja en la UI (canal `warnings`, badge del nav, aviso
      amarillo `role="status"` ≠ error rojo `role="alert"`).
- [ ] T1.4 Tests deterministas de la matriz completa (punto anterior).

### T2 — Bandeja: estado por host StatusNotifier (diseño ajustado)
- [ ] T2.1 Sondear `org.kde.StatusNotifierWatcher` en `ListNames` (D-Bus) y la
      extensión AppIndicator antes de emitir el consejo (`wm_compat.rs:250`).
- [ ] T2.2 **Ajuste 1 del mantenedor (obliga a rediseñar):** el estado
      primario es del **host**: presente / ausente / sondeo fallido o
      desconocido. La extensión es info **complementaria** (instalada /
      habilitada / desconocida, solo si se determina de forma fiable). Sin
      host: explicar que no se detectó dónde mostrar el indicador (nunca
      recomendar reinstalar una extensión que ya existe). Con host: solo se
      acredita disponibilidad del host, no visibilidad del icono.
- [ ] T2.3 **Ajuste 2 del mantenedor:** sondas con timeout, no bloqueantes y
      que degradan a «desconocido» en fallo; un error de sondeo **nunca** se
      reporta como «extensión ausente». Tests deterministas: host
      presente/ausente/fallo; extensión ausente/deshabilitada/desconocida,
      sin consejos contradictorios.
- [ ] T2.4 Sin dependencias nuevas; sondas inyectables (patrón puro + wrapper
      real, como `detect_wm_info_from_env`).

### T3 — Verificación y entrega
- [ ] T3.1 `npm test`, `npm run typecheck`, `cd src-tauri && cargo test`.
- [ ] T3.2 ~~Plan publicado en la issue~~ (publicado y aprobado con ajustes).
- [ ] T3.3 Comprobación en la sesión GNOME Wayland real (exigida por el
      mantenedor en la aprobación).
- [ ] T3.4 PR con plantilla del repo, aceptación y evidencia (requiere
      autorización explícita).

## Evidencia de commits

(pendiente: registrar cada work-unit commit acá)
