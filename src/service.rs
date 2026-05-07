//! Background services that expose or forward metrics from this crate.
//!
//! Each service runs in a single background task that is aborted when the
//! returned handle is dropped.

use std::{
    net::SocketAddr,
    sync::Arc,
    time::{Duration, Instant},
};

use hyper::{Request, Response, service::service_fn};
use tokio::{io::AsyncWriteExt as _, net::TcpListener, task::JoinSet};
use tokio_util::{sync::CancellationToken, task::AbortOnDropHandle};
use tracing::{debug, error, info, warn};

use crate::{Error, MetricsSource, parse_prometheus_metrics};

type BytesBody = http_body_util::Full<hyper::body::Bytes>;

/// HTTP server that exposes metrics in the OpenMetrics text format.
///
/// Aborts the accept loop and all in-flight connections on drop. For an
/// orderly shutdown that lets in-flight connections finish, call
/// [`shutdown`](Self::shutdown).
#[derive(Debug)]
pub struct MetricsServer {
    cancel: CancellationToken,
    task: AbortOnDropHandle<()>,
}

impl MetricsServer {
    /// Binds to `addr` and spawns the server in a background task.
    pub async fn spawn(
        addr: SocketAddr,
        registry: impl MetricsSource + Clone,
    ) -> std::io::Result<Self> {
        info!("Starting metrics server on {addr}");
        let listener = TcpListener::bind(addr).await?;
        let cancel = CancellationToken::new();
        let task = tokio::spawn(server_loop(listener, registry, cancel.clone()));
        Ok(Self {
            cancel,
            task: AbortOnDropHandle::new(task),
        })
    }

    /// Gracefully shuts down the server.
    ///
    /// Stops accepting new connections, signals in-flight connections to wind
    /// down, and waits for them to finish. Wrap in [`tokio::time::timeout`]
    /// to bound the wait.
    pub async fn shutdown(self) {
        self.cancel.cancel();
        let _ = self.task.await;
    }
}

async fn server_loop(
    listener: TcpListener,
    registry: impl MetricsSource + Clone,
    cancel: CancellationToken,
) {
    let mut tasks: JoinSet<()> = JoinSet::new();
    loop {
        tokio::select! {
            biased;
            () = cancel.cancelled() => break,
            res = listener.accept() => {
                match res {
                    Ok((stream, _addr)) => {
                        tasks.spawn(serve_connection(stream, registry.clone(), cancel.clone()));
                    }
                    Err(err) => {
                        error!("metrics server accept failed: {err:#}");
                        break;
                    }
                }
            }
            Some(res) = tasks.join_next(), if !tasks.is_empty() => {
                if let Err(err) = res {
                    if !err.is_cancelled() {
                        debug!("metrics connection task failed: {err:#}");
                    }
                }
            }
        }
    }
    while let Some(res) = tasks.join_next().await {
        if let Err(err) = res {
            if !err.is_cancelled() {
                debug!("metrics connection task failed: {err:#}");
            }
        }
    }
}

async fn serve_connection(
    stream: tokio::net::TcpStream,
    registry: impl MetricsSource + Clone,
    cancel: CancellationToken,
) {
    let io = hyper_util::rt::TokioIo::new(stream);
    let conn = hyper::server::conn::http1::Builder::new()
        .serve_connection(io, service_fn(move |req| handler(req, registry.clone())));
    let mut conn = std::pin::pin!(conn);
    tokio::select! {
        res = conn.as_mut() => {
            if let Err(err) = res {
                error!("Error serving metrics connection: {err:#}");
            }
        }
        () = cancel.cancelled() => {
            conn.as_mut().graceful_shutdown();
            if let Err(err) = conn.await {
                error!("Error during graceful metrics shutdown: {err:#}");
            }
        }
    }
}

/// Periodic dumper that writes metrics as CSV rows to a file.
///
/// Aborts the background task on drop.
#[derive(Debug)]
pub struct MetricsDumper {
    _task: AbortOnDropHandle<()>,
}

impl MetricsDumper {
    /// Opens `path` for writing (truncating if it exists) and spawns the dumper.
    ///
    /// Each tick appends a row with the elapsed time and one column per metric.
    pub async fn spawn(
        path: std::path::PathBuf,
        interval: Duration,
        registry: impl MetricsSource,
    ) -> Result<Self, Error> {
        info!(file = %path.display(), ?interval, "running metrics dumper");
        let file = tokio::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&path)
            .await?;
        let task = tokio::spawn(async move {
            let mut file = tokio::io::BufWriter::new(file);
            let start = Instant::now();
            let mut write_header = true;
            loop {
                let encoded = match registry.encode_openmetrics_to_string() {
                    Ok(s) => s,
                    Err(err) => {
                        error!("metrics dumper failed: {err:#}");
                        return;
                    }
                };
                if let Err(err) = dump_metrics(&mut file, &start, &encoded, write_header).await {
                    error!("metrics dumper failed: {err:#}");
                    return;
                }
                if !write_header {
                    tokio::time::sleep(interval).await;
                }
                write_header = false;
            }
        });
        Ok(Self {
            _task: AbortOnDropHandle::new(task),
        })
    }
}

