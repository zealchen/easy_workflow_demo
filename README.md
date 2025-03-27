# Rationale
This simple workflow service demo can incorporate key modern features for secure network communication, authorization, job dispatch, arbitrary binary execution, and streaming in Rust by leveraging core crates such as Tokio and Tonic.

# Requirements
Implement a prototype job worker service that provides an API to run arbitrary processes.

**Library**

- Worker library with methods to start/stop/query status and get the output of a process.
- Library should be able to stream the output of a running process.
    - Output should be from start of process execution.
    - Multiple concurrent clients should be supported.
- Add resource control for CPU, Memory and Disk IO per-process using Windows job objects.

**API**

- [GRPC](https://grpc.io/) API to start/stop/get status/stream output of a running process.
- Use mTLS authentication and verify client certificate. Set up strong set of cipher suites for TLS and good crypto setup for certificates. Do not use any other authentication protocols on top of mTLS.
- Use a simple authorization scheme.

**Client**

- CLI should be able to connect to worker service and start, stop, get status, and stream output of a process.

# Implementation Plan  

### **Server Side**  
- [x] gRPC using Tonic  
- [x] Asynchronous execution with Tokio  
- [x] mTLS authentication with configurable cipher suites  
- [x] Simple RBAC authorization  
- [x] Resource management on Windows/Linux  
- [x] Arbitrary binary execution  
- [x] Streaming output  

### **Client Side**  
- [x] CLI demo using Clap  
- [x] mTLS authentication  
- [x] Streaming job output  

### **Testing**  
- [x] Mock testing with Tokio  

### **CI/CD**  
- [x] Continuous Integration (CI) with GitHub Actions  
