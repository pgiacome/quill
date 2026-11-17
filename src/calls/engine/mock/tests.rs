use super::*;

/// Phase C2g: the audio SSRC is the first `a=ssrc:` line of the
/// audio `m=` section; video sections are ignored, and 0 is the
/// honest fallback (the join still goes out).
#[test]
fn group_offer_audio_source_id_parses_first_audio_ssrc() {
    let offer = "v=0\r\n\
            m=audio 9 UDP/TLS/RTP/SAVPF 111\r\n\
            a=ssrc:12345 cname:audio\r\n\
            a=ssrc:12346 cname:audio\r\n\
            m=video 9 UDP/TLS/RTP/SAVPF 96\r\n\
            a=ssrc:99999 cname:video\r\n";
    assert_eq!(group_offer_audio_source_id(offer), 12345);
}

/// tgcalls' payload is tdesktop's `GroupJoinPayload` JSON: its `ssrc`
/// is the audio ssrc, sent as a signed 32-bit number.
#[test]
fn group_offer_audio_source_id_reads_the_json_payload() {
    let payload = r#"{"ufrag":"u","pwd":"p","fingerprints":[],"ssrc":-1234567,"ssrc-groups":[]}"#;
    assert_eq!(group_offer_audio_source_id(payload), -1234567);
    assert_eq!(group_offer_audio_source_id(r#"{"ssrc":42}"#), 42);
    assert_eq!(group_offer_audio_source_id(r#"{"ufrag":"u"}"#), 0);
}

#[test]
fn group_offer_audio_source_id_zero_without_audio_ssrc() {
    assert_eq!(
        group_offer_audio_source_id("v=0\r\nm=video 9 UDP/TLS/RTP/SAVPF 96\r\n"),
        0
    );
    assert_eq!(group_offer_audio_source_id(""), 0);
}

/// Phase C2i: a mid-call screen-share intent survives a transport
/// reconnect — `connect` re-runs with the retained params and the
/// engine must re-issue screen-only (not flip back to the camera).
#[test]
fn retained_call_media_preserves_screen_share_across_reconnect() {
    let params = ConnectParams {
        custom_parameters: String::new(),
        encryption_key: vec![1; 256],
        is_outgoing: false,
        servers: Vec::new(),
        library_versions: vec!["13.0.0".into()],
        p2p_allowed: true,
        mic_input: Some("mic-1".into()),
        speaker_input: Some("speaker-1".into()),
        camera_input: Some("cam-1".into()),
    };
    // Fresh connect: no prior entry, share starts off.
    let fresh = retained_call_media(&params, None);
    assert!(!fresh.video.screen());
    assert_eq!(fresh.video.camera(), Some("cam-1"));
    assert_eq!(fresh.mic.as_deref(), Some("mic-1"));
    // Mid-call the user starts sharing, then the transport drops
    // and `connect` re-runs with the same retained params: the
    // share intent must survive so `set_media_sources` re-issues
    // screen-only (`camera.is_some() && !screen_share_on` stays
    // false for the camera).
    let mut sharing = fresh;
    sharing.video = VideoSource::Screen;
    let reconnected = retained_call_media(&params, Some(&sharing));
    assert!(reconnected.video.screen());
    assert_eq!(
        reconnected.video.camera(),
        None,
        "the share replaces the camera"
    );
    // Stopping share before the drop must not stick the flag on
    // either.
    let mut stopped = sharing;
    stopped.video = VideoSource::Off;
    assert!(!retained_call_media(&params, Some(&stopped)).video.screen());
}

#[test]
fn pick_camera_prefers_the_listed_pick() {
    let device = |id: &str, kind| MediaDevice {
        id: id.into(),
        name: id.into(),
        kind,
    };
    let devices = [
        device("mic-1", MediaDeviceKind::Microphone),
        device("cam-1", MediaDeviceKind::Camera),
        device("cam-2", MediaDeviceKind::Camera),
    ];
    assert_eq!(pick_camera(None, &devices), Some("cam-1".to_string()));
    assert_eq!(
        pick_camera(Some("cam-2"), &devices),
        Some("cam-2".to_string())
    );
    // An unplugged pick falls back to the first camera, never to a
    // microphone with the same id.
    assert_eq!(
        pick_camera(Some("gone"), &devices),
        Some("cam-1".to_string())
    );
    assert_eq!(
        pick_camera(Some("mic-1"), &devices),
        Some("cam-1".to_string())
    );
    assert_eq!(pick_camera(Some("cam-1"), &devices[..1]), None);
}
