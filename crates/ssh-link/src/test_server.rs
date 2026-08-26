//! An SSH far side that lives inside the test process: a russh server on
//! loopback and an agent on a Unix socket. Behind the `test-server`
//! feature, for this crate's suite and the app's; nothing of it reaches a
//! shipped binary.
//!
//! No sshd runs and none is needed: russh's server side compiles
//! unconditionally, which is what lets trust and auth be proven on every
//! machine the workspace builds on.

use std::sync::{Arc, Mutex};

use russh::keys::ssh_key;
use russh::keys::{Algorithm, PrivateKey};
use russh::server::{self, Auth, Msg, Session};
use russh::{Channel, ChannelId, MethodSet};

/// What the far side saw, for assertions that need it.
#[derive(Default)]
pub struct Seen {
    pub resizes: Vec<(u32, u32)>,
}

/// The test shell: accepts one authorized key, replies to pty and shell
/// requests, prints `ready`, echoes everything, and treats a lone 0x04 as
/// "exit with status 7".
pub struct Echo {
    authorized: ssh_key::PublicKey,
    seen: Arc<Mutex<Seen>>,
}

impl server::Handler for Echo {
    type Error = russh::Error;

    async fn auth_publickey(
        &mut self,
        _user: &str,
        key: &ssh_key::PublicKey,
    ) -> Result<Auth, Self::Error> {
        if *key == self.authorized {
            Ok(Auth::Accept)
        } else {
            Ok(Auth::Reject {
                proceed_with_methods: Some(MethodSet::empty()),
                partial_success: false,
            })
        }
    }

    async fn channel_open_session(
        &mut self,
        channel: Channel<Msg>,
        reply: server::ChannelOpenHandle,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        reply.accept().await;
        // The channel pumps itself; dropping it here would close it.
        std::mem::forget(channel);
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    async fn pty_request(
        &mut self,
        channel: ChannelId,
        _term: &str,
        _col_width: u32,
        _row_height: u32,
        _pix_width: u32,
        _pix_height: u32,
        _modes: &[(russh::Pty, u32)],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        session.channel_success(channel)?;
        Ok(())
    }

    async fn shell_request(
        &mut self,
        channel: ChannelId,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        session.channel_success(channel)?;
        session.data(channel, &b"ready\r\n"[..])?;
        Ok(())
    }

    async fn data(
        &mut self,
        channel: ChannelId,
        data: &[u8],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        if data == b"\x04" {
            session.exit_status_request(channel, 7)?;
            session.eof(channel)?;
            session.close(channel)?;
        } else {
            session.data(channel, data.to_vec())?;
        }
        Ok(())
    }

    async fn window_change_request(
        &mut self,
        _channel: ChannelId,
        col_width: u32,
        row_height: u32,
        _pix_width: u32,
        _pix_height: u32,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        self.seen.lock().unwrap().resizes.push((col_width, row_height));
        Ok(())
    }
}

/// A freshly-minted key of the given algorithm.
pub fn mint(algorithm: Algorithm) -> PrivateKey {
    PrivateKey::random(&mut rand::rng(), algorithm).unwrap()
}

/// Serve [`Echo`] on loopback with the given host keys, forever. Answers
/// the port and the far side's log. Must be called on a tokio runtime.
pub async fn serve_echo(
    host_keys: Vec<PrivateKey>,
    authorized: ssh_key::PublicKey,
) -> (u16, Arc<Mutex<Seen>>) {
    let seen = Arc::new(Mutex::new(Seen::default()));
    let config = Arc::new(server::Config { keys: host_keys, ..Default::default() });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let far = seen.clone();
    tokio::spawn(async move {
        loop {
            let Ok((stream, _)) = listener.accept().await else { break };
            let handler = Echo { authorized: authorized.clone(), seen: far.clone() };
            let config = config.clone();
            tokio::spawn(async move {
                let _ = server::run_stream(config, stream, handler).await;
            });
        }
    });
    (port, seen)
}

/// An agent on a Unix socket in `dir`, holding one identity. Answers the
/// socket path, ready to become `SSH_AUTH_SOCK`. Must be called on a tokio
/// runtime.
#[cfg(unix)]
pub async fn serve_agent(dir: &std::path::Path, identity: &PrivateKey) -> std::path::PathBuf {
    let sock = dir.join("agent.sock");
    let listener = tokio::net::UnixListener::bind(&sock).unwrap();
    tokio::spawn(russh::keys::agent::server::serve(
        tokio_stream::wrappers::UnixListenerStream::new(listener),
        (),
    ));
    let mut client = russh::keys::agent::client::AgentClient::connect_uds(&sock)
        .await
        .unwrap();
    client.add_identity(identity, &[]).await.unwrap();
    sock
}
