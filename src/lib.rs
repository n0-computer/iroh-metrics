pub mod bitswap;
pub mod config;
pub mod gateway;
pub mod req;
pub mod resolver;
pub mod store;

use crate::config::Config;
use opentelemetry::{
    global,
    sdk::{propagation::TraceContextPropagator, trace, Resource},
    trace::{TraceContextExt, TraceId},
};
use opentelemetry_otlp::WithExportConfig;
use prometheus_client::{encoding::text::encode, registry::Registry};
use std::env::consts::{ARCH, OS};
use std::time::Duration;
use tokio::task::JoinHandle;
use tracing::log::{debug, warn};
use tracing_opentelemetry::OpenTelemetrySpanExt;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Layer};

pub struct MetricsHandle {
    metrics_task: JoinHandle<()>,
}

impl MetricsHandle {
    /// Shutdown the tracing and metrics subsystems.
    pub fn shutdown(&self) {
        opentelemetry::global::shutdown_tracer_provider();
        self.metrics_task.abort();
    }

    /// Initialize the tracing and metrics subsystems.
    pub async fn new_with_tracer(cfg: Config) -> Result<Self, Box<dyn std::error::Error>> {
        init_tracer(cfg.clone())?;
        let metrics_task = init_metrics(cfg, Registry::default()).await?;
        Ok(MetricsHandle { metrics_task })
    }

    /// Initialize the tracing and metrics subsystems a with custom registry.
    pub async fn from_registry_with_tracer(
        cfg: Config,
        registry: Registry,
    ) -> Result<MetricsHandle, Box<dyn std::error::Error>> {
        init_tracer(cfg.clone())?;
        Self::from_registry(cfg, registry).await
    }

    /// Initialize the metrics subsystems with a custom registry.
    pub async fn from_registry(
        cfg: Config,
        registry: Registry,
    ) -> Result<MetricsHandle, Box<dyn std::error::Error>> {
        let metrics_task = init_metrics(cfg, registry).await?;
        Ok(MetricsHandle { metrics_task })
    }
}

/// Initialize the metrics subsystem.
pub async fn init_metrics(
    cfg: Config,
    registry: Registry,
) -> Result<JoinHandle<()>, Box<dyn std::error::Error>> {
    if !cfg.debug {
        let prom_gateway_uri = format!(
            "{}/metrics/job/{}/instance/{}",
            cfg.prom_gateway_endpoint, cfg.service_name, cfg.instance_id
        );
        let push_client = reqwest::Client::new();
        return Ok(tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(5)).await;
                let mut buff = Vec::new();
                encode(&mut buff, &registry).unwrap();
                let res = match push_client.post(&prom_gateway_uri).body(buff).send().await {
                    Ok(res) => res,
                    Err(e) => {
                        warn!("failed to push metrics: {}", e);
                        continue;
                    }
                };
                match res.status() {
                    reqwest::StatusCode::OK => {
                        debug!("pushed metrics to gateway");
                    }
                    _ => {
                        warn!("failed to push metrics to gateway: {:?}", res);
                        let body = res.text().await.unwrap();
                        warn!("error body: {}", body);
                    }
                }
            }
        }));
    }
    Ok(tokio::spawn(async move {}))
}

/// Initialize the tracing subsystem.
pub fn init_tracer(cfg: Config) -> Result<(), Box<dyn std::error::Error>> {
    let log_subscriber = fmt::layer()
        .pretty()
        .with_filter(EnvFilter::from_default_env());
    if cfg.debug {
        tracing_subscriber::registry().with(log_subscriber).init();
    } else {
        global::set_text_map_propagator(TraceContextPropagator::new());
        let tracer = opentelemetry_otlp::new_pipeline()
            .tracing()
            .with_exporter(
                opentelemetry_otlp::new_exporter()
                    .tonic()
                    .with_endpoint(cfg.collector_endpoint)
                    .with_timeout(std::time::Duration::from_secs(5)),
            )
            .with_trace_config(trace::config().with_resource(Resource::new(vec![
                opentelemetry::KeyValue::new("instance.id", cfg.instance_id),
                opentelemetry::KeyValue::new("service.name", cfg.service_name),
                opentelemetry::KeyValue::new("service.version", cfg.version),
                opentelemetry::KeyValue::new("service.build", cfg.build),
                opentelemetry::KeyValue::new("service.os", OS),
                opentelemetry::KeyValue::new("service.ARCH", ARCH),
                opentelemetry::KeyValue::new("service.environment", cfg.service_env),
            ])))
            .install_batch(opentelemetry::runtime::Tokio)?;

        let opentelemetry = tracing_opentelemetry::layer().with_tracer(tracer);
        tracing_subscriber::registry()
            .with(log_subscriber)
            .with(opentelemetry)
            .try_init()?;
    }
    Ok(())
}

pub fn get_current_trace_id() -> TraceId {
    tracing::Span::current()
        .context()
        .span()
        .span_context()
        .trace_id()
}
