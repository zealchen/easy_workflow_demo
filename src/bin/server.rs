use easy_workflow_demo::Result;
use metrics::{counter, gauge, histogram};
use metrics_exporter_prometheus::{Matcher, PrometheusBuilder, PrometheusHandle};
use rand::Rng;
use std::net::SocketAddr;
use tonic::{transport::Server, Request, Response, Status};
use tracing::instrument;
use tracing::{debug, info};
use tracing_subscriber::{filter::LevelFilter, EnvFilter};

// Import the generated proto code
pub mod demo {
    include!("../generated/demo.rs");
}

use demo::work_flow_server::WorkFlowServer;
use demo::{work_flow_server::WorkFlow, JobStatusRequest, JobStatusResponse};

const OID_ROLE: &str = "1.3.6.1.4.1.12345.1.1.1"; // Example OID for Role

#[derive(Debug, Default)]
pub struct WorkFlowService {}

#[tonic::async_trait]
impl WorkFlow for WorkFlowService {
    #[instrument(skip(self))]
    async fn get_job_status(
        &self,
        request: Request<JobStatusRequest>,
    ) -> std::result::Result<Response<JobStatusResponse>, Status> {
        let c = counter!("create_job_count_total", "action" => "create", "method" => "POST");
        c.increment(1);
        let mut rng = rand::rng();
        let random = rng.random_range(1..10);
        let gauge = gauge!("active_jobs");
        gauge.set(random);
        let random2 = rng.random_range(1..100) as f32 / 100.0;
        let histogram1 = histogram!("create_job_duration_seconds", 
                  "action" => "create", "status" => if random % 2 == 0 { "error" } else { "success" });
        histogram1.record(random2);

        // Extract client certificate
        let client_cert_info = match request.peer_certs() {
            Some(certs) => {
                if let Some(cert) = certs.first() {
                    // Parse the DER-encoded certificate using x509-parser
                    match x509_parser::parse_x509_certificate(cert.as_ref()) {
                        Ok((_, cert)) => {
                            // Extract CN from subject
                            let cn = cert
                                .subject()
                                .iter_common_name()
                                .next()
                                .and_then(|attr| attr.as_str().ok())
                                .map(|s| s.to_string())
                                .unwrap_or_else(|| "Unknown".to_string());

                            // Try to find our custom extension
                            let role = cert
                                .extensions()
                                .iter()
                                .find(|ext| ext.oid.to_string() == OID_ROLE)
                                .and_then(|ext| {
                                    std::str::from_utf8(ext.value).ok().map(|s| s.to_string())
                                })
                                .unwrap_or_else(|| "No role found".to_string());

                            (cn, role)
                        }
                        Err(_) => (
                            "Failed to parse certificate".to_string(),
                            "Unknown role".to_string(),
                        ),
                    }
                } else {
                    (
                        "No certificate found".to_string(),
                        "Unknown role".to_string(),
                    )
                }
            }
            None => (
                "No peer certificates".to_string(),
                "Unknown role".to_string(),
            ),
        };

        let (client_cn, client_role) = client_cert_info;
        debug!("Client CN: {}", client_cn);
        debug!("Client Role: {}", client_role);

        // Create response with certificate info
        let response = demo::JobStatusResponse {
            header: Some(demo::ResponseHeader {
                code: "0".to_string(),
                message: "success".to_string(),
            }),
            job_id: "1234567890".to_string(),
        };

        Ok(Response::new(response))
    }
}

fn init_log() {
    let varname = "LOG_LEVEL";
    let env_filter = if let Ok(log_level) = std::env::var(varname) {
        // Override to avoid simple logs to be spammed with tokio level informations
        let log_level = match &log_level[..] {
            "warn" => "server=warn,other=warn",
            "info" => "server=info,other=info",
            "debug" => "server=debug,other=debug",
            log_level => log_level,
        };
        EnvFilter::builder()
            .with_default_directive(LevelFilter::INFO.into())
            .parse_lossy(log_level)
    } else {
        EnvFilter::new("info")
    };
    if true {
        tracing_subscriber::fmt()
            .with_env_filter(env_filter)
            .json()
            .init();
    } else {
        tracing_subscriber::fmt()
            .with_env_filter(env_filter)
            .compact()
            .init();
    }
}

fn setup_metrics_exporter() {
    const EXPONENTIAL_SECONDS: &[f64] = &[
        0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
    ];

    // Prometheus metrics server started on http://127.0.0.1:9091/metrics
    PrometheusBuilder::new()
        .with_http_listener(SocketAddr::from(([127, 0, 0, 1], 9091)))
        .add_global_label("service", "my_awesome_service")
        .set_buckets_for_metric(
            Matcher::Full("create_job_duration_seconds".to_string()),
            EXPONENTIAL_SECONDS,
        )
        .unwrap()
        .install()
        .expect("failed to install Prometheus recorder")
}

#[tokio::main]
async fn main() -> Result<()> {
    init_log();
    setup_metrics_exporter();
    // Load certificates and private key files
    let cert_path = "certs/server.crt";
    let key_path = "certs/server.key";
    let ca_cert_path = "certs/ca.crt";

    // Load server certificate and key
    let cert = tokio::fs::read(cert_path).await?;
    let key = tokio::fs::read(key_path).await?;

    // Configure server TLS identity
    let server_identity = tonic::transport::Identity::from_pem(cert, key);

    // Configure CA certificate for client verification
    let client_ca_cert = tokio::fs::read(ca_cert_path).await?;
    let client_ca_cert = tonic::transport::Certificate::from_pem(client_ca_cert);

    // Create TLS configuration
    let tls_config = tonic::transport::ServerTlsConfig::new()
        .identity(server_identity)
        .client_ca_root(client_ca_cert);

    let addr = "127.0.0.1:50051".parse()?;
    let greeter = WorkFlowService::default();

    info!("WorkFlowServer listening on {}", addr);

    Server::builder()
        .tls_config(tls_config)?
        .add_service(WorkFlowServer::new(greeter))
        .serve(addr)
        .await?;

    Ok(())
}
