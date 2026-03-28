use crate::modules::connection::{ssh_impl, AuthMethod};

pub(crate) fn auth_method_to_auth_type(auth_method: AuthMethod) -> ssh_impl::AuthType {
    match auth_method {
        AuthMethod::Password { password, .. } => ssh_impl::AuthType::Password(Some(password)),
        AuthMethod::KeyboardInteractive {} => ssh_impl::AuthType::KeyboardInteractive,
        AuthMethod::PrivateKey {
            key_path,
            passphrase,
            ..
        } => ssh_impl::AuthType::PrivateKey(key_path, passphrase),
        AuthMethod::Agent { agent_path } => ssh_impl::AuthType::Agent(agent_path),
        AuthMethod::Certificate {
            certificate_path,
            private_key_path,
            passphrase,
            ..
        } => ssh_impl::AuthType::Certificate(certificate_path, private_key_path, passphrase),
    }
}
