//! Find a service's RFCOMM channel by asking the device's SDP server.
//! Protocol: docs/protocol/transport.md#sdp.

use std::time::{Duration, Instant};

use super::{classify, socket};
use crate::bt::{BdAddr, Uuid};
use crate::error::{Error, Result};
use crate::reader::Reader;

/// Spec: transport.md#sockets
const SDP_PSM: u16 = 1;
/// Spec: transport.md#sdp-pdu
const ERROR_RESPONSE: u8 = 0x01;
/// Spec: transport.md#sdp-pdu
const SEARCH_ATTRIBUTE_REQUEST: u8 = 0x06;
/// Spec: transport.md#sdp-pdu
const SEARCH_ATTRIBUTE_RESPONSE: u8 = 0x07;
/// ProtocolDescriptorList. Spec: transport.md#sdp-request
const PROTOCOL_DESCRIPTOR_LIST: u16 = 0x0004;
/// Spec: transport.md#sdp-response
const MAX_ROUNDS: u16 = 16;
/// Continuation state is at most 16 bytes. Spec: transport.md#sdp-request
const MAX_CONTINUATION: usize = 16;
/// Nesting deeper than this is not a real service record.
const MAX_DEPTH: usize = 8;

const TIMEOUT: Duration = Duration::from_secs(5);

/// The RFCOMM channel of `service` on the device, or `None` if it has no such service.
pub fn rfcomm_channel(address: BdAddr, service: Uuid) -> Result<Option<u8>> {
    let fd = socket::connect(address, socket::Target::L2cap { psm: SDP_PSM }, TIMEOUT)
        .map_err(|e| classify(e, "connecting to the SDP server", false))?;

    let mut attribute_lists = Vec::new();
    let mut continuation = Vec::new();
    for transaction in 1..=MAX_ROUNDS {
        let request = search_attribute_request(transaction, service, &continuation)?;
        socket::write_all(&fd, &request).map_err(|e| classify(e, "sending an SDP request", false))?;

        let readable =
            socket::wait_readable(&fd, Instant::now() + TIMEOUT).map_err(|e| classify(e, "waiting for SDP", false))?;
        if !readable {
            return Err(Error::Timeout("SDP service search".to_owned()));
        }
        let mut pdu = vec![0u8; 4096];
        let n = socket::read(&fd, &mut pdu).map_err(|e| classify(e, "reading SDP", false))?;
        pdu.truncate(n);

        let response = parse_response(&pdu, transaction)?;
        attribute_lists.extend_from_slice(response.attributes);
        if response.continuation.is_empty() {
            return channel_from_attribute_lists(&attribute_lists);
        }
        continuation = response.continuation.to_vec();
    }
    Err(Error::Protocol("SDP answer did not end after 16 rounds".to_owned()))
}

/// A ServiceSearchAttributeRequest for one 128-bit UUID, asking only for the
/// ProtocolDescriptorList. Spec: transport.md#sdp-request
fn search_attribute_request(transaction: u16, service: Uuid, continuation: &[u8]) -> Result<Vec<u8>> {
    let continuation_len = u8::try_from(continuation.len())
        .ok()
        .filter(|&len| usize::from(len) <= MAX_CONTINUATION)
        .ok_or_else(|| Error::Protocol("SDP continuation state too long".to_owned()))?;

    let mut parameters = vec![0x35, 0x11, 0x1C]; // sequence of 17 bytes: one 128-bit UUID
    parameters.extend(service.to_be_bytes());
    parameters.extend([0xFF, 0xFF]); // maximum attribute byte count
    parameters.extend([0x35, 0x03, 0x09]); // sequence of 3 bytes: one 16-bit attribute id
    parameters.extend(PROTOCOL_DESCRIPTOR_LIST.to_be_bytes());
    parameters.push(continuation_len);
    parameters.extend(continuation);

    let parameter_len =
        u16::try_from(parameters.len()).map_err(|_| Error::Protocol("SDP request too long".to_owned()))?;
    let mut pdu = vec![SEARCH_ATTRIBUTE_REQUEST];
    pdu.extend(transaction.to_be_bytes());
    pdu.extend(parameter_len.to_be_bytes());
    pdu.extend(parameters);
    Ok(pdu)
}

