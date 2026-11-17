use quill::ids::RequestId;
use quill::telegram::requests::search_messages;
use serde_json::Value;

#[test]
fn community_chat_type_filter_none_is_null() {
    // No community selected: the historical all-chat-types behavior (null
    // filter slot in `searchMessages`).
    let json = search_messages(RequestId(7), "hello", 20, None);
    let v: Value = serde_json::from_str(&json).unwrap();
    assert_eq!(v["chat_type_filter"], Value::Null);
}

#[test]
fn community_chat_type_filter_builds_community_constructor() {
    let json = search_messages(RequestId(7), "hello", 20, Some(42));
    let v: Value = serde_json::from_str(&json).unwrap();
    assert_eq!(
        v["chat_type_filter"]["@type"],
        "searchMessagesChatTypeFilterCommunity"
    );
    assert_eq!(v["chat_type_filter"]["community_id"], 42);
}
