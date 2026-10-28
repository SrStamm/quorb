//! OpenCode Go — cuotas de suscripción (5 h, semanal y ciclo de facturación).
//!
//! Endpoint: GET https://opencode.ai/zen/go/v1/usage con Authorization: Bearer <key>.
//! Sin x-opencode-org-id. Parseo tolerante por ventana; degradación parcial explícita.

use super::{net_error, Who};
use crate::config::OpenCodeGoConfig;
use crate::model::{Kind, Meter, Status, UsageReport};
use crate::secrets;
use serde_json::Value;

const URL: &str = "https://opencode.ai/zen/go/v1/usage";

pub async fn fetch(_cfg: &OpenCodeGoConfig, who: Who<'_>, http: &reqwest::Client) -> UsageReport {
    let mut report = who.report("OpenCode Go", Kind::Subscription);
    let Some(key) = who.secret(secrets::OPENCODE_GO_API_KEY) else {
        return report.fail(
            Status::Unconfigured,
            "Añade tu API key de OpenCode Go en Ajustes.",
        );
    };

    let res = match http
        .get(URL)
        .bearer_auth(&key)
        .header("Accept", "application/json")
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => return report.fail(Status::Error, net_error(e)),
    };

    // Los errores HTTP se resuelven ANTES de interpretar el cuerpo (precedencia
    // acordada); el cuerpo solo se lee para distinguir el 403 EntitlementError.
    let code = res.status().as_u16();
    let text = match res.text().await {
        Ok(t) => t,
        Err(e) => return report.fail(Status::Error, format!("Respuesta inesperada: {e}")),
    };
    let body: Option<Value> = serde_json::from_str(&text).ok();
    if let Some((status, msg)) = status_for_http(code, body.as_ref()) {
        return report.fail(status, msg);
    }

    let body: Value = match body {
        Some(v) => v,
        None => return report.fail(Status::Error, "Respuesta no JSON en el endpoint de uso"),
    };

    let parsed = parse_usage(&body);
    if parsed.meters.is_empty() && parsed.warnings.is_empty() {
        return report.fail(
            Status::Warning,
            "Respuesta sin datos de uso (¿ha cambiado el formato?)",
        );
    }
    report.meters = parsed.meters;
    if let Some(msg) = compose_message(&parsed.warnings) {
        report.status = Status::Warning;
        report.message = Some(msg);
    }
    report
}

/// Mapeo de estados HTTP a `Status` del informe (S1, sin red → testeable).
/// `200` devuelve `None` para que el flujo continúe con el parseo del cuerpo.
fn status_for_http(code: u16, body: Option<&Value>) -> Option<(Status, String)> {
    match code {
        200 => None,
        401 => Some((
            Status::Unconfigured,
            "API key de OpenCode Go rechazada (revísala en Console).".into(),
        )),
        403 => {
            // Cuerpo documentado (no capturado): {"type":"error","error":{"type":"EntitlementError",...}}.
            // Sin ese cuerpo → Unconfigured genérico: el fallo más probable y
            // accionable es la credencial; Console es donde se regenera.
            if body.map(is_entitlement_error).unwrap_or(false) {
                Some((
                    Status::Warning,
                    "La cuenta no tiene una suscripción Go activa.".into(),
                ))
            } else {
                Some((
                    Status::Unconfigured,
                    "No se ha podido autorizar con esta API key (compruébala en Ajustes).".into(),
                ))
            }
        }
        // Fixture sintético marcado: no reproducido en vivo (30 seq + 10 paralelo → 200).
        429 => Some((
            Status::Warning,
            "El endpoint de uso está limitando peticiones; reintento en el próximo ciclo.".into(),
        )),
        code => Some((Status::Error, format!("HTTP {code} en el endpoint de uso"))),
    }
}

fn is_entitlement_error(v: &Value) -> bool {
    v.get("error")
        .and_then(|e| e.get("type"))
        .and_then(Value::as_str)
        == Some("EntitlementError")
        || v.get("type").and_then(Value::as_str) == Some("EntitlementError")
}

