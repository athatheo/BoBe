use std::collections::HashMap;
use std::fmt;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use axum::routing::get;
use axum::{Router, middleware as axum_middleware};
use rustls::pki_types::pem::PemObject;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, UnixTime};
use rustls::server::WebPkiClientVerifier;
use rustls::server::danger::{ClientCertVerified, ClientCertVerifier};
use rustls::{
    CertificateError, DigitallySignedStruct, DistinguishedName, Error, RootCertStore,
    SignatureScheme,
};
use secrecy::ExposeSecret;
use tracing::{info, warn};

use super::gateway::BodyGateway;
use super::handlers::{adapter_stream, body_stream};
use super::protocol::BODY_MDNS_AUTH;
use crate::api::middleware::{ApiToken, bearer_auth};
use crate::config::BodyConfig;

struct PinnedClientCertVerifier {
    inner: Arc<dyn ClientCertVerifier>,
    enrolled_leaf: CertificateDer<'static>,
}

impl fmt::Debug for PinnedClientCertVerifier {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PinnedClientCertVerifier")
            .field("enrolled_leaf_len", &self.enrolled_leaf.as_ref().len())
            .finish_non_exhaustive()
    }
}

impl ClientCertVerifier for PinnedClientCertVerifier {
    fn offer_client_auth(&self) -> bool {
        self.inner.offer_client_auth()
    }

    fn client_auth_mandatory(&self) -> bool {
        self.inner.client_auth_mandatory()
    }

    fn root_hint_subjects(&self) -> &[DistinguishedName] {
        self.inner.root_hint_subjects()
    }

    fn verify_client_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        intermediates: &[CertificateDer<'_>],
        now: UnixTime,
    ) -> Result<ClientCertVerified, Error> {
        let verified = self
            .inner
            .verify_client_cert(end_entity, intermediates, now)?;
        if end_entity.as_ref() != self.enrolled_leaf.as_ref() {
            return Err(CertificateError::ApplicationVerificationFailure.into());
        }
        Ok(verified)
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        certificate: &CertificateDer<'_>,
        signature: &DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, Error> {
        self.inner
            .verify_tls12_signature(message, certificate, signature)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        certificate: &CertificateDer<'_>,
        signature: &DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, Error> {
        self.inner
            .verify_tls13_signature(message, certificate, signature)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.inner.supported_verify_schemes()
    }

    fn requires_raw_public_keys(&self) -> bool {
        self.inner.requires_raw_public_keys()
    }
}

fn load_certificates(path: &Path) -> anyhow::Result<Vec<CertificateDer<'static>>> {
    CertificateDer::pem_file_iter(path)
        .with_context(|| format!("open certificate {}", path.display()))?
        .collect::<Result<Vec<_>, _>>()
        .with_context(|| format!("parse certificate {}", path.display()))
}

fn load_private_key(path: &Path) -> anyhow::Result<PrivateKeyDer<'static>> {
    PrivateKeyDer::from_pem_file(path)
        .with_context(|| format!("parse private key {}", path.display()))
}

fn tls_config(config: &BodyConfig) -> anyhow::Result<axum_server::tls_rustls::RustlsConfig> {
    let cert_path = config
        .tls_cert_path
        .as_deref()
        .context("BodyLink requires tls_cert_path")?;
    let key_path = config
        .tls_key_path
        .as_deref()
        .context("BodyLink requires tls_key_path")?;
    let client_ca_path = config
        .client_ca_path
        .as_deref()
        .context("BodyLink requires client_ca_path")?;
    let enrolled_path = config
        .enrolled_client_cert_path
        .as_deref()
        .context("BodyLink requires enrolled_client_cert_path")?;

    let server_chain = load_certificates(Path::new(cert_path))?;
    anyhow::ensure!(!server_chain.is_empty(), "BodyLink server chain is empty");
    let private_key = load_private_key(Path::new(key_path))?;
    let mut client_roots = RootCertStore::empty();
    for certificate in load_certificates(Path::new(client_ca_path))? {
        client_roots
            .add(certificate)
            .context("add BodyLink client CA certificate")?;
    }
    anyhow::ensure!(!client_roots.is_empty(), "BodyLink client CA file is empty");
    let enrolled_leaf = CertificateDer::from_pem_file(Path::new(enrolled_path))
        .context("parse enrolled BodyLink client certificate")?;

    let webpki: Arc<dyn ClientCertVerifier> = WebPkiClientVerifier::builder(Arc::new(client_roots))
        .build()
        .context("build BodyLink client certificate verifier")?;
    let pinned: Arc<dyn ClientCertVerifier> = Arc::new(PinnedClientCertVerifier {
        inner: webpki,
        enrolled_leaf,
    });
    let mut server_config = rustls::ServerConfig::builder()
        .with_client_cert_verifier(pinned)
        .with_single_cert(server_chain, private_key)
        .context("build BodyLink TLS server configuration")?;
    server_config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
    Ok(axum_server::tls_rustls::RustlsConfig::from_config(
        Arc::new(server_config),
    ))
}