/// Periodic exporter that pushes metrics to a Prometheus push gateway.
///
/// Aborts the background task on drop.
#[derive(Debug)]
pub struct MetricsPushExporter {
    _task: AbortOnDropHandle<()>,
}

impl MetricsPushExporter {
    /// Spawns the push exporter in a background task.
    pub fn spawn(cfg: MetricsExporterConfig, registry: impl MetricsSource) -> Self {
        let task = tokio::spawn(exporter_loop(cfg, registry));
        Self {
            _task: AbortOnDropHandle::new(task),
        }
    }
}

async fn exporter_loop(cfg: MetricsExporterConfig, registry: impl MetricsSource) {
    let MetricsExporterConfig {
        interval,
        endpoint,
        service_name,
        instance_name,
        username,
        password,
    } = cfg;

    // All of the `.expect`s were previously internal to the `reqwest::Client::new` call.
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let push_client = reqwest::Client::builder()
        .use_preconfigured_tls(
            rustls::ClientConfig::builder_with_provider(provider.clone())
                .with_safe_default_protocol_versions()
                .expect("no TLS 1.3 support in ring")
                .dangerous()
                .with_custom_certificate_verifier(Arc::new(
                    rustls_platform_verifier::Verifier::new(provider)
                        .expect("rustls platform verifier incompatible with ring"),
                )),
        )
        .build()
        .expect("reqwest incompatible with ring");

    let prom_gateway_uri =
        format!("{endpoint}/metrics/job/{service_name}/instance/{instance_name}");
    loop {
        tokio::time::sleep(interval).await;
        let buf = match registry.encode_openmetrics_to_string() {
            Ok(buf) => buf,
            Err(err) => {
                warn!("failed to encode metrics: {err:#}");
                continue;
            }
        };
        let mut req = push_client.post(&prom_gateway_uri);
        if let Some(username) = username.clone() {
            req = req.basic_auth(username, Some(password.clone()));
        }
        let res = match req.body(buf).send().await {
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
}

/// HTTP handler that responds with the OpenMetrics encoding of the metrics.
#[allow(clippy::unused_async)]
async fn handler(
    _req: Request<hyper::body::Incoming>,
    registry: impl MetricsSource,
) -> Result<Response<BytesBody>, Error> {
    let content = registry.encode_openmetrics_to_string()?;
    let response = Response::builder()
        .header(hyper::header::CONTENT_TYPE, "text/plain; charset=utf-8")
        .body(body_full(content))
        .expect("Failed to build response");

    Ok(response)
}

/// Creates a new [`BytesBody`] with given content.
fn body_full(content: impl Into<hyper::body::Bytes>) -> BytesBody {
    http_body_util::Full::new(content.into())
}

/// Dump metrics to a file.
async fn dump_metrics(
    file: &mut tokio::io::BufWriter<tokio::fs::File>,
    start: &Instant,
    encoded: &str,
    write_header: bool,
) -> std::io::Result<()> {
    let m = parse_prometheus_metrics(encoded);
    let time_since_start = start.elapsed().as_millis() as f64;

    // take the keys from m and sort them
    let mut keys: Vec<&String> = m.keys().collect();
    keys.sort();

    let mut metrics = String::new();
    if write_header {
        metrics.push_str("time");
        for key in keys.iter() {
            metrics.push(',');
            metrics.push_str(key);
        }
        metrics.push('\n');
    }

    metrics.push_str(&format!("{time_since_start}"));
    for key in keys.iter() {
        let value = m[*key];
        let formatted_value = format!("{value:.3}");
        metrics.push(',');
        metrics.push_str(&formatted_value);
    }
    metrics.push('\n');

    file.write_all(metrics.as_bytes()).await?;
    file.flush().await?;
    Ok(())
}

/// Configuration for pushing metrics to a remote endpoint.
#[derive(PartialEq, Eq, Debug, Default, serde::Deserialize, Clone)]
pub struct MetricsExporterConfig {
    /// The push interval.
    pub interval: Duration,
    /// The endpoint url for the push metrics collector.
    pub endpoint: String,
    /// The name of the service you're exporting metrics for.
    ///
    /// Generally, `metrics_exporter` is good enough for use
    /// outside of production deployments.
    pub service_name: String,
    /// The name of the instance you're exporting metrics for.
    ///
    /// This should be device-unique. If not, this will sum up
    /// metrics from different devices.
    ///
    /// E.g. `username-laptop`, `username-phone`, etc.
    ///
    /// Another potential scheme with good privacy would be a
    /// keyed blake3 hash of the secret key. (This gives you
    /// an identifier that is as unique as a `NodeID`, but
    /// can't be correlated to `NodeID`s.)
    pub instance_name: String,
    /// The username for basic auth for the push metrics collector.
    pub username: Option<String>,
    /// The password for basic auth for the push metrics collector.
    pub password: String,
}
