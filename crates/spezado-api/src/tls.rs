// SPDX-License-Identifier: AGPL-3.0-or-later
//! TLS for serving other devices: a certificate of this workspace's own, made once and kept, whose fingerprint the
//! pairing link carries – an app checks it instead of a certificate authority; a browser asks once whether to trust
//! it.

use std::sync::Arc;

use rustls::{
    ServerConfig,
    pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer},
};
use sha2::{Digest, Sha256};
use spezado_app::Workspace;

use crate::ServeError;

/// Where the certificate and its key are kept, in hexadecimal DER, and for which host name.
const CERTIFICATE: &str = "tls-certificate";
const KEY: &str = "tls-key";
const HOST: &str = "tls-host";

/// The certificate of this workspace for `host`, made when there is none for it yet; the server's configuration and
/// the fingerprint, `sha256:` and the hash of the certificate in hexadecimal.
pub fn configuration(workspace: &Workspace, host: &str) -> Result<(Arc<ServerConfig>, String), ServeError> {
    let other = |e: &dyn std::fmt::Display| ServeError::Io(std::io::Error::other(e.to_string()));
    let stored = |key| workspace.workspace_setting(key).map_err(|e| other(&e));
    let (certificate, key) = match (stored(CERTIFICATE)?, stored(KEY)?, stored(HOST)?) {
        (Some(certificate), Some(key), Some(made_for)) if made_for == host => {
            (hex::decode(certificate).map_err(|e| other(&e))?, hex::decode(key).map_err(|e| other(&e))?)
        }
        _ => {
            let names = vec![host.to_owned(), "localhost".to_owned()];
            let made = rcgen::generate_simple_self_signed(names).map_err(|e| other(&e))?;
            let (certificate, key) = (made.cert.der().to_vec(), made.signing_key.serialize_der());
            let keep = |k, v: &[u8]| workspace.set_workspace_setting(k, &hex::encode(v)).map_err(|e| other(&e));
            keep(CERTIFICATE, &certificate)?;
            keep(KEY, &key)?;
            workspace.set_workspace_setting(HOST, host).map_err(|e| other(&e))?;
            (certificate, key)
        }
    };
    let fingerprint = format!("sha256:{}", hex::encode(Sha256::digest(&certificate)));
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let config = ServerConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(|e| other(&e))?
        .with_no_client_auth()
        .with_single_cert(vec![CertificateDer::from(certificate)], PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(key)))
        .map_err(|e| other(&e))?;
    Ok((Arc::new(config), fingerprint))
}