fn start_mdns(config: &BodyConfig) -> Option<mdns_sd::ServiceDaemon> {
    if !config.mdns_enabled {
        return None;
    }
    let daemon = mdns_sd::ServiceDaemon::new().ok()?;
    let hostname = config
        .advertised_hostname
        .trim_end_matches('.')
        .trim_end_matches(".local");
    let fqdn = format!("{hostname}.local.");
    let properties = HashMap::from([
        ("pv".to_string(), "1".to_string()),
        ("path".to_string(), "/body/v1".to_string()),
        ("tls".to_string(), "1".to_string()),
        ("auth".to_string(), BODY_MDNS_AUTH.to_string()),
        ("kid".to_string(), config.trust_key_id.clone()),
        ("scope".to_string(), "local".to_string()),
    ]);
    let service = match mdns_sd::ServiceInfo::new(
        "_bobe-body-hub._tcp.local.",
        &format!("BoBe Body Hub on {hostname}"),
        &fqdn,
        "",
        config.port,
        properties,
    ) {
        Ok(service) => service,
        Err(error) => {
            warn!(%error, "body.mdns_service_failed");
            return None;
        }
    };
    if let Err(error) = daemon.register(service) {
        warn!(%error, "body.mdns_register_failed");
        return None;
    }
    Some(daemon)
}

pub(crate) async fn run(
    gateway: Arc<BodyGateway>,
    config: BodyConfig,
    shutdown: tokio_util::sync::CancellationToken,
) -> anyhow::Result<()> {
    validate_config(&config)?;
    let host = config
        .host
        .parse::<IpAddr>()
        .context("parse BodyLink host")?;
    let address = SocketAddr::new(host, config.port);
    let adapter_address = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), config.adapter_port);
    let tls = tls_config(&config)?;
    let mdns = (!host.is_loopback()).then(|| start_mdns(&config)).flatten();

    let body_app = Router::new()
        .route("/body/v1", get(body_stream))
        .with_state(Arc::clone(&gateway));
    let adapter_app = Router::new()
        .route("/body/adapter", get(adapter_stream))
        .layer(axum_middleware::from_fn(bearer_auth))
        .layer(axum::Extension(ApiToken::new(config.adapter_token.clone())))
        .with_state(Arc::clone(&gateway));

    let body_handle = axum_server::Handle::new();
    let adapter_handle = axum_server::Handle::new();
    let body_shutdown = body_handle.clone();
    let adapter_shutdown = adapter_handle.clone();
    tokio::spawn(async move {
        shutdown.cancelled().await;
        body_shutdown.graceful_shutdown(Some(Duration::from_secs(5)));
        adapter_shutdown.graceful_shutdown(Some(Duration::from_secs(5)));
    });

    info!(%address, %adapter_address, "body.gateway_listening");
    let body_server = axum_server::bind_rustls(address, tls)
        .handle(body_handle)
        .serve(body_app.into_make_service());
    let adapter_server = axum_server::bind(adapter_address)
        .handle(adapter_handle)
        .serve(adapter_app.into_make_service());
    gateway.mark_running();
    let result = tokio::try_join!(body_server, adapter_server);
    gateway.mark_stopped();
    if let Some(mdns) = mdns {
        drop(mdns.shutdown());
    }
    result.map(|_| ()).context("BodyLink gateway server")
}

fn validate_config(config: &BodyConfig) -> anyhow::Result<()> {
    anyhow::ensure!(config.enabled, "BodyLink gateway is disabled");
    anyhow::ensure!(
        config.controller_epoch > 0,
        "BodyLink controller_epoch must be positive"
    );
    anyhow::ensure!(
        !config.enrolled_device_id.trim().is_empty(),
        "BodyLink enrolled_device_id is required"
    );
    anyhow::ensure!(
        !config.trust_key_id.trim().is_empty(),
        "BodyLink trust_key_id is required"
    );
    anyhow::ensure!(
        !config.adapter_token.expose_secret().is_empty(),
        "BodyLink adapter_token is required"
    );
    anyhow::ensure!(
        config.port != config.adapter_port,
        "BodyLink body and adapter ports must differ"
    );
    Ok(())
}
