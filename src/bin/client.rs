use clap::Args;
use clap::{Parser, Subcommand};
use easy_workflow_demo::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::fs::File;
use tokio::io::AsyncReadExt;
use tonic::{
    transport::{Channel, ClientTlsConfig},
    Request,
};
// Import the generated proto code
pub mod demo {
    // tonic::include_proto!("demo");
    // include!(concat!(env!("OUT_DIR"), "/demo.rs"));
    include!("../generated/demo.rs");
}

use demo::{work_flow_client::WorkFlowClient, Entrypoint};
use demo::{EnvironmentVariables, JobStatusRequest, Quota};

/// Easy Workflow CLI - A command line tool for managing workflow jobs
#[derive(Parser)]
#[command(version, about, long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    #[clap(flatten)]
    cert_args: CertArgs,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Args, Debug)]
struct CertArgs {
    /// Path to certificates directory
    #[arg(long)]
    cert_config: Option<String>,

    /// Path to client certificate file
    #[arg(long)]
    crt: Option<String>,

    /// Path to CA certificate file
    #[arg(long)]
    ca_crt: Option<String>,

    /// Path to client private key file
    #[arg(long)]
    key: Option<String>,
}

/// Available commands for managing workflow jobs
#[derive(Subcommand)]
enum Commands {
    /// Create and submit a new workflow job
    Create(CreateArgs),
}

/// Arguments for creating a new workflow job
#[derive(Args, Debug)]
struct CreateArgs {
    /// Command to execute
    #[arg(long, required = true)]
    cmd: String,

    /// Environment variables in format KEY=VALUE
    #[arg(long)]
    env: Vec<String>,

    /// CPU quota (e.g., "1" for one core, "0.5" for half core)
    #[arg(long, default_value = "1")]
    cpu: u32,

    /// Memory quota in MB
    #[arg(long, default_value = "1024")]
    memory: u32,

    /// IO quota in MB
    #[arg(long, default_value = "1024")]
    io: u32,

    /// Task timeout in seconds
    #[arg(long, default_value = "0")]
    timeout: u32,

    /// Number of times to retry the task if it fails
    #[arg(long, default_value = "0")]
    retry_count: u32,

    /// Task priority (-10 to 10, higher number means higher priority)
    #[arg(long, default_value = "0")]
    priority: i32,

    /// Labels to attach to the task (format: KEY=VALUE)
    #[arg(long)]
    labels: Vec<String>,

    /// Annotations to attach to the task (format: KEY=VALUE)
    #[arg(long)]
    annotations: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Certs {
    ca_crt: PathBuf,
    crt: PathBuf,
    key: PathBuf,
}

impl Certs {
    async fn try_from(args: &CertArgs) -> Result<Self> {
        let certs = match args.cert_config {
            Some(ref config) => {
                let mut file = File::open(config).await?;
                let mut content = String::new();
                file.read_to_string(&mut content).await?;
                serde_json::from_str(content.as_str())?
            }
            None => Self {
                ca_crt: args.ca_crt.clone().map(PathBuf::from).unwrap_or_default(),
                crt: args.crt.clone().map(PathBuf::from).unwrap_or_default(),
                key: args.key.clone().map(PathBuf::from).unwrap_or_default(),
            },
        };

        if !certs.ca_crt.is_file() {
            Err(anyhow::format_err!("{:?} is not exists.", certs.ca_crt))
        } else if !certs.crt.is_file() {
            Err(anyhow::format_err!("{:?} is not exists.", certs.crt))
        } else if !certs.key.is_file() {
            Err(anyhow::format_err!("{:?} is not exists.", certs.key))
        } else {
            Ok(certs)
        }
    }
}

async fn open_tls_client(certs: Certs) -> Result<WorkFlowClient<Channel>> {
    println!("certs: {:?}", certs);
    // Load client certificate and key
    let cert = tokio::fs::read(certs.crt).await?;
    let key = tokio::fs::read(certs.key).await?;

    // Client identity (client certificate and key)
    let client_identity = tonic::transport::Identity::from_pem(cert, key);

    // CA certificate to verify server
    let server_ca_cert = tokio::fs::read(certs.ca_crt).await?;
    let server_ca_cert = tonic::transport::Certificate::from_pem(server_ca_cert);

    // Configure TLS
    let tls_config = ClientTlsConfig::new()
        .domain_name("localhost") // Must match the server's certificate CN
        .identity(client_identity)
        .ca_certificate(server_ca_cert);

    // Create a channel with TLS configuration
    let channel = Channel::from_static("https://localhost:50051")
        .tls_config(tls_config)?
        .connect()
        .await?;
    Ok(WorkFlowClient::new(channel))
}

impl TryFrom<String> for EnvironmentVariables {
    type Error = anyhow::Error;

    fn try_from(value: String) -> Result<Self> {
        let kv = value.split('=').collect::<Vec<_>>();
        if kv.len() != 2 {
            Err(format!("invalid env input: {}", value.as_str()).into())
        } else {
            Ok(EnvironmentVariables {
                key: kv[0].into(),
                value: kv[1].into(),
            })
        }
    }
}

async fn handle_create(mut client: WorkFlowClient<Channel>, args: CreateArgs) -> Result<()> {
    let envs = args
        .env
        .into_iter()
        .map(EnvironmentVariables::try_from)
        .collect::<Vec<_>>();
    println!("envs: {:?}", envs);
    if let Some(can_parse_envs) = envs
        .iter()
        .reduce(|acc, e| if acc.is_err() { acc } else { e })
    {
        if can_parse_envs.is_err() {
            // how to return err, if can_parse_envs is_err
            return Err("parse envs failed".into());
        }
    }
    let envs = envs.into_iter().map(|e| e.unwrap()).collect::<Vec<_>>();
    let request: Request<JobStatusRequest> = Request::new(JobStatusRequest {
        entrypoint: Some(Entrypoint {
            cmd: args.cmd,
            envs,
        }),
        quota: Some(Quota {
            cpu: args.cpu,
            memory: args.memory,
            io: args.io,
        }),
        timeout: args.timeout,
        retry_count: args.retry_count,
        priority: args.priority,
        labels: args.labels,
        annotations: args.annotations,
    });

    let response = client.get_job_status(request).await?;
    println!("RESPONSE={:?}", response);

    let response = response.into_inner();
    println!("Server message with: {}", response.header.unwrap().message);
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let certs = Certs::try_from(&cli.cert_args).await?;
    let client = open_tls_client(certs).await?;
    match cli.command {
        Commands::Create(args) => {
            handle_create(client, args).await?;
        }
    }

    Ok(())
}