struct Response<'a> {
    attributes: &'a [u8],
    continuation: &'a [u8],
}

/// Spec: transport.md#sdp-response
fn parse_response(pdu: &[u8], transaction: u16) -> Result<Response<'_>> {
    let mut header = Reader::new(pdu, "SDP response");
    let id = header.u8()?;
    let answered = header.u16_be()?;
    let len = header.u16_be()?;
    let mut parameters = Reader::new(header.take(usize::from(len))?, "SDP response");
    match id {
        SEARCH_ATTRIBUTE_RESPONSE if answered == transaction => {
            let count = parameters.u16_be()?;
            let attributes = parameters.take(usize::from(count))?;
            let continuation_len = parameters.u8()?;
            if usize::from(continuation_len) > MAX_CONTINUATION {
                return Err(Error::Protocol("SDP continuation state too long".to_owned()));
            }
            let continuation = parameters.take(usize::from(continuation_len))?;
            Ok(Response {
                attributes,
                continuation,
            })
        }
        SEARCH_ATTRIBUTE_RESPONSE => Err(Error::Protocol(format!(
            "SDP answered transaction {answered}, expected {transaction}"
        ))),
        ERROR_RESPONSE => Err(Error::Protocol(format!(
            "SDP error response 0x{:04X}",
            parameters.u16_be()?
        ))),
        other => Err(Error::Protocol(format!("unexpected SDP PDU 0x{other:02X}"))),
    }
}

/// The parts of an SDP data element budctl looks at. Spec: transport.md#sdp-data-elements
#[derive(Debug, PartialEq)]
enum Element {
    Uint(u64),
    Uuid(Uuid),
    /// A sequence or an alternative.
    List(Vec<Element>),
    Other,
}

fn element(r: &mut Reader, depth: usize) -> Result<Element> {
    if depth > MAX_DEPTH {
        return Err(Error::Protocol("SDP data nested too deeply".to_owned()));
    }
    let header = r.u8()?;
    let (kind, size) = (header >> 3, header & 0x07);
    let len = match size {
        0 if kind == 0 => 0, // nil
        0 => 1,
        1 => 2,
        2 => 4,
        3 => 8,
        4 => 16,
        5 => usize::from(r.u8()?),
        6 => usize::from(r.u16_be()?),
        _ => usize::try_from(r.u32_be()?).map_err(|_| Error::Protocol("SDP element too long".to_owned()))?,
    };
    let data = r.take(len)?;
    Ok(match kind {
        1 => match data.len() {
            1 | 2 | 4 | 8 => Element::Uint(data.iter().fold(0, |n, &b| (n << 8) | u64::from(b))),
            _ => Element::Other,
        },
        3 => Element::Uuid(Uuid::from_sdp(data)?),
        6 | 7 => {
            let mut inner = Reader::new(data, "SDP sequence");
            let mut items = Vec::new();
            while !inner.is_empty() {
                items.push(element(&mut inner, depth + 1)?);
            }
            Element::List(items)
        }
        _ => Element::Other,
    })
}

/// The RFCOMM channel in the (joined) AttributeLists of a ServiceSearchAttributeResponse.
/// Spec: transport.md#sdp-rfcomm-channel
fn channel_from_attribute_lists(bytes: &[u8]) -> Result<Option<u8>> {
    let mut r = Reader::new(bytes, "SDP attribute lists");
    let Element::List(records) = element(&mut r, 0)? else {
        return Err(Error::Protocol("SDP attribute lists are not a sequence".to_owned()));
    };
    for record in &records {
        let Element::List(attributes) = record else { continue };
        for pair in attributes.chunks(2) {
            if let [Element::Uint(id), value] = pair
                && *id == u64::from(PROTOCOL_DESCRIPTOR_LIST)
                && let Some(channel) = find_rfcomm(value)
            {
                return u8::try_from(channel)
                    .map(Some)
                    .map_err(|_| Error::Protocol(format!("RFCOMM channel {channel} out of range")));
            }
        }
    }
    Ok(None)
}

