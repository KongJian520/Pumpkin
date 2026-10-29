//! Encoding for Java player profiles supplied through the plugin API.
use bytes::Bytes;
use pumpkin_core::net::java::JavaClient;
use pumpkin_data::packet::CURRENT_MC_VERSION;
use pumpkin_protocol::{
    Property,
    java::client::play::{CPlayerInfoUpdate, Player, PlayerAction, PlayerInfoFlags},
};

/// Encodes a profile registration without adding a player to the visible Tab list.
///
/// # Errors
/// Returns an error for oversized properties, an invalid profile name, or an encoding failure.
pub fn encode_profile(
    id: uuid::Uuid,
    name: &str,
    properties: &[Property],
) -> Result<Bytes, String> {
    if name.is_empty()
        || name.len() > 16
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    {
        return Err("Profile name must contain 1-16 ASCII letters, digits, or underscores".into());
    }
    if properties.len() > 16
        || properties.iter().any(|property| {
            property.name.is_empty()
                || property.name.len() > 64
                || property.value.len() > 16_384
                || property
                    .signature
                    .as_ref()
                    .is_some_and(|signature| signature.len() > 4096)
        })
    {
        return Err("Profile properties exceed the plugin API size limits".into());
    }
    let actions = [
        PlayerAction::AddPlayer { name, properties },
        PlayerAction::UpdateListed(false),
    ];
    let entries = [Player {
        uuid: id,
        actions: &actions,
    }];
    let flags = PlayerInfoFlags::ADD_PLAYER | PlayerInfoFlags::UPDATE_LISTED;
    let packet = CPlayerInfoUpdate::new(flags.bits(), &entries);
    let mut bytes = Vec::new();
    // The connection's translation pipeline consumes the current protocol format.
    JavaClient::write_packet_for_version(&packet, CURRENT_MC_VERSION, &mut bytes)
        .map_err(|error| error.to_string())?;
    Ok(bytes.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use pumpkin_protocol::ser::NetworkReadExt;

    #[test]
    fn profile_packet_preserves_signed_textures_and_is_unlisted() {
        let id = uuid::Uuid::from_u128(0x00112233445566778899aabbccddeeff);
        let properties = [Property {
            name: "textures".into(),
            value: "skin-value".into(),
            signature: Some("skin-signature".into()),
        }];
        let bytes = encode_profile(id, "NpcGuide", &properties).unwrap();
        let mut packet = bytes.as_ref();
        // Skip the version-dependent packet ID; the remainder is a fixed wire fixture.
        packet.get_var_int().unwrap();
        let mut expected = vec![0x09, 0x01];
        expected.extend_from_slice(&id.as_u128().to_be_bytes());
        expected.extend_from_slice(
            b"\x08NpcGuide\x01\x08textures\x0askin-value\x01\x0eskin-signature\x00",
        );
        assert_eq!(packet, expected);
        assert!(encode_profile(id, "invalid name", &properties).is_err());
        let oversized = [Property {
            name: "textures".into(),
            value: "a".repeat(16_385).into(),
            signature: None,
        }];
        assert!(encode_profile(id, "NpcGuide", &oversized).is_err());
    }
}
