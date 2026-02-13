//! Tests for IPC command construction and serialization.

#[cfg(test)]
mod tests {
    use crux_protocol::*;
    use serde_json::json;

    #[test]
    fn test_split_pane_command_from_json() {
        let json = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "crux:pane/split",
            "params": {
                "target_pane_id": 42,
                "direction": "right",
                "cwd": "/tmp"
            }
        });

        let req: JsonRpcRequest = serde_json::from_value(json).unwrap();
        assert_eq!(req.method, "crux:pane/split");

        let params: SplitPaneParams = serde_json::from_value(req.params.unwrap()).unwrap();
        assert_eq!(params.target_pane_id, Some(PaneId(42)));
        assert_eq!(params.direction, SplitDirection::Right);
        assert_eq!(params.cwd, Some("/tmp".to_string()));
    }

    #[test]
    fn test_send_text_command_from_json() {
        let json = json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "crux:pane/send-text",
            "params": {
                "pane_id": 1,
                "text": "echo hello",
                "bracketed_paste": true
            }
        });

        let req: JsonRpcRequest = serde_json::from_value(json).unwrap();
        let params: SendTextParams = serde_json::from_value(req.params.unwrap()).unwrap();
        assert_eq!(params.text, "echo hello");
        assert!(params.bracketed_paste);
    }

    #[test]
    fn test_handshake_command_from_json() {
        let json = json!({
            "jsonrpc": "2.0",
            "id": "handshake-1",
            "method": "crux:handshake",
            "params": {
                "client_name": "crux-cli",
                "client_version": "0.1.0",
                "protocol_version": "1.0",
                "capabilities": ["pane", "window", "clipboard"]
            }
        });

        let req: JsonRpcRequest = serde_json::from_value(json).unwrap();
        assert_eq!(req.id, Some(JsonRpcId::String("handshake-1".to_string())));

        let params: HandshakeParams = serde_json::from_value(req.params.unwrap()).unwrap();
        assert_eq!(params.client_name, "crux-cli");
        assert_eq!(params.capabilities, vec!["pane", "window", "clipboard"]);
    }

    #[test]
    fn test_list_panes_command_no_params() {
        let json = json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "crux:pane/list"
        });

        let req: JsonRpcRequest = serde_json::from_value(json).unwrap();
        assert_eq!(req.method, "crux:pane/list");
        assert!(req.params.is_none());
    }

    #[test]
    fn test_notification_no_id() {
        let json = json!({
            "jsonrpc": "2.0",
            "method": "crux:events/notify",
            "params": {
                "event": "pane_closed"
            }
        });

        let req: JsonRpcRequest = serde_json::from_value(json).unwrap();
        assert!(req.id.is_none(), "notification should not have an id");
    }

    #[test]
    fn test_error_response() {
        let resp = JsonRpcResponse::error(JsonRpcId::Number(1), -1001, "Pane not found");
        let json = serde_json::to_value(&resp).unwrap();

        assert!(json["error"].is_object());
        assert_eq!(json["error"]["code"], -1001);
        assert_eq!(json["error"]["message"], "Pane not found");
        assert!(json["result"].is_null() || !json.get("result").is_some());
    }

    #[test]
    fn test_success_response() {
        let result_data = json!({
            "pane_id": 5,
            "window_id": 1,
            "tab_id": 2
        });
        let resp = JsonRpcResponse::success(JsonRpcId::Number(10), result_data.clone());
        let json = serde_json::to_value(&resp).unwrap();

        assert_eq!(json["result"], result_data);
        assert!(json["error"].is_null() || !json.get("error").is_some());
    }

    #[test]
    fn test_window_create_params() {
        let json = json!({
            "title": "My Window",
            "width": 1920,
            "height": 1080
        });

        let params: WindowCreateParams = serde_json::from_value(json).unwrap();
        assert_eq!(params.title, Some("My Window".to_string()));
        assert_eq!(params.width, Some(1920));
        assert_eq!(params.height, Some(1080));
    }

    #[test]
    fn test_clipboard_write_text() {
        let json = json!({
            "content_type": "text",
            "text": "Hello clipboard"
        });

        let params: ClipboardWriteParams = serde_json::from_value(json).unwrap();
        assert_eq!(
            params.content_type,
            crux_protocol::ClipboardContentType::Text
        );
        assert_eq!(params.text, Some("Hello clipboard".to_string()));
    }

    #[test]
    fn test_clipboard_read_result_variants() {
        // Test text variant
        let text_result = ClipboardReadResult::Text {
            text: "clipboard text".to_string(),
        };
        let json = serde_json::to_value(&text_result).unwrap();
        assert_eq!(json["content_type"], "text");
        assert_eq!(json["text"], "clipboard text");

        // Test image variant
        let img_result = ClipboardReadResult::Image {
            image_path: "/tmp/clip.png".to_string(),
        };
        let json = serde_json::to_value(&img_result).unwrap();
        assert_eq!(json["content_type"], "image");

        // Test file paths variant
        let files_result = ClipboardReadResult::FilePaths {
            paths: vec!["file1.txt".to_string(), "file2.txt".to_string()],
        };
        let json = serde_json::to_value(&files_result).unwrap();
        assert_eq!(json["content_type"], "file_paths");
        assert_eq!(json["paths"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn test_ime_state_result() {
        let json = json!({
            "composing": true,
            "preedit_text": "ㅎㅏㄴ",
            "input_source": "com.apple.inputmethod.Korean.2SetKorean"
        });

        let result: ImeStateResult = serde_json::from_value(json).unwrap();
        assert!(result.composing);
        assert_eq!(result.preedit_text, Some("ㅎㅏㄴ".to_string()));
        assert!(result.input_source.unwrap().contains("Korean"));
    }

    #[test]
    fn test_pane_info_deserialization() {
        let json = json!({
            "pane_id": 7,
            "window_id": 2,
            "tab_id": 1,
            "size": {
                "cols": 80,
                "rows": 24
            },
            "is_active": true,
            "is_zoomed": false,
            "cursor_x": 0,
            "cursor_y": 0,
            "title": "bash",
            "cwd": "/home/user",
            "tty": "/dev/ttys002"
        });

        let info: PaneInfo = serde_json::from_value(json).unwrap();
        assert_eq!(info.pane_id, PaneId(7));
        assert_eq!(info.size.cols, 80);
        assert_eq!(info.size.rows, 24);
        assert!(info.is_active);
    }
}

#[cfg(test)]
mod proptests {
    use crux_protocol::*;
    use proptest::prelude::*;

    fn arb_pane_id() -> impl Strategy<Value = PaneId> {
        (1u64..1000).prop_map(PaneId)
    }

    fn arb_method() -> impl Strategy<Value = &'static str> {
        prop_oneof![
            Just("crux:pane/split"),
            Just("crux:pane/send-text"),
            Just("crux:pane/list"),
            Just("crux:pane/close"),
            Just("crux:window/create"),
            Just("crux:clipboard/read"),
        ]
    }

    proptest! {
        #[test]
        fn jsonrpc_request_serialization_never_panics(
            id in any::<u64>(),
            method in arb_method(),
        ) {
            let req = JsonRpcRequest::new(JsonRpcId::Number(id), method, None);
            let _ = serde_json::to_string(&req);
        }

        #[test]
        fn send_text_params_roundtrip(
            pane_id in arb_pane_id(),
            text in ".{0,100}",
            bracketed in any::<bool>(),
        ) {
            let params = SendTextParams {
                pane_id: Some(pane_id),
                text: text.clone(),
                bracketed_paste: bracketed,
            };
            let json = serde_json::to_string(&params).unwrap();
            let parsed: SendTextParams = serde_json::from_str(&json).unwrap();
            prop_assert_eq!(parsed.text, text);
            prop_assert_eq!(parsed.bracketed_paste, bracketed);
        }

        #[test]
        fn split_pane_result_roundtrip(
            pane_id in arb_pane_id(),
            cols in 1u32..200,
            rows in 1u32..100,
        ) {
            let result = SplitPaneResult {
                pane_id,
                window_id: WindowId(1),
                tab_id: TabId(1),
                size: PaneSize {
                    cols,
                    rows,
                },
                tty: Some("/dev/ttys001".to_string()),
            };
            let json = serde_json::to_string(&result).unwrap();
            let parsed: SplitPaneResult = serde_json::from_str(&json).unwrap();
            prop_assert_eq!(parsed.pane_id, pane_id);
            prop_assert_eq!(parsed.size.cols, cols);
            prop_assert_eq!(parsed.size.rows, rows);
        }

        #[test]
        fn window_create_params_roundtrip(
            width in 100u32..3000,
            height in 100u32..2000,
        ) {
            let params = WindowCreateParams {
                title: Some("Test".to_string()),
                width: Some(width),
                height: Some(height),
            };
            let json = serde_json::to_string(&params).unwrap();
            let parsed: WindowCreateParams = serde_json::from_str(&json).unwrap();
            prop_assert_eq!(parsed.width, Some(width));
            prop_assert_eq!(parsed.height, Some(height));
        }
    }
}