#[derive(Default)]
struct Parsed {
    meters: Vec<Meter>,
    warnings: Vec<String>,
}

fn compose_message(warnings: &[String]) -> Option<String> {
    if warnings.is_empty() {
        None
    } else {
        Some(warnings.join("; "))
    }
}

fn parse_usage(v: &Value) -> Parsed {
    let mut out = Parsed::default();
    let Some(usage) = v.get("usage").and_then(Value::as_object) else {
        out.warnings
            .push("Formato inesperado: falta `usage`".into());
        return out;
    };

    // Definir ventanas en orden estable
    let windows = [
        ("rolling", "five_hour", "Cuota · 5 h", Some(18000i64)),
        ("weekly", "weekly", "Cuota · semana", Some(604800i64)),
        ("monthly", "monthly", "Cuota · ciclo de facturación", None),
    ];

    for (key, id, label, window_seconds) in windows {
        match usage.get(key).and_then(Value::as_object) {
            None => {
                out.warnings
                    .push(format!("Ventana ausente o malformada: {key}"));
                continue;
            }
            Some(wobj) => {
                // status desconocido o ausente -> diagnosticado
                match wobj.get("status").and_then(Value::as_str) {
                    Some("ok") => {}
                    Some("rate-limited") => {
                        out.warnings
                            .push(format!("Ventana limitada por tarifa: {key}"));
                    }
                    Some(other) => {
                        out.warnings
                            .push(format!("Estado desconocido en {key}: {other}"));
                    }
                    None => {
                        out.warnings
                            .push(format!("Estado desconocido en {key} (sin campo `status`)"));
                    }
                }

                // percent: solo finito en [0,100]
                let percent = match wobj.get("percent") {
                    Some(Value::Number(n)) => {
                        if let Some(f) = n.as_f64() {
                            if f.is_finite() && (0.0..=100.0).contains(&f) {
                                Some(f)
                            } else {
                                out.warnings
                                    .push(format!("`percent` fuera de rango en {key}"));
                                None
                            }
                        } else {
                            out.warnings.push(format!("`percent` no numérico en {key}"));
                            None
                        }
                    }
                    Some(_) => {
                        out.warnings.push(format!("`percent` no numérico en {key}"));
                        None
                    }
                    None => {
                        out.warnings.push(format!("`percent` ausente en {key}"));
                        None
                    }
                };

                if percent.is_none() {
                    // No hay porcentaje interpretable; si el resto está roto, seguimos con advertencias
                    // pero no creamos un medidor con datos inválidos
                    continue;
                }

                // resetsAt: independiente; si inválido/ausente se advierte pero se conserva el porcentaje
                let resets_at = match wobj.get("resetsAt") {
                    Some(Value::String(s)) if !s.is_empty() => {
                        // Aceptar ISO8601 tal cual; si es claramente inválido no lo convertimos, advertimos
                        // El formato esperado es RFC3339/ISO8601
                        Some(s.clone())
                    }
                    Some(_) | None => {
                        out.warnings
                            .push(format!("`resetsAt` inválido o ausente en {key}"));
                        None
                    }
                };

                out.meters.push(Meter {
                    id: id.into(),
                    label: label.into(),
                    percent,
                    used: None,
                    limit: None,
                    unit: None,
                    resets_at,
                    window_seconds,
                });
            }
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // Fixture sintético marcado: no reproducido en vivo (30 seq + 10 paralelo → 200).
    fn entitlement_body() -> Value {
        json!({"type":"error","error":{"type":"EntitlementError","message":"OpenCode Go subscription required."}})
    }

    fn auth_error_body() -> Value {
        json!({"type":"error","error":{"type":"AuthError","message":"Unauthorized"}})
    }

    #[test]
    fn http_200_continues_to_parsing() {
        assert!(status_for_http(200, None).is_none());
    }

    #[test]
    fn http_401_maps_to_unconfigured() {
        let (st, msg) = status_for_http(401, Some(&auth_error_body())).unwrap();
        assert_eq!(st, Status::Unconfigured);
        assert!(msg.contains("rechazada"));
    }

    #[test]
    fn http_403_entitlement_maps_to_warning() {
        // Procedencia: cuerpo DOCUMENTADO, no capturado (sin key válida sin Go).
        let (st, msg) = status_for_http(403, Some(&entitlement_body())).unwrap();
        assert_eq!(st, Status::Warning);
        assert!(msg.contains("suscripción Go"));
    }

    #[test]
    fn http_403_generic_maps_to_unconfigured() {
        let other = json!({"type":"error","error":{"type":"Other"}});
        for body in [None, Some(&other)] {
            let (st, _) = status_for_http(403, body).unwrap();
            assert_eq!(st, Status::Unconfigured);
        }
    }

    #[test]
    fn http_429_maps_to_warning() {
        // Fixture sintético marcado: 429 no reproducido en vivo.
        let (st, _) = status_for_http(429, None).unwrap();
        assert_eq!(st, Status::Warning);
    }

    #[test]
    fn http_5xx_maps_to_error() {
        let (st, _) = status_for_http(500, None).unwrap();
        assert_eq!(st, Status::Error);
    }

    fn fixture_real() -> Value {
        serde_json::json!({
            "usage": {
                "rolling": {"status": "ok", "percent": 0, "resetsAt": "2026-10-27T18:58:40.827Z"},
                "weekly": {"status": "ok", "percent": 1, "resetsAt": "2026-11-02T00:00:00.000Z"},
                "monthly": {"status": "ok", "percent": 24, "resetsAt": "2026-11-17T21:06:53.000Z"}
            }
        })
    }

    #[test]
    fn parse_real_fixture_three_windows() {
        let p = parse_usage(&fixture_real());
        assert_eq!(p.meters.len(), 3);
        assert_eq!(p.meters[0].id, "five_hour");
        assert_eq!(p.meters[1].id, "weekly");
        assert_eq!(p.meters[2].id, "monthly");
        assert_eq!(p.meters[0].window_seconds, Some(18000));
        assert_eq!(p.meters[1].window_seconds, Some(604800));
        assert_eq!(p.meters[2].window_seconds, None);
        assert_eq!(p.meters[0].percent, Some(0.0));
        assert!(p.warnings.is_empty());
    }

    #[test]
    fn rate_limited_window_warning_keeps_meters() {
        let v = serde_json::json!({
            "usage": {
                "rolling": {"status": "rate-limited", "percent": 100, "resetsAt": "2026-10-27T18:58:40.827Z"},
                "weekly": {"status": "ok", "percent": 10, "resetsAt": "2026-11-02T00:00:00.000Z"},
                "monthly": {"status": "ok", "percent": 20, "resetsAt": "2026-11-17T21:06:53.000Z"}
            }
        });
        let p = parse_usage(&v);
        assert_eq!(p.meters.len(), 3);
        assert!(p
            .warnings
            .iter()
            .any(|w| w.contains("rolling") || w.contains("tarifa")));
    }

    #[test]
    fn missing_window_partial_warning() {
        let v = serde_json::json!({
            "usage": {
                "rolling": {"status": "ok", "percent": 0, "resetsAt": "2026-10-27T18:58:40.827Z"},
                "weekly": {"status": "ok", "percent": 1, "resetsAt": "2026-11-02T00:00:00.000Z"}
            }
        });
        let p = parse_usage(&v);
        assert_eq!(p.meters.len(), 2);
        assert!(p.warnings.iter().any(|w| w.contains("monthly")));
    }

    #[test]
    fn malformed_window_partial_warning() {
        let v = serde_json::json!({
            "usage": {
                "rolling": {"status": "ok", "percent": 0, "resetsAt": "2026-10-27T18:58:40.827Z"},
                "weekly": {"status": "ok", "percent": 1, "resetsAt": "2026-11-02T00:00:00.000Z"},
                "monthly": "bad"
            }
        });
        let p = parse_usage(&v);
        assert_eq!(p.meters.len(), 2);
        assert!(p.warnings.iter().any(|w| w.contains("monthly")));
    }

    #[test]
    fn unknown_status_or_missing_status() {
        let v = serde_json::json!({
            "usage": {
                "rolling": {"status": "ok", "percent": 0, "resetsAt": "2026-10-27T18:58:40.827Z"},
                "weekly": {"status": "weird", "percent": 1, "resetsAt": "2026-11-02T00:00:00.000Z"},
                "monthly": {"percent": 5, "resetsAt": "2026-11-17T21:06:53.000Z"}
            }
        });
        let p = parse_usage(&v);
        assert_eq!(p.meters.len(), 3);
        assert!(p.warnings.iter().any(|w| w.contains("desconocido")));
    }

    #[test]
    fn percent_validation_bounds() {
        // Fuera de rango, no numérico y nulo (json! de un f64 no finito = null,
        // que el parser diagnostica como valor no interpretable).
        for bad in [json!(-1), json!(101), json!("x"), Value::Null] {
            let v = json!({"usage":{"rolling":{"status":"ok","percent":bad,"resetsAt":"2026-10-27T18:58:40.827Z"},"weekly":{"status":"ok","percent":1,"resetsAt":"2026-11-02T00:00:00.000Z"},"monthly":{"status":"ok","percent":2,"resetsAt":"2026-11-17T21:06:53.000Z"}}});
            let p = parse_usage(&v);
            // Si rolling es inválido, se pierde ese medidor pero pueden quedar los otros
            assert!(p.warnings.iter().any(|w| w.contains("percent")));
        }
        // cero válido
        let v0 = json!({"usage":{"rolling":{"status":"ok","percent":0,"resetsAt":"2026-10-27T18:58:40.827Z"},"weekly":{"status":"ok","percent":1,"resetsAt":"2026-11-02T00:00:00.000Z"},"monthly":{"status":"ok","percent":2,"resetsAt":"2026-11-17T21:06:53.000Z"}}});
        assert!(parse_usage(&v0)
            .meters
            .iter()
            .any(|m| m.id == "five_hour" && m.percent == Some(0.0)));
    }

    #[test]
    fn resets_at_independent_from_percent() {
        let v = serde_json::json!({"usage":{"rolling":{"status":"ok","percent":10,"resetsAt":null},"weekly":{"status":"ok","percent":1,"resetsAt":"2026-11-02T00:00:00.000Z"},"monthly":{"status":"ok","percent":2,"resetsAt":"2026-11-17T21:06:53.000Z"}}});
        let p = parse_usage(&v);
        assert_eq!(p.meters.len(), 3);
        let r = p.meters.iter().find(|m| m.id == "five_hour").unwrap();
        assert_eq!(r.percent, Some(10.0));
        assert!(r.resets_at.is_none());
        assert!(p.warnings.iter().any(|w| w.contains("resetsAt")));
    }

    #[test]
    fn combined_single_warning_names_all_affected_windows() {
        // Composición acordada: rate-limited en rolling + monthly ausente →
        // UN único Warning cuyo mensaje nombra ambas ventanas.
        let v = json!({"usage":{"rolling":{"status":"rate-limited","percent":100,"resetsAt":"2026-10-27T18:58:40.827Z"},"weekly":{"status":"ok","percent":1,"resetsAt":"2026-11-02T00:00:00.000Z"}}});
        let p = parse_usage(&v);
        let msg = compose_message(&p.warnings).expect("debe haber un Warning combinado");
        assert!(p.warnings.iter().any(|w| w.contains("rolling")));
        assert!(p.warnings.iter().any(|w| w.contains("monthly")));
        // Ambas ventanas nombradas en el MISMO mensaje compuesto.
        assert!(msg.contains("rolling") && msg.contains("monthly"));
        // Los medidores válidos se conservan (weekly).
        assert_eq!(p.meters.len(), 2);
    }

    #[test]
    fn entitlement_error_detection() {
        let v = serde_json::json!({"type":"error","error":{"type":"EntitlementError","message":"OpenCode Go subscription required."}});
        assert!(is_entitlement_error(&v));
    }
}
