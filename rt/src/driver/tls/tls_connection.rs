use crate::api::ip::tcp::{TcpError, TcpSocket, TcpStack};
use crate::api::ip::{IpProtocol, SocketAddress};
use crate::driver::tls::config::Config;
use crate::driver::tls::handshake::{ClientHandshake, ServerHandshake};
use crate::driver::tls::key_schedule::KeySchedule;
use crate::driver::tls::record::{ClientRecord, ServerRecord};
use crate::driver::tls::TlsError;
use crate::prelude::Address;
use heapless::{consts::*, ArrayLength, Vec};
use hkdf::Hkdf;
use hmac::Hmac;
use rand_core::{CryptoRng, RngCore};
use sha2::{Digest, Sha256};

use crate::driver::tls::application_data::ApplicationData;
use crate::driver::tls::content_types::ContentType;
use crate::driver::tls::parse_buffer::ParseBuffer;
use aes_gcm::aead::{generic_array::GenericArray, AeadInPlace, Buffer, NewAead};
use aes_gcm::Aes128Gcm;

enum State {
    Handhshaking,
    HandshakeComplete,
}

pub struct TlsConnection<RNG, D>
where
    RNG: CryptoRng + RngCore + Copy + 'static,
    D: TcpStack + 'static,
{
    delegate: TcpSocket<D>,
    config: &'static Config<RNG>,
    state: State,
    key_schedule: KeySchedule<Sha256, U16, U12>,
}

impl<RNG, D> TlsConnection<RNG, D>
where
    RNG: CryptoRng + RngCore + Copy,
    D: TcpStack,
{
    pub fn new(config: &'static Config<RNG>, delegate: TcpSocket<D>) -> Self {
        Self {
            delegate,
            config,
            state: State::Handhshaking,
            key_schedule: KeySchedule::new(),
        }
    }

    async fn transmit(&mut self, record: &ClientRecord<'_, RNG>) -> Result<(), TlsError> {
        let mut buf: Vec<u8, U1024> = Vec::new();
        record.encode(&mut buf, self.key_schedule.transcript_hash());
        log::info!(
            "**** transmit, hash={:x?}",
            self.key_schedule.transcript_hash().clone().finalize()
        );
        self.delegate_socket()
            .write(&buf)
            .await
            .map(|_| ())
            .map_err(|e| TlsError::TcpError(e))?;

        self.key_schedule.increment_write_counter();
        Ok(())
    }

    async fn receive(&mut self) -> Result<ServerRecord, TlsError> {
        let mut record =
            ServerRecord::read(&mut self.delegate, self.key_schedule.transcript_hash()).await?;

        if let State::Handhshaking = self.state {
            if let ServerRecord::ApplicationData(ApplicationData { header, mut data }) = record {
                log::info!("decrypting {:x?}", &header);
                let crypto = Aes128Gcm::new(&self.key_schedule.get_server_key());
                let nonce = &self.key_schedule.get_server_nonce();
                log::info!("server write nonce {:x?}", nonce);
                let result = crypto.decrypt_in_place(
                    &self.key_schedule.get_server_nonce(),
                    &header,
                    &mut data,
                );

                let content_type =
                    ContentType::of(*data.last().unwrap()).ok_or(TlsError::InvalidRecord)?;

                match content_type {
                    ContentType::Handshake => {
                        let mut buf = ParseBuffer::new(&data[..data.len() - 1]);
                        //let inner = ServerHandshake::parse(&data[..data.len() - 1]);
                        let inner = ServerHandshake::parse(&mut buf);
                        log::debug!("===> inner ==> {:?}", inner);
                        record = ServerRecord::Handshake(inner.unwrap());
                    }
                    _ => {
                        return Err(TlsError::InvalidHandshake);
                    }
                }
                log::debug!("decrypt result {:?}", result);
                log::debug!("decrypted {:?} --> {:x?}", content_type, data);
                self.key_schedule.increment_read_counter();
            }
        };
        log::info!(
            "**** receive, hash={:x?}",
            self.key_schedule.transcript_hash().clone().finalize()
        );
        Ok(record)
    }

    pub async fn connect(&mut self, proto: IpProtocol, dst: SocketAddress) -> Result<(), TlsError> {
        log::info!("connecting delegate socket");
        self.delegate_socket()
            .connect(proto, dst)
            .await
            .map_err(|e| TlsError::TcpError(e))?;
        self.handshake().await?;
        log::info!("handshake complete");
        Ok(())
    }

    pub async fn handshake(&mut self) -> Result<(), TlsError> {
        self.key_schedule.initialize_early_secret();
        let client_hello = ClientRecord::client_hello(self.config);
        self.transmit(&client_hello).await;
        log::info!("sent client hello");

        loop {
            let record = self.receive().await?;

            match record {
                ServerRecord::Handshake(handshake) => match handshake {
                    ServerHandshake::ServerHello(server_hello) => {
                        log::info!("********* ServerHello");
                        if let ClientRecord::Handshake(ClientHandshake::ClientHello(
                            ref client_hello,
                        )) = client_hello
                        {
                            let shared = server_hello
                                .calculate_shared_secret(&client_hello.secret)
                                .ok_or(TlsError::InvalidKeyShare)?;

                            self.key_schedule
                                .initialize_handshake_secret(shared.as_bytes());
                        }
                    }
                    ServerHandshake::EncryptedExtensions(_) => {}
                    ServerHandshake::Certificate(_) => {}
                    ServerHandshake::CertificateVerify(_) => {}
                    ServerHandshake::Finished(_) => {
                        log::info!("FINISHED!")
                    }
                },
                ServerRecord::Alert => {
                    unimplemented!("alert not handled")
                }
                ServerRecord::ApplicationData(application_data) => {
                    /*
                    match application_data {
                        ApplicationData { header, mut data } => {
                            log::info!("decrypting {:x?}", &header);
                            let crypto = Aes128Gcm::new(&self.key_schedule.get_server_key());
                            let nonce = &self.key_schedule.get_server_nonce();
                            log::info!("server write nonce {:x?}", nonce);
                            let result = crypto.decrypt_in_place(
                                &self.key_schedule.get_server_nonce(),
                                &header,
                                &mut data,
                            );

                            let content_type = ContentType::of(*data.last().unwrap())
                                .ok_or(TlsError::InvalidRecord)?;

                            match content_type {
                                ContentType::Invalid => {}
                                ContentType::ChangeCipherSpec => {}
                                ContentType::Alert => {}
                                ContentType::Handshake => {
                                    let mut buf = ParseBuffer::new(&data[..data.len() - 1]);
                                    //let inner = ServerHandshake::parse(&data[..data.len() - 1]);
                                    let inner = ServerHandshake::parse(&mut buf);
                                    log::debug!("===> inner ==> {:?}", inner);
                                }
                                ContentType::ApplicationData => {}
                            }
                            log::debug!("decrypt result {:?}", result);
                            log::debug!("decrypted {:?} --> {:x?}", content_type, data);
                        }
                    }
                    self.key_schedule.increment_read_counter();
                     */
                }
                ServerRecord::ChangeCipherSpec(..) => {
                    // ignore fake CCS
                }
            }
        }

        Ok(())
    }

    pub fn delegate_socket(&mut self) -> &mut TcpSocket<D> {
        &mut self.delegate
    }
}
