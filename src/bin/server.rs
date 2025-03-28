use std::error::Error;
use tonic::{transport::Server, Request, Response, Status};

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
    async fn get_job_status(
        &self,
        request: Request<JobStatusRequest>,
    ) -> Result<Response<JobStatusResponse>, Status> {
        println!("Got a request: {:?}", request.get_ref().entrypoint);

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
        println!("Client CN: {}", client_cn);
        println!("Client Role: {}", client_role);

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

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
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

    println!("WorkFlowServer listening on {}", addr);

    Server::builder()
        .tls_config(tls_config)?
        .add_service(WorkFlowServer::new(greeter))
        .serve(addr)
        .await?;

    Ok(())
}
