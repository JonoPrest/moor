//! Framing (plan 2.1): round-trip, partial reads, oversized frames.

use nits_protocol::{ClientMsg, Envelope, ProtocolVersion, RequestId};
use nitsd::codec::{self, CodecError, MAX_FRAME};
use tokio::io::{AsyncWriteExt, duplex};

fn cancel(n: u64) -> Envelope<ClientMsg> {
    Envelope::current(ClientMsg::Cancel {
        id: RequestId::new(n),
    })
}

#[tokio::test]
async fn round_trips_several_frames() {
    let (mut a, mut b) = duplex(64);
    let writer = tokio::spawn(async move {
        for i in 0..3 {
            codec::write_msg(&mut a, &cancel(i)).await.unwrap();
        }
    });
    for i in 0..3 {
        let got = codec::read_msg::<_, ClientMsg>(&mut b)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(got, cancel(i));
    }
    writer.await.unwrap();
    assert!(
        codec::read_msg::<_, ClientMsg>(&mut b)
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn reassembles_partial_writes() {
    let (mut a, mut b) = duplex(4);
    let bytes = codec::encode(&cancel(7)).unwrap();
    let mut frame = u32::try_from(bytes.len()).unwrap().to_be_bytes().to_vec();
    frame.extend(&bytes);
    let writer = tokio::spawn(async move {
        for chunk in frame.chunks(3) {
            a.write_all(chunk).await.unwrap();
            a.flush().await.unwrap();
            tokio::task::yield_now().await;
        }
    });
    let got = codec::read_msg::<_, ClientMsg>(&mut b)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(got, cancel(7));
    writer.await.unwrap();
}

#[tokio::test]
async fn eof_mid_frame_is_an_error() {
    let (mut a, mut b) = duplex(64);
    a.write_all(&10u32.to_be_bytes()).await.unwrap();
    a.write_all(b"abc").await.unwrap();
    drop(a);
    assert!(matches!(
        codec::read_frame(&mut b).await,
        Err(CodecError::Io(_))
    ));
}

#[tokio::test]
async fn oversized_frame_is_rejected_before_allocation() {
    let (mut a, mut b) = duplex(64);
    a.write_all(&(MAX_FRAME + 1).to_be_bytes()).await.unwrap();
    let err = codec::read_frame(&mut b).await.unwrap_err();
    assert!(matches!(err, CodecError::Oversized { len } if len == MAX_FRAME + 1));
    assert!(matches!(
        codec::write_frame(&mut a, &vec![0; MAX_FRAME as usize + 1]).await,
        Err(CodecError::Oversized { .. })
    ));
}

#[test]
fn version_is_visible_on_every_frame() {
    let bytes = codec::encode(&Envelope {
        v: ProtocolVersion::new(0, 1, 0),
        msg: ClientMsg::Cancel {
            id: RequestId::new(1),
        },
    })
    .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["v"], "0.1.0");
    assert_eq!(json["msg"]["type"], "Cancel");
}
