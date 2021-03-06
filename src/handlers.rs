use actix_web::{http::ContentEncoding, HttpRequest, HttpResponse};
use bytes::Bytes;
use config::System;
use envoy_reader::EnvoyReader;
use prometheus;
use prometheus::{Encoder, GaugeVec, IntGaugeVec, TextEncoder};

use GIT_REVISION;
use RUST_VERSION;
use VERSION;

lazy_static! {
    static ref BUILD_INFO: GaugeVec = register_gauge_vec!(
        "envoy_build_info",
        "A metric with a constant '1' value labeled by version, revision and rustversion",
        &["version", "revision", "rustversion"]
    ).unwrap();
    static ref ONLINE: IntGaugeVec = register_int_gauge_vec!(
        "envoy_online",
        "Metric scraping successful",
        &["host", "envoy"]
    ).unwrap();
    static ref CURRENT_WATTS: IntGaugeVec = register_int_gauge_vec!(
        "envoy_current_watts",
        "Number of watts being produced",
        &["host", "envoy"]
    ).unwrap();
    static ref TODAY_WATT_HOURS: IntGaugeVec = register_int_gauge_vec!(
        "envoy_today_watt_hours",
        "Number of watt hours produced today",
        &["host", "envoy"]
    ).unwrap();
    static ref LIFETIME_WATT_HOURS: IntGaugeVec = register_int_gauge_vec!(
        "envoy_lifetime_watt_hours",
        "Number of watt hours ever produced",
        &["host", "envoy"]
    ).unwrap();
    static ref INVERTER_LAST_WATTS: IntGaugeVec = register_int_gauge_vec!(
        "envoy_inverter_last_watts",
        "Number of watts last reported produced by an inverter",
        &["host", "envoy", "inverter"]
    ).unwrap();
}

static LANDING_PAGE: &'static str = "<html>
<head><title>Enphase Envoy Exporter</title></head>
<body>
<h1>Enphase Envoy Exporter</h1>
<p><a href=\"/metrics\">Metrics</a></p>
</body>
";

pub fn index(_req: &HttpRequest<Vec<System>>) -> HttpResponse {
    HttpResponse::Ok()
        .content_encoding(ContentEncoding::Auto)
        .content_type("text/html")
        .body(LANDING_PAGE)
}

pub fn metrics(req: &HttpRequest<Vec<System>>) -> HttpResponse {
    for sys in req.state().clone() {
        let host = sys.host.unwrap_or(String::from(""));
        let url = sys.url.unwrap_or(String::from(""));
        let user = sys.user.unwrap_or(String::from(""));
        let pass = sys.pass.unwrap_or(String::from(""));
        let sn = sys.sn.unwrap_or(String::from(""));
        let status = match EnvoyReader::status(&url, &user, &pass) {
            Ok(x) => x,
            Err(x) => {
                ONLINE.with_label_values(&[&host, &sn]).set(0);
                println!("{} failed: {}", host, x);
                continue;
            }
        };
        ONLINE
            .with_label_values(&[&host, &sn])
            .set(if status.online { 1 } else { 0 });
        CURRENT_WATTS
            .with_label_values(&[&host, &sn])
            .set(status.watts_now);
        LIFETIME_WATT_HOURS
            .with_label_values(&[&host, &sn])
            .set(status.watt_hours_lifetime);
        TODAY_WATT_HOURS
            .with_label_values(&[&host, &sn])
            .set(status.watt_hours_today);
        for (inverter_serial, watts) in status.inverters {
            INVERTER_LAST_WATTS
                .with_label_values(&[&host, &sn, inverter_serial.as_str()])
                .set(watts);
        }
    }
    let git_revision = GIT_REVISION.unwrap_or("");
    let rust_version = RUST_VERSION.unwrap_or("");
    BUILD_INFO
        .with_label_values(&[&VERSION, &git_revision, &rust_version])
        .set(1.0);

    let metric_families = prometheus::gather();
    let encoder = TextEncoder::new();
    let mut buffer = vec![];
    encoder.encode(&metric_families, &mut buffer).unwrap();

    HttpResponse::Ok()
        .content_encoding(ContentEncoding::Auto)
        .content_type(encoder.format_type())
        .body(Bytes::from(buffer))
}
