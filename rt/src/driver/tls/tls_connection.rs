use crate::api::ip::tcp::{TcpError, TcpSocket, TcpStack};
use crate::api::ip::{IpProtocol, SocketAddress};
use crate::driver::tls::config::Config;
use crate::driver::tls::handshake::{ClientHandshake, HandshakeType, ServerHandshake};
use crate::driver::tls::key_schedule::KeySchedule;
use crate::driver::tls::record::{ClientRecord, ServerRecord};
use crate::driver::tls::TlsError;
use crate::prelude::Address;
use digest::generic_array::typenum::Unsigned;
use heapless::{consts::*, ArrayLength, Vec};
use hkdf::Hkdf;
use hmac::Hmac;
use rand_core::{CryptoRng, RngCore};
use sha2::{Digest, Sha256};

use crate::driver::tls::application_data::ApplicationData;
use crate::driver::tls::content_types::ContentType;
use crate::driver::tls::handshake::HandshakeType::Finished;
use crate::driver::tls::parse_buffer::ParseBuffer;
use aes_gcm::aead::{generic_array::GenericArray, AeadInPlace, Buffer, NewAead};
use aes_gcm::Aes128Gcm;
use digest::{BlockInput, FixedOutput, Reset, Update};

enum State {
    Unencrypted,
    Encrypted,
}

pub struct TlsConnection<RNG, Tcp, D>
where
    RNG: CryptoRng + RngCore + Copy + 'static,
    Tcp: TcpStack + 'static,
    D: Update + BlockInput + FixedOutput + Reset + Default + Clone,
    D::BlockSize: ArrayLength<u8>,
    D::OutputSize: ArrayLength<u8>,
{
    delegate: TcpSocket<Tcp>,
    config: &'static Config<RNG>,
    state: State,
    key_schedule: KeySchedule<D, U16, U12>,
}

