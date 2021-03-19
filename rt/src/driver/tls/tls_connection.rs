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
use aes_gcm::aead::{generic_array::GenericArray, AeadInPlace, Buffer, NewAead};
use aes_gcm::Aes128Gcm;

pub struct TlsConnection<RNG, D>
where
    RNG: CryptoRng + RngCore + Copy + 'static,
    D: TcpStack + 'static,
{
    delegate: TcpSocket<D>,
    config: &'static Config<RNG>,
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
        let record =
            ServerRecord::read(&mut self.delegate, self.key_schedule.transcript_hash()).await?;
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
        log::info!("connected delegate socket");

        self.handshake().await?;

        log::info!("handshake complete");

        /*
        loop {
            let next_record = self.receive().await?;
            log::debug!("server record --> {:?}", next_record);

            match next_record {
                ServerRecord::Handshake(_) => {}
                ServerRecord::Alert => {}
                ServerRecord::ApplicationData(ApplicationData { header, mut data }) => {
                    log::info!("decrypting {:x?}", &header);
                    let crypto = Aes128Gcm::new(&self.key_schedule.get_server_key());
                    let nonce = &self.key_schedule.get_server_nonce();
                    log::info!("server write nonce {:x?}", nonce);
                    let result = crypto.decrypt_in_place(
                        &self.key_schedule.get_server_nonce(),
                        &header,
                        &mut data,
                    );
                    log::debug!("decrypt result {:?}", result);
                    let content_type = ContentType::of(data[data.len() - 1]);
                    log::debug!("content-type {:?}", content_type);
                    //log::debug!("decrypted --> {:x?}", data);
                    log::debug!("hi");
                    //ServerRecord::parse(result);
                    if result.is_err() {
                        panic!("unable to decrypt");
                        break;
                    }
                    self.key_schedule.increment_read_counter();
                }
                ServerRecord::ChangeCipherSpec(_) => {}
            }
        }

         */
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

                            log::info!("ecdhe {:x?}", shared.as_bytes());

                            self.key_schedule
                                .initialize_handshake_secret(shared.as_bytes());

                            log::info!("***** handshake key schedule initialized");
                        }
                    }
                    ServerHandshake::EncryptedExtensions(_) => {}
                },
                ServerRecord::Alert => {
                    unimplemented!("alert not handled")
                }
                ServerRecord::ApplicationData(application_data) => {
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
                                    let inner = ServerHandshake::parse(&data[..data.len() - 1]);
                                    log::debug!("===> inner ==> {:?}", inner);
                                }
                                ContentType::ApplicationData => {}
                            }
                            log::debug!("decrypt result {:?}", result);
                            log::debug!("decrypted {:?} --> {:x?}", content_type, data);
                        }
                    }
                    self.key_schedule.increment_read_counter();
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
