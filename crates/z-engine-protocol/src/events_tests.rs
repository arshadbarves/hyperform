use super::*;

#[test]
fn events_use_camel_case_tags_and_fields() {
    let event = Event::TextDelta {
        agent_id: AgentId::main(),
        message_id: MessageId::from("m1"),
        text: "hi".into(),
    };
    let json = serde_json::to_value(&event).unwrap();
    assert_eq!(json["type"], "textDelta");
    assert_eq!(json["agentId"], "main");
    assert_eq!(json["messageId"], "m1");
    let back: Event = serde_json::from_value(json).unwrap();
    assert_eq!(back, event);
}

#[test]
fn trust_required_round_trips() {
    let event = Event::TrustRequired {
        project_root: "/work/app".into(),
        defines: vec!["hooks".into(), "MCP servers".into()],
    };
    let json = serde_json::to_value(&event).unwrap();
    assert_eq!(json["type"], "trustRequired");
    assert_eq!(json["projectRoot"], "/work/app");
    assert_eq!(json["defines"][1], "MCP servers");
    let back: Event = serde_json::from_value(json).unwrap();
    assert_eq!(back, event);
}

#[test]
fn compaction_started_names_its_trigger() {
    let event = Event::CompactionStarted {
        trigger: CompactionTrigger::Manual,
    };
    let json = serde_json::to_value(&event).unwrap();
    assert_eq!(json["type"], "compactionStarted");
    assert_eq!(json["trigger"], "manual");
    let back: Event = serde_json::from_value(json).unwrap();
    assert_eq!(back, event);
}

#[test]
fn envelope_nests_the_event() {
    let envelope = EventEnvelope {
        session_id: SessionId::from("S"),
        seq: 3,
        event: Event::notice(NoticeLevel::Warn, "careful"),
    };
    let json = serde_json::to_value(&envelope).unwrap();
    assert_eq!(json["sessionId"], "S");
    assert_eq!(json["event"]["type"], "notice");
    assert_eq!(json["event"]["level"], "warn");
}

#[test]
fn decision_advice_for_the_gui_round_trips() {
    use crate::decisions::{TurnTone, Urgency, UrgencyInfo};
    let tone = Event::TurnToneJudged {
        turn_id: TurnId::from("t1"),
        tone: TurnTone::DoneWell,
    };
    let json = serde_json::to_value(&tone).unwrap();
    assert_eq!(json["type"], "turnToneJudged");
    assert_eq!(
        (json["turnId"].clone(), json["tone"].clone()),
        ("t1".into(), "done_well".into())
    );
    assert_eq!(serde_json::from_value::<Event>(json).unwrap(), tone);
    let urgency = Event::UrgencyScored {
        urgency: UrgencyInfo {
            key: "notice:disk full".into(),
            urgency: Urgency::High,
        },
    };
    let json = serde_json::to_value(&urgency).unwrap();
    assert_eq!(json["type"], "urgencyScored");
    assert_eq!(json["urgency"]["urgency"], "high");
    assert_eq!(serde_json::from_value::<Event>(json).unwrap(), urgency);
}

#[test]
fn route_chosen_round_trips() {
    let event = Event::RouteChosen {
        route: crate::decisions::RouteInfo {
            agent_id: AgentId::main(),
            effort: Some(Effort::High),
            model: None,
            reason: "complex task".into(),
        },
    };
    let json = serde_json::to_value(&event).unwrap();
    assert_eq!(json["type"], "routeChosen");
    assert_eq!(json["route"]["agentId"], "main");
    assert_eq!(json["route"]["effort"], "high");
    assert!(json["route"]["model"].is_null());
    assert_eq!(serde_json::from_value::<Event>(json).unwrap(), event);
}