/// A protocol descriptor is a sequence that starts with the protocol's UUID; for RFCOMM the
/// channel number comes next. Searching nested lists also covers records with alternatives.
fn find_rfcomm(element: &Element) -> Option<u64> {
    let Element::List(items) = element else { return None };
    if let [Element::Uuid(uuid), Element::Uint(channel), ..] = items.as_slice()
        && *uuid == Uuid::RFCOMM
    {
        return Some(*channel);
    }
    items.iter().find_map(find_rfcomm)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::mock::hex;

    const SERVICE: Uuid = Uuid::from_u128(0x0000_079A_D102_11E1_9B23_0002_5B00_A5A5);

    /// One record: ProtocolDescriptorList = (L2CAP), (RFCOMM, channel 15).
    /// Spec: transport.md#sdp-rfcomm-channel
    const ATTRIBUTE_LISTS: &str = "35 13 35 11 09 00 04 35 0C 35 03 19 01 00 35 05 19 00 03 08 0F";

    #[test]
    fn request_layout() {
        let pdu = search_attribute_request(1, SERVICE, &[]).unwrap();
        let mut expected = hex("06 00 01 00 1B 35 11 1C");
        expected.extend(SERVICE.to_be_bytes());
        expected.extend(hex("FF FF 35 03 09 00 04 00"));
        assert_eq!(pdu, expected);

        let next = search_attribute_request(2, SERVICE, &[0xAB, 0xCD]).unwrap();
        assert_eq!(next.get(1..5), Some(&hex("00 02 00 1D")[..]));
        assert!(next.ends_with(&hex("00 04 02 AB CD")));
    }

    #[test]
    fn finds_the_channel_in_one_response() {
        let mut pdu = hex("07 00 01 00 18 00 15");
        pdu.extend(hex(ATTRIBUTE_LISTS));
        pdu.push(0x00);
        let response = parse_response(&pdu, 1).unwrap();
        assert!(response.continuation.is_empty());
        assert_eq!(channel_from_attribute_lists(response.attributes).unwrap(), Some(15));
    }

    #[test]
    fn joins_continued_responses() {
        let lists = hex(ATTRIBUTE_LISTS);
        let (first, second) = lists.split_at(9);
        let mut pdu1 = vec![0x07, 0x00, 0x01, 0x00, 0x0E, 0x00, 0x09];
        pdu1.extend(first);
        pdu1.extend([0x02, 0xAB, 0xCD]);
        let r1 = parse_response(&pdu1, 1).unwrap();
        assert_eq!(r1.continuation, &[0xAB, 0xCD]);

        let mut pdu2 = vec![0x07, 0x00, 0x02, 0x00, 0x0F, 0x00, 0x0C];
        pdu2.extend(second);
        pdu2.push(0x00);
        let r2 = parse_response(&pdu2, 2).unwrap();

        let joined = [r1.attributes, r2.attributes].concat();
        assert_eq!(channel_from_attribute_lists(&joined).unwrap(), Some(15));
    }

    #[test]
    fn no_rfcomm_descriptor_means_no_channel() {
        // ProtocolDescriptorList with L2CAP only.
        let lists = hex("35 0C 35 0A 09 00 04 35 05 35 03 19 01 00");
        assert_eq!(channel_from_attribute_lists(&lists).unwrap(), None);
        // No matching record at all.
        assert_eq!(channel_from_attribute_lists(&hex("35 00")).unwrap(), None);
    }

    #[test]
    fn errors_and_garbage_are_protocol_errors() {
        assert!(matches!(
            parse_response(&hex("01 00 01 00 02 00 03"), 1),
            Err(Error::Protocol(_))
        ));
        assert!(matches!(
            parse_response(&hex("07 00 09 00 03 00 00 00"), 1),
            Err(Error::Protocol(_))
        ));
        assert!(parse_response(&hex("07 00 01 00 05 00 09"), 1).is_err());
        assert!(channel_from_attribute_lists(&hex("35 05 35")).is_err());
        assert!(channel_from_attribute_lists(&hex("09 00 04")).is_err());
        // Deep nesting is refused instead of recursing without bound.
        let mut nested = Vec::new();
        for _ in 0..20 {
            let mut outer = vec![0x35, u8::try_from(nested.len()).unwrap()];
            outer.extend(nested);
            nested = outer;
        }
        assert!(channel_from_attribute_lists(&nested).is_err());
    }
}