impl<RNG, Tcp, D> TlsConnection<RNG, Tcp, D>
where
    RNG: CryptoRng + RngCore + Copy,
    Tcp: TcpStack,
    D: Update + BlockInput + FixedOutput + Reset + Default + Clone,
    D::BlockSize: ArrayLength<u8>,
    D::OutputSize: ArrayLength<u8>,
{
    pub fn new(config: &'static Config<RNG>, delegate: TcpSocket<Tcp>) -> Self {
        Self {
            delegate,
            config,
            state: State::Unencrypted,
            key_schedule: KeySchedule::new(),
        }
    }

    fn encrypt<N: ArrayLength<u8>>(
        &self,
        buf: &mut Vec<u8, N>,
    ) -> Result<ApplicationData, TlsError> {
        //unimplemented!()
        log::info!("encrypt key {:x?}", self.key_schedule.get_client_key());
        log::info!("encrypt nonce {:x?}", self.key_schedule.get_client_nonce());
        log::info!("plaintext {} {:x?}", buf.len(), buf);
        let crypto = Aes128Gcm::new_varkey(&self.key_schedule.get_client_key()).unwrap();
        let nonce = &self.key_schedule.get_client_nonce();
        log::info!("client write nonce {:x?}", nonce);
        let len = (buf.len() + <Aes128Gcm as AeadInPlace>::TagSize::to_usize());
        log::info!(
            "output size {}",
            <Aes128Gcm as AeadInPlace>::TagSize::to_usize()
        );
        let len_bytes = (len as u16).to_be_bytes();
        let additional_data = [
            ContentType::ApplicationData as u8,
            0x03,
            0x03,
            len_bytes[0],
            len_bytes[1],
        ];
        crypto
            .encrypt_in_place(nonce, &additional_data, buf)
            .map_err(|_| TlsError::InvalidApplicationData)?;
        log::info!("aad {:x?}", additional_data);
        log::info!("ciphertext ## {} ## {:x?}", buf.len(), buf);
        //Ok(())
        let mut header = Vec::new();
        header.extend_from_slice(&additional_data);
        let mut data = Vec::new();
        data.extend(buf.iter());
        Ok(ApplicationData { header, data })
        //let result =
        //crypto.decrypt_in_place(&self.key_schedule.get_server_nonce(), &header, &mut data);
        //Ok(())
    }

    async fn transmit(&mut self, record: &ClientRecord<'_, RNG, D>) -> Result<(), TlsError> {
        let mut buf: Vec<u8, U1024> = Vec::new();
        record.encode(&mut buf, self.key_schedule.transcript_hash());
        log::info!(
            "**** transmit, hash={:x?}",
            self.key_schedule.transcript_hash().clone().finalize()
        );

        /*
        if let State::Encrypted = self.state {
            log::info!("ENCRYPTING {:x?}", buf);
            log::info!(" push content type {}", record.content_type() as u8);
            buf.push(record.content_type() as u8);
            let encrypted = ClientRecord::<RNG, D>::ApplicationData(self.encrypt(&mut buf)?);
            buf.clear();
            encrypted.encode(&mut buf, self.key_schedule.transcript_hash());
        }
         */

        self.delegate_socket()
            .write(&buf)
            .await
            .map(|_| ())
            .map_err(|e| TlsError::TcpError(e))?;

        self.key_schedule.increment_write_counter();
        Ok(())
    }

    async fn receive(&mut self) -> Result<ServerRecord<D>, TlsError> {
        let mut record =
            ServerRecord::read(&mut self.delegate, self.key_schedule.transcript_hash()).await?;

        //if let State::Handhshaking = self.state {
        //if let ServerRecord::Handshake(ServerHandshake::ServerHello(_)) = record {
        //self.state = State::EncryptedHandshaking;
        //}
        if let State::Encrypted = self.state {
            if let ServerRecord::ApplicationData(ApplicationData { header, mut data }) = record {
                log::info!("decrypting {:x?} with {}", &header, data.len());
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
                        let mut inner = ServerHandshake::parse(&mut buf);
                        if let Ok(ServerHandshake::Finished(ref mut finished)) = inner {
                            finished
                                .hash
                                .replace(self.key_schedule.transcript_hash().clone().finalize());
                        }
                        log::debug!("===> inner ==> {:?}", inner);
                        record = ServerRecord::Handshake(inner.unwrap());
                        //if hash_later {
                        self.key_schedule
                            .transcript_hash()
                            .update(&data[..data.len() - 1]);
                        log::info!("hash {:x?}", &data[..data.len() - 1]);
                        //}
                    }
                    _ => {
                        return Err(TlsError::InvalidHandshake);
                    }
                }
                log::debug!("decrypt result {:?}", result);
                //log::debug!("decrypted {:?} --> {:x?}", content_type, data);
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

                            self.state = State::Encrypted;
                        }
                    }
                    ServerHandshake::EncryptedExtensions(_) => {}
                    ServerHandshake::Certificate(_) => {}
                    ServerHandshake::CertificateVerify(_) => {}
                    ServerHandshake::Finished(finished) => {
                        log::info!("************* Finished");
                        let verified = self.key_schedule.verify_server_finished(&finished);
                        if verified {
                            log::info!("FINISHED! server verified {}", verified);
                            let client_finished = self
                                .key_schedule
                                .create_client_finished()
                                .map_err(|_| TlsError::InvalidHandshake)?;

                            //let client_finished =
                            //ClientRecord::Handshake(ClientHandshake::Finished(client_finished));
                            let client_finished =
                                ClientHandshake::<RNG, D>::Finished(client_finished);
                            let mut buf = Vec::<u8, U1024>::new();
                            let mut next_hash = self.key_schedule.transcript_hash().clone();
                            client_finished.encode(&mut buf, &mut next_hash);
                            buf.push(ContentType::Handshake as u8);
                            let client_finished = self.encrypt(&mut buf)?;

                            // ----------------------------------------
                            let crypto = Aes128Gcm::new(&self.key_schedule.get_client_key());
                            let nonce = &self.key_schedule.get_client_nonce();
                            log::info!("server write nonce {:x?}", nonce);
                            let mut decrypt_buf = Vec::<u8, U1024>::new();
                            decrypt_buf.extend(client_finished.data.iter());
                            let result = crypto.decrypt_in_place(
                                &nonce,
                                &client_finished.header,
                                &mut decrypt_buf,
                            );
                            log::info!("decrypted my own {:?} {:x?}", result, decrypt_buf);
                            // ----------------------------------------

                            let client_finished = ClientRecord::ApplicationData(client_finished);

                            log::info!("sending client FINISH");
                            self.transmit(&client_finished).await?;
                            self.key_schedule.replace_transcript_hash(next_hash);
                        }
                        break;
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

    pub fn delegate_socket(&mut self) -> &mut TcpSocket<Tcp> {
        &mut self.delegate
    }
}
