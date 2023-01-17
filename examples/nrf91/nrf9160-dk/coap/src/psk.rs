use {
    crate::{IDENTITY, PSK, SECURITY_TAG},
    core::fmt::write,
    defmt::Format,
    heapless::String,
    nrf_modem::Error,
};

/// Credential Storage Management Types
#[derive(Clone, Copy, Format)]
#[allow(dead_code)]
enum CSMType {
    RootCert = 0,
    ClientCert = 1,
    ClientPrivateKey = 2,
    Psk = 3,
    PskId = 4,
    // ...
}

/// This function deletes a key or certificate from the nrf modem
async fn key_delete(ty: CSMType) -> Result<(), Error> {
    let mut cmd: String<32> = String::new();
    write(
        &mut cmd,
        format_args!("AT%CMNG=3,{},{}", SECURITY_TAG, ty as u32),
    )
    .unwrap();
    nrf_modem::send_at::<32>(cmd.as_str()).await?;
    Ok(())
}

/// This function writes a key or certificate to the nrf modem
async fn key_write(ty: CSMType, data: &str) -> Result<(), Error> {
    let mut cmd: String<128> = String::new();
    write(
        &mut cmd,
        format_args!(r#"AT%CMNG=0,{},{},"{}""#, SECURITY_TAG, ty as u32, data),
    )
    .unwrap();

    nrf_modem::send_at::<128>(&cmd.as_str()).await?;

    Ok(())
}

/// Delete existing keys/certificates and loads new ones based on config.rs entries
pub async fn install_psk_id_and_psk() -> Result<(), Error> {
    assert!(
        !&IDENTITY.is_empty() && !&PSK.is_empty(),
        "PSK ID and PSK must not be empty. Set them in the `config` module."
    );

    key_delete(CSMType::PskId).await?;
    key_delete(CSMType::Psk).await?;

    key_write(CSMType::PskId, &IDENTITY).await?;
    key_write(CSMType::Psk, &encode_psk_as_hex(&PSK.as_bytes()[..])).await?;

    Ok(())
}

fn encode_psk_as_hex(psk: &[u8]) -> String<128> {
    fn hex_from_digit(num: u8) -> char {
        if num < 10 {
            (b'0' + num) as char
        } else {
            (b'a' + num - 10) as char
        }
    }

    let mut s: String<128> = String::new();
    for ch in psk {
        s.push(hex_from_digit(ch / 16)).unwrap();
        s.push(hex_from_digit(ch % 16)).unwrap();
    }

    s
}
