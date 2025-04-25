use native_tls::Identity;
use rcgen::{CertificateParams, DistinguishedName, DnType};
use rsa::pkcs8::EncodePrivateKey;
use rsa::rand_core::OsRng;
use rsa::RsaPrivateKey;

use crate::db::new_profile;

pub struct Profile {
    pub name: String,
    pub identity: Identity,
    pub active: bool,
}

impl Profile {
    pub fn new(name: String) -> Self {
        let mut distinguished_name = DistinguishedName::new();
        distinguished_name.push(DnType::CommonName, &name);

        let mut params = CertificateParams::new(vec![]).unwrap();
        params.distinguished_name = distinguished_name;
        params.not_before = time::OffsetDateTime::now_utc();
        params.not_after = time::OffsetDateTime::now_utc() + time::Duration::days(1825); // 5 years
        params.is_ca = rcgen::IsCa::ExplicitNoCa;

        // Normally we would use the default P256 keypair, but Windows doesn't support it
        // in their crypto APIs so we use RSA instead.
        // Relevant issue: https://github.com/sfackler/rust-native-tls/issues/233
        let mut rng = OsRng;
        let bits = 2048;
        let private_key = RsaPrivateKey::new(&mut rng, bits).unwrap();
        let private_key_der = private_key.to_pkcs8_der().unwrap();
        let key_pair = rcgen::KeyPair::try_from(private_key_der.as_bytes()).unwrap();

        let certificate = params.self_signed(&key_pair).unwrap();
        let cert_pem = certificate.pem();
        let key_pem = key_pair.serialize_pem();

        let _ = new_profile(name.clone(), cert_pem.clone(), key_pem.clone());

        let identity = Identity::from_pkcs8(cert_pem.as_bytes(), key_pem.as_bytes()).unwrap();

        Self {
            name,
            identity,
            active: true,
        }
    }
}
